//! Public runtime roles, Redis backend contracts and shared coordinator/replica ownership.

mod activation;
mod commit;
mod recovery;
mod revision_state;
mod snapshots;

use std::sync::Arc;

use async_trait::async_trait;
use deadpool_redis::Pool;

use super::document::validate_route_document;
use super::journal::TransactionJournal;
use super::persistence::{
    PersistenceError, RouteConfigPersistence, StoredRouteRevision, TransactionRecord,
};
use super::redis_store::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisRevision, RouteConfigRedisStore,
    RouteConfigRedisStoreError,
};
use super::revision::RevisionActor;
use super::ConsoleConfig;
use crate::routing::config::{
    ActiveConfigSource, RouteConfigReplaceError, RouteConfigSnapshot, RouteConfigStore,
    RouteConfigYaml,
};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct RouteConfigRuntimeError {
    code: &'static str,
    message: String,
}

impl RouteConfigRuntimeError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_persistence(error: PersistenceError) -> Self {
        Self::new(error.code(), error.to_string())
    }

    fn from_redis(error: RouteConfigRedisStoreError) -> Self {
        Self::new(error.code(), error.to_string())
    }

    fn from_replace(error: RouteConfigReplaceError) -> Self {
        Self::new(error.code(), error.to_string())
    }

    fn revision_conflict(message: impl Into<String>) -> Self {
        Self::new("console_revision_conflict", message)
    }

    fn mutation_not_supported() -> Self {
        Self::new(
            "console_mutation_not_supported",
            "This Gateway console runtime role does not own route mutations",
        )
    }

    fn indeterminate(message: impl Into<String>) -> Self {
        Self::new("console_redis_indeterminate", message)
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

#[async_trait]
pub trait RouteConfigRedisBackend: Send + Sync {
    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError>;

    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError>;

    async fn activate_revision(
        &self,
        expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        prepared_record: &TransactionRecord,
        activated_record: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError>;

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError>;
}

#[derive(Clone, Debug)]
pub struct PooledRouteConfigRedisBackend {
    pool: Pool,
    store: RouteConfigRedisStore,
}

impl PooledRouteConfigRedisBackend {
    pub fn new(pool: Pool, store: RouteConfigRedisStore) -> Self {
        Self { pool, store }
    }
}

#[async_trait]
impl RouteConfigRedisBackend for PooledRouteConfigRedisBackend {
    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.store.store_revision(&self.pool, revision).await
    }

    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.store
            .store_prepared_transaction(&self.pool, record)
            .await
    }

    async fn activate_revision(
        &self,
        expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        prepared_record: &TransactionRecord,
        activated_record: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.store
            .activate_revision(
                &self.pool,
                expected_active_revision,
                revision,
                prepared_record,
                activated_record,
            )
            .await
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        self.store.load_active_revision(&self.pool).await
    }
}

#[derive(Clone)]
pub struct RouteConfigCoordinator {
    route_config: Arc<RouteConfigStore>,
    persistence: RouteConfigPersistence,
    journal: TransactionJournal,
    redis: Arc<dyn RouteConfigRedisBackend>,
}

impl std::fmt::Debug for RouteConfigCoordinator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RouteConfigCoordinator")
            .field(
                "route_config",
                &self.route_config.snapshot().revision().id(),
            )
            .field("state_root", &self.persistence.state_root())
            .finish_non_exhaustive()
    }
}

impl RouteConfigCoordinator {
    pub fn route_config(&self) -> &Arc<RouteConfigStore> {
        &self.route_config
    }

    pub fn load_revisions(&self) -> Result<Vec<StoredRouteRevision>, RouteConfigRuntimeError> {
        self.persistence
            .load_revisions()
            .map_err(RouteConfigRuntimeError::from_persistence)
    }

    pub fn load_revision(
        &self,
        revision_id: &str,
    ) -> Result<Option<StoredRouteRevision>, RouteConfigRuntimeError> {
        self.persistence
            .load_revision(revision_id)
            .map_err(RouteConfigRuntimeError::from_persistence)
    }
}

#[derive(Clone)]
pub struct RouteConfigReplica {
    route_config: Arc<RouteConfigStore>,
    redis: Arc<dyn RouteConfigRedisBackend>,
}

impl std::fmt::Debug for RouteConfigReplica {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RouteConfigReplica")
            .field(
                "route_config",
                &self.route_config.snapshot().revision().id(),
            )
            .finish_non_exhaustive()
    }
}

impl RouteConfigReplica {
    pub fn route_config(&self) -> &Arc<RouteConfigStore> {
        &self.route_config
    }

    pub async fn reconcile_active_revision(
        &self,
    ) -> Result<Option<Arc<RouteConfigSnapshot>>, RouteConfigRuntimeError> {
        let Some(active) = self
            .redis
            .load_active_revision()
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
        else {
            return Ok(None);
        };
        let current = self.route_config.snapshot();
        if active.metadata().id() == current.revision().id() {
            return Ok(None);
        }
        let validated =
            validate_route_document(active.document().clone()).map_err(|diagnostics| {
                RouteConfigRuntimeError::new(
                    "console_route_validation_failed",
                    diagnostics.to_string(),
                )
            })?;
        let snapshot = self
            .route_config
            .install_external_validated(
                validated,
                active.metadata().clone(),
                ActiveConfigSource::Redis,
            )
            .map_err(RouteConfigRuntimeError::from_replace)?;
        Ok(Some(snapshot))
    }
}

#[derive(Clone)]
pub struct RouteConfigRuntime {
    route_config: Arc<RouteConfigStore>,
    coordinator: Option<RouteConfigCoordinator>,
    replica: RouteConfigReplica,
}

impl std::fmt::Debug for RouteConfigRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RouteConfigRuntime")
            .field(
                "active_revision",
                &self.route_config.snapshot().revision().id().to_string(),
            )
            .field("has_coordinator", &self.coordinator.is_some())
            .finish_non_exhaustive()
    }
}

impl RouteConfigRuntime {
    pub fn new(
        route_config: Arc<RouteConfigStore>,
        console: &ConsoleConfig,
        redis_pool: Pool,
        writable: bool,
    ) -> Result<Self, RouteConfigRuntimeError> {
        let persistence = RouteConfigPersistence::new(&console.state_dir, &console.routes_file)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let redis_store = RouteConfigRedisStore::new(console.redis_namespace.clone())
            .map_err(RouteConfigRuntimeError::from_redis)?;
        let redis: Arc<dyn RouteConfigRedisBackend> =
            Arc::new(PooledRouteConfigRedisBackend::new(redis_pool, redis_store));
        Ok(Self::with_backend(
            route_config,
            persistence,
            redis,
            writable,
        ))
    }

    pub fn with_backend(
        route_config: Arc<RouteConfigStore>,
        persistence: RouteConfigPersistence,
        redis: Arc<dyn RouteConfigRedisBackend>,
        writable: bool,
    ) -> Self {
        let journal = TransactionJournal::new(persistence.clone());
        let coordinator = writable.then(|| RouteConfigCoordinator {
            route_config: Arc::clone(&route_config),
            persistence,
            journal,
            redis: Arc::clone(&redis),
        });
        let replica = RouteConfigReplica {
            route_config: Arc::clone(&route_config),
            redis,
        };
        Self {
            route_config,
            coordinator,
            replica,
        }
    }

    pub fn route_config(&self) -> &Arc<RouteConfigStore> {
        &self.route_config
    }

    pub fn coordinator(&self) -> Option<&RouteConfigCoordinator> {
        self.coordinator.as_ref()
    }

    pub fn replica(&self) -> &RouteConfigReplica {
        &self.replica
    }

    pub async fn recover_startup(&self) -> Result<(), RouteConfigRuntimeError> {
        let Some(coordinator) = &self.coordinator else {
            return Ok(());
        };
        coordinator.recover_startup().await
    }

    pub async fn commit_document(
        &self,
        expected_revision: &str,
        document: RouteConfigYaml,
        message: Option<String>,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        let Some(coordinator) = &self.coordinator else {
            return Err(RouteConfigRuntimeError::mutation_not_supported());
        };
        coordinator
            .commit_document(expected_revision, document, message)
            .await
    }

    pub async fn commit_automation_document(
        &self,
        expected_revision: &str,
        document: RouteConfigYaml,
        message: Option<String>,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        let Some(coordinator) = &self.coordinator else {
            return Err(RouteConfigRuntimeError::mutation_not_supported());
        };
        coordinator
            .commit_document_as(
                expected_revision,
                document,
                message,
                RevisionActor::CredentialPoolAutomation,
            )
            .await
    }

    pub async fn reconcile_from_redis(
        &self,
    ) -> Result<Option<Arc<RouteConfigSnapshot>>, RouteConfigRuntimeError> {
        self.replica.reconcile_active_revision().await
    }
}
