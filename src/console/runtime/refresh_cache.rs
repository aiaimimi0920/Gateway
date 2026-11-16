//! Recover only an explicitly admitted publication of the currently active revision.
use super::*;
use crate::console::TransactionPhase;

impl RouteConfigRuntime {
    pub(crate) async fn repair_refresh_cache(&self) -> Result<(), RouteConfigRuntimeError> {
        let owner = &self.refresh_coordinator;
        let guard = owner
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let snapshot = self.route_config.snapshot();
        let revision =
            RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
                .map_err(RouteConfigRuntimeError::from_redis)?;
        let publications = owner
            .persistence
            .load_refresh_publications_locked(&guard, revision.metadata().id())
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        for publication in publications {
            let intent = &publication.intent;
            let prepared = &intent.prepared;
            let path = owner
                .persistence
                .transaction_path(prepared.tx_id())
                .map_err(RouteConfigRuntimeError::from_persistence)?;
            if !path.try_exists().map_err(|_| unavailable())? {
                continue; // Orphan admission is not an executable transaction.
            }
            let transaction = owner
                .journal
                .load_transaction(prepared.tx_id())
                .map_err(RouteConfigRuntimeError::from_persistence)?;
            if transaction
                .prepared_identity()
                .map_err(RouteConfigRuntimeError::from_persistence)?
                != *prepared
            {
                return Err(unavailable());
            }
            if transaction.phase() == TransactionPhase::Aborted {
                continue;
            }
            if transaction.phase() != TransactionPhase::Committed {
                return Err(unavailable());
            }
            let actual_intent = owner
                .persistence
                .load_refresh_intent_locked(&guard, prepared)
                .map_err(RouteConfigRuntimeError::from_persistence)?;
            if actual_intent != *intent {
                return Err(unavailable());
            }
            owner.validate_refresh_intent_locked(&guard, prepared)?;
            let next = owner
                .persistence
                .load_revision(prepared.new_revision())
                .map_err(RouteConfigRuntimeError::from_persistence)?
                .ok_or_else(unavailable)?;
            let next =
                RouteConfigRedisRevision::new(next.metadata().clone(), next.document().clone())
                    .map_err(RouteConfigRuntimeError::from_redis)?;
            if !crate::console::refresh_grants::same_authority_document(&next, &revision)
                .map_err(RouteConfigRuntimeError::from_persistence)?
            {
                return Err(unavailable());
            }
            let parent = owner
                .persistence
                .load_revision(prepared.old_revision().ok_or_else(unavailable)?)
                .map_err(RouteConfigRuntimeError::from_persistence)?
                .ok_or_else(unavailable)?;
            let parent =
                RouteConfigRedisRevision::new(parent.metadata().clone(), parent.document().clone())
                    .map_err(RouteConfigRuntimeError::from_redis)?;
            let record = owner
                .redis
                .load_refresh_grant_locked(&guard, &publication.grant)
                .await
                .map_err(RouteConfigRuntimeError::from_redis)?
                .ok_or_else(unavailable)?;
            if !publication.grant.matches(&record)
                || !record.matches_publication(
                    &parent,
                    &revision,
                    &intent.provider,
                    &intent.credential,
                )
            {
                return Err(unavailable());
            }
            if owner
                .redis
                .settle_transaction_locked(&guard, &transaction)
                .await
                .map_err(RouteConfigRuntimeError::from_redis)?
                != RouteConfigRedisSettlement::Activated
            {
                return Err(unavailable());
            }
            let record = owner
                .redis
                .mark_refresh_grant_published_locked(&guard, &revision, &record)
                .await
                .map_err(RouteConfigRuntimeError::from_redis)?;
            // Explicit read-only Luma accounts have no inference credential cache to repair.
            // The transaction, finite projection and permanent grant were verified above.
            if record.original().luma_oauth.is_some() {
                continue;
            }
            owner
                .redis
                .repair_refresh_grant_cache(&revision, &record, prepared)
                .await
                .map_err(RouteConfigRuntimeError::from_redis)?;
        }
        Ok(())
    }
}

fn unavailable() -> RouteConfigRuntimeError {
    RouteConfigRuntimeError::new(
        "console_recovery_required",
        "OAuth cache publication provenance is unavailable or conflicts with authority",
    )
}
