//! Activation outcomes preserve durable phase transitions, rollback and snapshot publication order.

use super::{RouteConfigCoordinator, RouteConfigRuntimeError};
use crate::console::persistence::TransactionPhase;
use crate::console::revision::RevisionMetadata;
use crate::routing::config::RouteConfigSnapshot;
use std::sync::Arc;
use time::OffsetDateTime;

impl RouteConfigCoordinator {
    pub(super) fn finish_commit(
        &self,
        guard: &crate::console::WriterLockGuard,
        tx_id: &str,
        validated: crate::console::document::ValidatedRouteDocument,
        revision: RevisionMetadata,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        self.journal
            .transition_locked(
                guard,
                tx_id,
                TransactionPhase::RedisActivated,
                OffsetDateTime::now_utc(),
                None,
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let snapshot = self
            .route_config
            .install_external_validated(validated, revision, self.redis.active_source())
            .map_err(RouteConfigRuntimeError::from_replace)?;
        self.journal
            .transition_locked(
                guard,
                tx_id,
                TransactionPhase::Committed,
                OffsetDateTime::now_utc(),
                None,
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        Ok(snapshot)
    }

    pub(super) async fn resolve_indeterminate_activation(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &crate::console::TransactionRecord,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        self.resolve_failed_activation(
            guard,
            record,
            RouteConfigRuntimeError::indeterminate(
                "Gateway console indeterminate activation was fenced and the transaction revoked",
            ),
            None,
        )
        .await
    }

    pub(super) async fn resolve_failed_activation(
        &self,
        guard: &crate::console::WriterLockGuard,
        record: &crate::console::TransactionRecord,
        failure: RouteConfigRuntimeError,
        rollback_code: Option<&str>,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        let (outcome, active) = self.fence_and_reload(guard, record).await?;
        if outcome == super::RouteConfigRedisSettlement::Activated {
            let active = active.as_ref().ok_or_else(|| {
                RouteConfigRuntimeError::new(
                    "console_recovery_required",
                    "Activated transaction lost its authority",
                )
            })?;
            let (validated, metadata) =
                super::revision_state::validated_and_metadata_from_active(active, record)?;
            self.finish_commit(guard, record.tx_id(), validated, metadata)
        } else {
            let code = rollback_code.unwrap_or(if active.is_none() {
                "redis_indeterminate_no_active_revision"
            } else {
                "redis_indeterminate_rolled_back"
            });
            self.apply_settlement(guard, record, outcome, active.as_ref(), code)?;
            Err(failure)
        }
    }

    pub(super) fn abort_without_yaml_change(
        &self,
        guard: &crate::console::WriterLockGuard,
        tx_id: &str,
        code: &str,
    ) -> Result<(), RouteConfigRuntimeError> {
        self.journal
            .transition_locked(
                guard,
                tx_id,
                TransactionPhase::Aborted,
                OffsetDateTime::now_utc(),
                Some(code),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        Ok(())
    }

    pub(super) fn rollback_yaml_and_abort(
        &self,
        guard: &crate::console::WriterLockGuard,
        tx_id: &str,
        receipt: &crate::console::YamlReplaceReceipt,
        code: &str,
    ) -> Result<(), RouteConfigRuntimeError> {
        self.persistence
            .restore_routes_yaml_locked(guard, receipt)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        self.journal
            .transition_locked(
                guard,
                tx_id,
                TransactionPhase::Aborted,
                OffsetDateTime::now_utc(),
                Some(code),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        Ok(())
    }
}
