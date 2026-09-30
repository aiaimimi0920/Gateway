//! File-backed authority for single-node installations without Redis.
//! The coordinator still owns the writer lock, immutable archives and recovery journal.
use super::{RouteConfigRedisBackend, RouteConfigRedisRevision, RouteConfigRedisStoreError};
use crate::console::persistence::{PersistenceError, RouteConfigPersistence, TransactionRecord};
use crate::console::redis_store::RouteConfigRedisActivationOutcome as Outcome;
use crate::console::WriterLockGuard;
use crate::routing::config::ActiveConfigSource;
use async_trait::async_trait;

pub(super) struct LocalRouteConfigBackend {
    persistence: RouteConfigPersistence,
}

impl LocalRouteConfigBackend {
    pub(super) fn new(persistence: RouteConfigPersistence) -> Self {
        Self { persistence }
    }

    pub(super) fn load_store(
        &self,
    ) -> Result<crate::routing::config::RouteConfigStore, RouteConfigRedisStoreError> {
        let id = self
            .persistence
            .local_active_revision()
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .ok_or_else(|| {
                RouteConfigRedisStoreError::from_persistence(PersistenceError::recovery_required(
                    "Local authority is missing",
                ))
            })?;
        self.revision(&id)?
            .into_store(ActiveConfigSource::Recovered)
    }

    pub(super) fn initialize(
        &self,
        store: &crate::routing::config::RouteConfigStore,
    ) -> Result<(), RouteConfigRedisStoreError> {
        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        if self
            .persistence
            .local_active_revision()
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .is_some()
        {
            return Ok(());
        }
        // Establish authority before any local transaction can replace YAML.
        // This makes first-commit crash recovery identical to later commits.
        if !crate::console::journal::TransactionJournal::new(self.persistence.clone())
            .load_transactions()
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .is_empty()
        {
            return Err(RouteConfigRedisStoreError::from_persistence(
                PersistenceError::recovery_required("Missing local authority with an existing journal; storage modes cannot be switched in place")));
        }
        let snapshot = store.snapshot();
        let canonical = crate::console::document::canonicalize_route_document(snapshot.document())
            .map_err(|error| {
                RouteConfigRedisStoreError::from_persistence(PersistenceError::corruption(
                    error.to_string(),
                ))
            })?;
        if self
            .persistence
            .load_revision(snapshot.revision().id())
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .is_none()
        {
            self.persistence
                .archive_revision_locked(&guard, snapshot.revision(), &canonical)
                .map_err(RouteConfigRedisStoreError::from_persistence)?;
        }
        self.revision(snapshot.revision().id())?;
        self.persistence
            .write_local_active_locked(&guard, snapshot.revision().id())
            .map_err(RouteConfigRedisStoreError::from_persistence)
    }

    fn revision(&self, id: &str) -> Result<RouteConfigRedisRevision, RouteConfigRedisStoreError> {
        let stored = self
            .persistence
            .load_revision(id)
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .ok_or_else(|| {
                RouteConfigRedisStoreError::from_persistence(PersistenceError::corruption(
                    "Local active revision archive is missing",
                ))
            })?;
        RouteConfigRedisRevision::new(stored.metadata().clone(), stored.document().clone())
    }
}

#[async_trait]
impl RouteConfigRedisBackend for LocalRouteConfigBackend {
    fn active_source(&self) -> ActiveConfigSource {
        ActiveConfigSource::Recovered
    }

    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        // The coordinator archived and synced this exact document before calling the backend.
        let archived = self.revision(revision.metadata().id())?;
        if archived.to_json()? != revision.to_json()? {
            return Err(RouteConfigRedisStoreError::from_persistence(
                PersistenceError::corruption("Local revision archive does not match the candidate"),
            ));
        }
        Ok(())
    }

    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        let archived = crate::console::persistence::read_transaction_json(
            &self
                .persistence
                .transactions_dir()
                .join(format!("{}.json", record.tx_id())),
        )
        .map_err(RouteConfigRedisStoreError::from_persistence)?;
        if archived != *record {
            return Err(RouteConfigRedisStoreError::from_persistence(
                PersistenceError::corruption(
                    "Local prepared transaction does not match the journal",
                ),
            ));
        }
        Ok(())
    }

    async fn activate_revision(
        &self,
        _expected: Option<&str>,
        _revision: &RouteConfigRedisRevision,
        _prepared: &TransactionRecord,
        _activated: &TransactionRecord,
    ) -> Result<Outcome, RouteConfigRedisStoreError> {
        Err(RouteConfigRedisStoreError::from_persistence(
            PersistenceError::corruption("Local activation requires the coordinator writer guard"),
        ))
    }

    async fn activate_revision_locked(
        &self,
        guard: &WriterLockGuard,
        expected: Option<&str>,
        revision: &RouteConfigRedisRevision,
        _prepared: &TransactionRecord,
        _activated: &TransactionRecord,
    ) -> Result<Outcome, RouteConfigRedisStoreError> {
        let active = self
            .persistence
            .local_active_revision()
            .map_err(RouteConfigRedisStoreError::from_persistence)?;
        if active.as_deref() == Some(revision.metadata().id()) {
            return Ok(Outcome::AlreadyActive);
        }
        if active.as_deref() != expected {
            return Ok(Outcome::RevisionConflict { actual: active });
        }
        // The legacy RedisActivated journal phase means authority was published;
        // here that authority is an atomically replaced local revision pointer.
        match self
            .persistence
            .write_local_active_locked(guard, revision.metadata().id())
        {
            Ok(()) => Ok(Outcome::Activated),
            Err(_) => Ok(Outcome::Indeterminate),
        }
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        let id = self
            .persistence
            .local_active_revision()
            .map_err(RouteConfigRedisStoreError::from_persistence)?
            .ok_or_else(|| {
                RouteConfigRedisStoreError::from_persistence(PersistenceError::recovery_required(
                    "Local authority disappeared after initialization",
                ))
            })?;
        self.revision(&id).map(Some)
    }
}
