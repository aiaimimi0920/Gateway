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
        tx_id: &str,
        receipt: &crate::console::YamlReplaceReceipt,
        expected_revision: &str,
        revision: &RevisionMetadata,
        validated: crate::console::document::ValidatedRouteDocument,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        match self.redis.load_active_revision().await {
            Ok(Some(active)) if active.metadata().id() == revision.id() => {
                self.finish_commit(guard, tx_id, validated, revision.clone())
            }
            Ok(Some(active)) if active.metadata().id() == expected_revision => {
                self.rollback_yaml_and_abort(
                    guard,
                    tx_id,
                    receipt,
                    "redis_indeterminate_rolled_back",
                )?;
                Err(RouteConfigRuntimeError::indeterminate(
                    "Gateway console Redis CAS transport became indeterminate and the previous active revision remained authoritative",
                ))
            }
            Ok(None) => {
                self.rollback_yaml_and_abort(
                    guard,
                    tx_id,
                    receipt,
                    "redis_indeterminate_no_active_revision",
                )?;
                Err(RouteConfigRuntimeError::indeterminate(
                    "Gateway console Redis CAS transport became indeterminate and Redis still has no authoritative active revision",
                ))
            }
            Ok(Some(active)) => Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                format!(
                    "Gateway console Redis CAS became indeterminate and Redis now points at unrelated revision '{}'",
                    active.metadata().id()
                ),
            )),
            Err(error) => Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                format!(
                    "Gateway console Redis CAS became indeterminate and the authoritative active revision could not be re-read: {}",
                    error
                ),
            )),
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
