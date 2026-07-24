use std::sync::Arc;

use async_trait::async_trait;
use deadpool_redis::Pool;
use time::OffsetDateTime;
use uuid::Uuid;

use super::document::{canonicalize_route_document, validate_route_document};
use super::journal::TransactionJournal;
use super::persistence::{
    PersistenceError, RouteConfigPersistence, TransactionPhase, TransactionRecord,
};
use super::redis_store::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisRevision, RouteConfigRedisStore,
    RouteConfigRedisStoreError,
};
use super::revision::{RevisionActor, RevisionMetadata};
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

    pub async fn commit_document(
        &self,
        expected_revision: &str,
        document: RouteConfigYaml,
        message: Option<String>,
    ) -> Result<Arc<RouteConfigSnapshot>, RouteConfigRuntimeError> {
        let validated = validate_route_document(document).map_err(|diagnostics| {
            RouteConfigRuntimeError::new("console_route_validation_failed", diagnostics.to_string())
        })?;
        let current = self.route_config.snapshot();
        if current.revision().id() != expected_revision {
            return Err(RouteConfigRuntimeError::revision_conflict(format!(
                "Gateway console expected revision '{}' but active revision is '{}'",
                expected_revision,
                current.revision().id()
            )));
        }
        let revision = RevisionMetadata::from_validated(
            current.revision().sequence().saturating_add(1),
            Some(current.revision().id().to_string()),
            RevisionActor::ManagementToken,
            OffsetDateTime::now_utc(),
            message,
            &validated,
        );
        let canonical = canonicalize_route_document(validated.document()).map_err(|error| {
            RouteConfigRuntimeError::new("console_route_compilation_failed", error.to_string())
        })?;
        let redis_revision =
            RouteConfigRedisRevision::new(revision.clone(), validated.document().clone())
                .map_err(RouteConfigRuntimeError::from_redis)?;

        let guard = self
            .persistence
            .try_writer_lock()
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let locked_current = self.route_config.snapshot();
        if locked_current.revision().id() != expected_revision {
            return Err(RouteConfigRuntimeError::revision_conflict(format!(
                "Gateway console expected revision '{}' but active revision is '{}'",
                expected_revision,
                locked_current.revision().id()
            )));
        }

        self.persistence
            .archive_revision_locked(&guard, &revision, &canonical)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let (previous_yaml_present, previous_yaml_digest) = self
            .persistence
            .current_routes_state_locked(&guard)
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let tx_id = format!("tx-{}", Uuid::new_v4());
        let tx_path = format!("revisions/{}", revision.id());
        let prepared = TransactionRecord::new_prepared(
            tx_id,
            Some(expected_revision),
            revision.id(),
            tx_path,
            previous_yaml_present,
            previous_yaml_digest.as_deref(),
            validated.yaml_digest(),
            OffsetDateTime::now_utc(),
        )
        .map_err(RouteConfigRuntimeError::from_persistence)?;
        self.journal
            .persist_locked(&guard, &prepared)
            .map_err(RouteConfigRuntimeError::from_persistence)?;

        if let Err(error) = self.redis.store_revision(&redis_revision).await {
            self.abort_without_yaml_change(
                &guard,
                prepared.tx_id(),
                "redis_revision_store_failed",
            )?;
            return Err(RouteConfigRuntimeError::from_redis(error));
        }
        if let Err(error) = self.redis.store_prepared_transaction(&prepared).await {
            self.abort_without_yaml_change(&guard, prepared.tx_id(), "redis_prepare_store_failed")?;
            return Err(RouteConfigRuntimeError::from_redis(error));
        }

        let receipt = self
            .persistence
            .replace_routes_yaml_for_transaction_locked(
                &guard,
                &prepared,
                validated.canonical_yaml(),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let yaml_replaced = self
            .journal
            .record_yaml_replaced_locked(
                &guard,
                prepared.tx_id(),
                &receipt,
                OffsetDateTime::now_utc(),
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;
        let mut activated = yaml_replaced.clone();
        activated
            .transition(
                TransactionPhase::RedisActivated,
                OffsetDateTime::now_utc(),
                None,
            )
            .map_err(RouteConfigRuntimeError::from_persistence)?;

        match self
            .redis
            .activate_revision(
                Some(expected_revision),
                &redis_revision,
                &prepared,
                &activated,
            )
            .await
            .map_err(RouteConfigRuntimeError::from_redis)?
        {
            RouteConfigRedisActivationOutcome::Activated
            | RouteConfigRedisActivationOutcome::AlreadyActive => {
                self.finish_commit(&guard, prepared.tx_id(), validated, revision)
            }
            RouteConfigRedisActivationOutcome::RevisionConflict { actual } => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_revision_conflict",
                )?;
                Err(RouteConfigRuntimeError::revision_conflict(format!(
                    "Gateway console Redis active revision conflict: expected '{}', actual '{}'",
                    expected_revision,
                    actual.unwrap_or_else(|| "<none>".to_string())
                )))
            }
            RouteConfigRedisActivationOutcome::PreparedTransactionMismatch => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_prepared_transaction_mismatch",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_invalid_state",
                    "Gateway console Redis prepared transaction did not match the local transaction record",
                ))
            }
            RouteConfigRedisActivationOutcome::RevisionPayloadMismatch => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_revision_payload_mismatch",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_invalid_state",
                    "Gateway console Redis immutable revision payload did not match the candidate revision",
                ))
            }
            RouteConfigRedisActivationOutcome::Unavailable => {
                self.rollback_yaml_and_abort(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    "redis_unavailable",
                )?;
                Err(RouteConfigRuntimeError::new(
                    "console_redis_unavailable",
                    "Gateway console Redis CAS could not start because Redis was unavailable",
                ))
            }
            RouteConfigRedisActivationOutcome::Indeterminate => {
                self.resolve_indeterminate_activation(
                    &guard,
                    prepared.tx_id(),
                    &receipt,
                    expected_revision,
                    &revision,
                    validated,
                )
                .await
            }
        }
    }

    fn finish_commit(
        &self,
        guard: &super::WriterLockGuard,
        tx_id: &str,
        validated: super::document::ValidatedRouteDocument,
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
            .install_external_validated(validated, revision, ActiveConfigSource::Redis)
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

    async fn resolve_indeterminate_activation(
        &self,
        guard: &super::WriterLockGuard,
        tx_id: &str,
        receipt: &super::YamlReplaceReceipt,
        expected_revision: &str,
        revision: &RevisionMetadata,
        validated: super::document::ValidatedRouteDocument,
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

    fn abort_without_yaml_change(
        &self,
        guard: &super::WriterLockGuard,
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

    fn rollback_yaml_and_abort(
        &self,
        guard: &super::WriterLockGuard,
        tx_id: &str,
        receipt: &super::YamlReplaceReceipt,
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

    pub async fn reconcile_from_redis(
        &self,
    ) -> Result<Option<Arc<RouteConfigSnapshot>>, RouteConfigRuntimeError> {
        self.replica.reconcile_active_revision().await
    }
}
