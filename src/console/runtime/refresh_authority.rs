//! Private typed refresh admission reuses the normal writer, never the public mutation capability.
use super::*;
use crate::console::{document::canonicalize_route_document, persistence::RefreshIntent};
use crate::token_refresh::PendingRefresh;

/// Only finite, receipt-backed projections may use the refresh transaction/recovery path.
pub(crate) trait RefreshPublicationTicket: Sync {
    fn project(
        &self,
        snapshot: &crate::routing::config::RouteConfigSnapshot,
    ) -> Result<Option<RouteConfigYaml>, ()>;
    fn identity(&self) -> (&str, &str);
    fn grant_locator(
        &self,
    ) -> Result<crate::console::refresh_grants::GrantLocator, PersistenceError>;
}
impl RefreshPublicationTicket for PendingRefresh {
    fn project(
        &self,
        snapshot: &crate::routing::config::RouteConfigSnapshot,
    ) -> Result<Option<RouteConfigYaml>, ()> {
        self.project(snapshot)
    }
    fn identity(&self) -> (&str, &str) {
        self.identity()
    }
    fn grant_locator(
        &self,
    ) -> Result<crate::console::refresh_grants::GrantLocator, PersistenceError> {
        self.grant_locator()
    }
}

pub(super) struct RefreshAdmission<'a> {
    pub(super) ticket: &'a dyn RefreshPublicationTicket,
    pub(super) require_existing_authority: bool,
}

impl RouteConfigRuntime {
    pub(crate) async fn commit_refresh_ticket(
        &self,
        ticket: &dyn RefreshPublicationTicket,
    ) -> Result<(), RouteConfigRuntimeError> {
        let snapshot = self.route_config.snapshot();
        let Some(document) = ticket
            .project(&snapshot)
            .map_err(|_| invalid_projection())?
        else {
            return Ok(()); // Saved or superseded; callers retain ownership of any cache retry.
        };
        self.refresh_coordinator
            .commit_with_refresh(
                snapshot.revision().id(),
                document,
                Some("Refresh OAuth credential".into()),
                RevisionActor::CredentialPoolAutomation,
                Some(RefreshAdmission {
                    ticket,
                    require_existing_authority: self.coordinator.is_none(),
                }),
            )
            .await
            .map(|_| ())
    }

    pub(crate) async fn recover_refresh_authority(&self) -> Result<(), RouteConfigRuntimeError> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.recover_startup().await
        } else {
            self.refresh_coordinator.recover_refresh_only().await
        }
    }
}

impl RouteConfigCoordinator {
    pub(super) fn record_refresh_intent_locked(
        &self,
        guard: &super::super::WriterLockGuard,
        ticket: &dyn RefreshPublicationTicket,
        record: &TransactionRecord,
        revision: &super::super::revision::RevisionMetadata,
        document: &RouteConfigYaml,
    ) -> Result<(), RouteConfigRuntimeError> {
        let snapshot = self.route_config.snapshot();
        if record.old_revision() != Some(snapshot.revision().id()) {
            return Err(RouteConfigRuntimeError::revision_conflict(
                "Refresh authority changed during admission",
            ));
        }
        let expected = ticket
            .project(&snapshot)
            .map_err(|_| invalid_projection())?
            .ok_or_else(|| {
                RouteConfigRuntimeError::revision_conflict(
                    "Refresh authorization was superseded during admission",
                )
            })?;
        if serde_json::to_value(expected).map_err(|_| invalid_projection())?
            != serde_json::to_value(document).map_err(|_| invalid_projection())?
        {
            return Err(invalid_projection());
        }
        let records = self
            .journal
            .inspect_read_only_locked(guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        if records
            .iter()
            .any(|record| refresh_recovery::is_pending(record.phase()))
        {
            return Err(RouteConfigRuntimeError::new(
                "console_recovery_required",
                "Refresh cannot bypass pending configuration transactions",
            ));
        }
        // Pin the original archive before publishing the provenance or Prepared record.
        match self
            .persistence
            .load_revision(snapshot.revision().id())
            .map_err(RouteConfigRuntimeError::from_persistence)?
        {
            Some(parent)
                if parent.metadata().document_digest() == snapshot.revision().document_digest()
                    && parent.metadata().yaml_digest() == snapshot.revision().yaml_digest() => {}
            Some(_) => return Err(invalid_projection()),
            None => {
                let canonical = canonicalize_route_document(snapshot.document())
                    .map_err(|_| invalid_projection())?;
                self.persistence
                    .archive_revision_locked(guard, snapshot.revision(), &canonical)
                    .map_err(RouteConfigRuntimeError::from_persistence)?;
            }
        }
        let (provider, credential) = ticket.identity();
        let intent = RefreshIntent::new(
            record,
            provider,
            credential,
            snapshot.revision().document_digest(),
            revision.document_digest(),
        )
        .map_err(RouteConfigRuntimeError::from_persistence)?;
        self.persistence
            .store_refresh_intent_locked(guard, &intent)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let grant = ticket
            .grant_locator()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        self.persistence
            .store_refresh_publication_locked(
                guard,
                &crate::console::persistence::RefreshPublication::new(intent, grant),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)
    }
}

fn invalid_projection() -> RouteConfigRuntimeError {
    RouteConfigRuntimeError::new(
        "console_refresh_projection_invalid",
        "Refresh admission does not match its finite credential projection",
    )
}
