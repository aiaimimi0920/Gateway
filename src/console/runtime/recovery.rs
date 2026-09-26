//! Startup recovery reconciles journal phases against authoritative Redis and local YAML state.

use super::revision_state::{
    current_matches_record_new, current_matches_record_previous, receipt_from_record,
    redis_kept_previous_revision, redis_revision_matches_record,
    validated_and_metadata_from_active,
};
use super::{RouteConfigCoordinator, RouteConfigRuntimeError};
use crate::console::persistence::{TransactionPhase, TransactionRecord};
use crate::console::redis_store::RouteConfigRedisRevision;
use time::OffsetDateTime;

impl RouteConfigCoordinator {
    pub async fn recover_startup(&self) -> Result<(), RouteConfigRuntimeError> {
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let records = self
            .journal
            .inspect_locked(&guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let active_revision = self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?;

        for record in &records {
            match record.phase() {
                TransactionPhase::Prepared => {
                    if redis_revision_matches_record(active_revision.as_ref(), record)? {
                        self.complete_recovered_commit(
                            &guard,
                            record,
                            active_revision.as_ref(),
                            record.phase(),
                        )?;
                    } else if redis_kept_previous_revision(active_revision.as_ref(), record) {
                        self.abort_prepared_without_cas(&guard, record)?;
                    } else {
                        return Err(RouteConfigRuntimeError::new(
                            "console_recovery_required",
                            format!(
                                "Prepared transaction '{}' disagrees with the authoritative Redis active revision",
                                record.tx_id()
                            ),
                        ));
                    }
                }
                TransactionPhase::YamlReplaced => {
                    if redis_revision_matches_record(active_revision.as_ref(), record)? {
                        self.complete_recovered_commit(
                            &guard,
                            record,
                            active_revision.as_ref(),
                            record.phase(),
                        )?;
                    } else if redis_kept_previous_revision(active_revision.as_ref(), record) {
                        self.rollback_yaml_and_abort(
                            &guard,
                            record.tx_id(),
                            &receipt_from_record(record),
                            "yaml_replaced_without_redis_activation",
                        )?;
                    } else {
                        return Err(RouteConfigRuntimeError::new(
                            "console_recovery_required",
                            format!(
                                "YamlReplaced transaction '{}' disagrees with the authoritative Redis active revision",
                                record.tx_id()
                            ),
                        ));
                    }
                }
                TransactionPhase::RedisActivated => {
                    if redis_revision_matches_record(active_revision.as_ref(), record)? {
                        self.complete_recovered_commit(
                            &guard,
                            record,
                            active_revision.as_ref(),
                            record.phase(),
                        )?;
                    } else {
                        return Err(RouteConfigRuntimeError::new(
                            "console_recovery_required",
                            format!(
                                "RedisActivated transaction '{}' is ahead of the authoritative Redis active revision",
                                record.tx_id()
                            ),
                        ));
                    }
                }
                TransactionPhase::Committed | TransactionPhase::Aborted => {}
            }
        }

        if active_revision.is_none() {
            if let Some(record) = records
                .iter()
                .filter(|record| record.phase() == TransactionPhase::Committed)
                .max_by_key(|record| record.created_at())
            {
                self.install_committed_local_revision(&guard, record)?;
            }
        } else if let Some(active_revision) = active_revision.as_ref() {
            self.ensure_store_matches_revision(active_revision)?;
        }

        Ok(())
    }

    fn abort_prepared_without_cas(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRuntimeError> {
        let current = self
            .persistence
            .current_routes_state_locked(guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        if current_matches_record_previous(&current, record) {
            self.abort_without_yaml_change(guard, record.tx_id(), "prepared_without_yaml_replace")
        } else if current_matches_record_new(&current, record) {
            self.rollback_yaml_and_abort(
                guard,
                record.tx_id(),
                &receipt_from_record(record),
                "yaml_replaced_before_phase_record",
            )
        } else {
            Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                format!(
                    "Prepared transaction '{}' has an ambiguous YAML mirror during startup recovery",
                    record.tx_id()
                ),
            ))
        }
    }

    fn complete_recovered_commit(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
        active_revision: Option<&RouteConfigRedisRevision>,
        phase: TransactionPhase,
    ) -> Result<(), RouteConfigRuntimeError> {
        let (validated, metadata) = if let Some(active_revision) = active_revision {
            validated_and_metadata_from_active(active_revision, record)?
        } else {
            self.validated_and_metadata_from_archive(record)?
        };
        match phase {
            TransactionPhase::Prepared => {
                let current = self
                    .persistence
                    .current_routes_state_locked(guard)
                    .map_err(RouteConfigRuntimeError::from_persistence)?;
                let receipt = if current_matches_record_new(&current, record) {
                    receipt_from_record(record)
                } else if current_matches_record_previous(&current, record) {
                    self.persistence
                        .replace_routes_yaml_for_transaction_locked(
                            guard,
                            record,
                            validated.canonical_yaml(),
                        )
                        .map_err(RouteConfigRuntimeError::from_persistence)?
                } else {
                    return Err(RouteConfigRuntimeError::new(
                        "console_recovery_required",
                        format!(
                            "Prepared transaction '{}' does not match either the previous or new YAML during startup recovery",
                            record.tx_id()
                        ),
                    ));
                };
                self.journal
                    .record_yaml_replaced_locked(
                        guard,
                        record.tx_id(),
                        &receipt,
                        OffsetDateTime::now_utc(),
                    )
                    .map_err(RouteConfigRuntimeError::from_persistence)?;
                self.journal
                    .transition_locked(
                        guard,
                        record.tx_id(),
                        TransactionPhase::RedisActivated,
                        OffsetDateTime::now_utc(),
                        None,
                    )
                    .map_err(RouteConfigRuntimeError::from_persistence)?;
            }
            TransactionPhase::YamlReplaced
            | TransactionPhase::RedisActivated
            | TransactionPhase::Committed => {
                self.ensure_yaml_matches_revision(guard, record, validated.canonical_yaml())?;
                if phase == TransactionPhase::YamlReplaced {
                    self.journal
                        .transition_locked(
                            guard,
                            record.tx_id(),
                            TransactionPhase::RedisActivated,
                            OffsetDateTime::now_utc(),
                            None,
                        )
                        .map_err(RouteConfigRuntimeError::from_persistence)?;
                }
            }
            TransactionPhase::Aborted => {}
        }

        self.install_snapshot_if_needed(metadata, validated)?;
        if matches!(
            phase,
            TransactionPhase::Prepared
                | TransactionPhase::YamlReplaced
                | TransactionPhase::RedisActivated
        ) {
            self.journal
                .transition_locked(
                    guard,
                    record.tx_id(),
                    TransactionPhase::Committed,
                    OffsetDateTime::now_utc(),
                    None,
                )
                .map_err(RouteConfigRuntimeError::from_persistence)?;
        }
        Ok(())
    }

    fn install_committed_local_revision(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRuntimeError> {
        let (validated, metadata) = self.validated_and_metadata_from_archive(record)?;
        self.ensure_yaml_matches_revision(guard, record, validated.canonical_yaml())?;
        self.install_snapshot_if_needed(metadata, validated)
    }
}
