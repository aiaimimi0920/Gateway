//! Reconciliation first closes the remote gate, then re-reads authority.
//! A revoked transaction can share a revision with a successful successor;
//! repair its mirror from authority, but never credit the old transaction.
use super::revision_state::{
    redis_kept_previous_revision, redis_revision_matches_record, validated_and_metadata_from_active,
};
use super::*;
use crate::console::TransactionPhase;

impl RouteConfigCoordinator {
    pub(super) async fn settle_pending_transaction(
        &self,
        guard: &super::super::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<RouteConfigRedisSettlement, RouteConfigRuntimeError> {
        let (outcome, active) = self.fence_and_reload(guard, record).await?;
        self.apply_settlement(
            guard,
            record,
            outcome,
            active.as_ref(),
            "yaml_replaced_without_redis_activation",
        )?;
        Ok(outcome)
    }

    pub(super) async fn fence_and_reload(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<
        (RouteConfigRedisSettlement, Option<RouteConfigRedisRevision>),
        RouteConfigRuntimeError,
    > {
        let outcome = self
            .redis
            .settle_transaction_locked(guard, record)
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?;
        let active = self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?;
        Ok((outcome, active))
    }

    pub(super) fn apply_settlement(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &TransactionRecord,
        outcome: RouteConfigRedisSettlement,
        active: Option<&RouteConfigRedisRevision>,
        rollback_code: &str,
    ) -> Result<(), RouteConfigRuntimeError> {
        if redis_revision_matches_record(active, record)? {
            if outcome == RouteConfigRedisSettlement::Activated {
                self.complete_recovered_commit(guard, record, active, record.phase())?;
            } else if matches!(
                record.phase(),
                TransactionPhase::Prepared | TransactionPhase::YamlReplaced
            ) {
                let (validated, metadata) =
                    validated_and_metadata_from_active(active.unwrap(), record)?;
                self.ensure_yaml_matches_revision(guard, record, validated.canonical_yaml())?;
                self.install_snapshot_if_needed(metadata, validated)?;
                self.abort_without_yaml_change(guard, record.tx_id(), "redis_transaction_revoked")?;
            } else {
                return Err(unsettled());
            }
        } else if outcome == RouteConfigRedisSettlement::Revoked
            && redis_kept_previous_revision(active, record)
        {
            match record.phase() {
                TransactionPhase::Prepared => self.abort_prepared_without_cas(guard, record)?,
                TransactionPhase::YamlReplaced => self.rollback_yaml_and_abort(
                    guard,
                    record.tx_id(),
                    &super::revision_state::receipt_from_record(record),
                    rollback_code,
                )?,
                _ => return Err(unsettled()),
            }
        } else {
            return Err(unsettled());
        }
        Ok(())
    }
}

fn unsettled() -> RouteConfigRuntimeError {
    RouteConfigRuntimeError::new(
        "console_recovery_required",
        "Gateway console transaction fence and current authority have not converged",
    )
}
