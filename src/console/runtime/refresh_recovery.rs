//! Worker recovery is restricted to durably admitted refresh transactions, not ambient journal writes.
use super::*;
use crate::console::{revision::RevisionActor, TransactionPhase};

pub(super) fn is_pending(phase: TransactionPhase) -> bool {
    matches!(
        phase,
        TransactionPhase::Prepared
            | TransactionPhase::YamlReplaced
            | TransactionPhase::RedisActivated
    )
}

impl RouteConfigCoordinator {
    pub(super) async fn recover_refresh_only(&self) -> Result<(), RouteConfigRuntimeError> {
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        // Do not repair an unknown journal tail before checking the complete admission set.
        let records = self
            .journal
            .inspect_read_only_locked(&guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let pending: Vec<_> = records
            .iter()
            .filter(|record| is_pending(record.phase()))
            .collect();
        for record in &pending {
            self.validate_refresh_intent_locked(&guard, record)?;
        }
        if self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
            .is_none()
        {
            return Err(restricted_recovery(
                "Worker refresh requires existing configuration authority",
            ));
        }
        for record in pending {
            self.settle_pending_transaction(&guard, record).await?;
        }
        let active = self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
            .ok_or_else(|| {
                restricted_recovery("Worker refresh requires existing configuration authority")
            })?;
        // This is the existing replica-like memory adoption, never local Committed bootstrap or arbitrary YAML repair.
        self.ensure_store_matches_revision(&active)
    }

    pub(super) fn validate_refresh_intent_locked(
        &self,
        guard: &super::super::WriterLockGuard,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRuntimeError> {
        let intent = self
            .persistence
            .load_refresh_intent_locked(guard, record)
            .map_err(|_| {
                restricted_recovery("Pending transaction has no valid refresh admission proof")
            })?;
        let parent = record
            .old_revision()
            .ok_or_else(|| restricted_recovery("Refresh transaction lacks its parent"))?;
        let parent = self
            .persistence
            .load_revision(parent)
            .map_err(RouteConfigRuntimeError::from_persistence)?
            .ok_or_else(|| restricted_recovery("Refresh parent archive is unavailable"))?;
        let next = self
            .persistence
            .load_revision(record.new_revision())
            .map_err(RouteConfigRuntimeError::from_persistence)?
            .ok_or_else(|| restricted_recovery("Refresh candidate archive is unavailable"))?;
        for archive in [&parent, &next] {
            let validated = validate_route_document(archive.document().clone()).map_err(|_| {
                restricted_recovery("Refresh archive is not a valid route document")
            })?;
            if validated.document_digest() != archive.metadata().document_digest()
                || validated.yaml_digest() != archive.metadata().yaml_digest()
            {
                return Err(restricted_recovery("Refresh archive is not canonical"));
            }
        }
        if parent.metadata().document_digest() != intent.parent_document_digest
            || next.metadata().document_digest() != intent.new_document_digest
            || next.metadata().parent() != Some(parent.metadata().id())
            || next.metadata().actor() != RevisionActor::CredentialPoolAutomation
            || next.metadata().sequence() != parent.metadata().sequence().saturating_add(1)
            || !crate::token_refresh::is_refresh_projection(
                parent.document(),
                next.document(),
                &intent.provider,
                &intent.credential,
            )
        {
            return Err(restricted_recovery(
                "Refresh admission and archived credential projection differ",
            ));
        }
        Ok(())
    }
}

fn restricted_recovery(message: &'static str) -> RouteConfigRuntimeError {
    RouteConfigRuntimeError::new("console_recovery_required", message)
}
