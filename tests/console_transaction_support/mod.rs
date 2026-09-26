//! Transaction fixtures own isolated directories, journals and explicit Redis outcomes.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use neuro_gateway::console::document::canonicalize_route_document;
use neuro_gateway::console::journal::TransactionJournal;
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::console::{
    RouteConfigPersistence, RouteConfigRedisActivationOutcome, RouteConfigRedisBackend,
    RouteConfigRedisRevision, RouteConfigRedisStoreError, RouteConfigRuntime, TransactionPhase,
    TransactionRecord,
};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

pub struct RuntimeHarness {
    _temp: TestDirectory,
    pub routes: PathBuf,
    pub store: Arc<RouteConfigStore>,
    pub runtime: RouteConfigRuntime,
    pub backend: FakeRedisBackend,
    pub journal: TransactionJournal,
    pub persistence: RouteConfigPersistence,
}

impl RuntimeHarness {
    pub fn new(
        label: &str,
        document: RouteConfigYaml,
        mode: FakeRedisMode,
        writable: bool,
    ) -> Self {
        let temp = TestDirectory::new(label);
        let routes = temp.path().join("routes.yaml");
        let canonical = canonicalize_route_document(&document).unwrap();
        fs::write(&routes, canonical.canonical_yaml()).unwrap();
        let store = Arc::new(RouteConfigStore::from_document(document).unwrap());
        let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
        let runtime_persistence = persistence.clone();
        let journal = TransactionJournal::new(persistence.clone());
        let backend = FakeRedisBackend::new(mode);
        let runtime = RouteConfigRuntime::with_backend(
            Arc::clone(&store),
            runtime_persistence,
            Arc::new(backend.clone()),
            writable,
        );
        Self {
            _temp: temp,
            routes,
            store,
            runtime,
            backend,
            journal,
            persistence,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FakeRedisBackend {
    state: Arc<Mutex<FakeRedisState>>,
}

impl FakeRedisBackend {
    pub fn new(mode: FakeRedisMode) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeRedisState {
                mode,
                ..FakeRedisState::default()
            })),
        }
    }

    pub fn active_revision_id(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .active_revision
            .as_ref()
            .map(|revision| revision.metadata().id().to_string())
    }

    pub fn set_active_revision(&self, revision: RouteConfigRedisRevision) {
        self.state.lock().unwrap().active_revision = Some(revision);
    }

    pub fn set_mode(&self, mode: FakeRedisMode) {
        self.state.lock().unwrap().mode = mode;
    }
}

#[async_trait]
impl RouteConfigRedisBackend for FakeRedisBackend {
    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.state
            .lock()
            .unwrap()
            .revisions
            .insert(revision.metadata().id().to_string(), revision.clone());
        Ok(())
    }

    async fn store_prepared_transaction(
        &self,
        record: &TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.state
            .lock()
            .unwrap()
            .transactions
            .insert(record.tx_id().to_string(), record.clone());
        Ok(())
    }

    async fn activate_revision(
        &self,
        _expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        _prepared_record: &TransactionRecord,
        activated_record: &TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        let mut state = self.state.lock().unwrap();
        state.transactions.insert(
            activated_record.tx_id().to_string(),
            activated_record.clone(),
        );
        match &state.mode {
            FakeRedisMode::Activated => {
                state.active_revision = Some(revision.clone());
                Ok(RouteConfigRedisActivationOutcome::Activated)
            }
            FakeRedisMode::BootstrapOnly => {
                if _expected_active_revision.is_some() {
                    return Ok(RouteConfigRedisActivationOutcome::RevisionConflict {
                        actual: None,
                    });
                }
                state.active_revision = Some(revision.clone());
                Ok(RouteConfigRedisActivationOutcome::Activated)
            }
            FakeRedisMode::RevisionConflict { actual } => {
                Ok(RouteConfigRedisActivationOutcome::RevisionConflict {
                    actual: actual.clone(),
                })
            }
            FakeRedisMode::IndeterminateNewRevision => {
                state.active_revision = Some(revision.clone());
                Ok(RouteConfigRedisActivationOutcome::Indeterminate)
            }
            FakeRedisMode::IndeterminateOldRevision => {
                Ok(RouteConfigRedisActivationOutcome::Indeterminate)
            }
        }
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        Ok(self.state.lock().unwrap().active_revision.clone())
    }
}

#[derive(Clone, Debug, Default)]
struct FakeRedisState {
    mode: FakeRedisMode,
    revisions: BTreeMap<String, RouteConfigRedisRevision>,
    transactions: BTreeMap<String, TransactionRecord>,
    active_revision: Option<RouteConfigRedisRevision>,
}

#[derive(Clone, Debug, Default)]
pub enum FakeRedisMode {
    #[default]
    Activated,
    BootstrapOnly,
    RevisionConflict {
        actual: Option<String>,
    },
    IndeterminateNewRevision,
    IndeterminateOldRevision,
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gateway-console-transaction-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn document(provider_id: &str, model: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    api_key: test-secret
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider_id}]
aliases:
  answer: {model}
"#
    ))
    .unwrap()
}

pub fn redis_revision_from_document(
    sequence: u64,
    parent: Option<String>,
    document: &RouteConfigYaml,
) -> RouteConfigRedisRevision {
    let validated =
        neuro_gateway::console::document::validate_route_document(document.clone()).unwrap();
    let metadata = RevisionMetadata::from_validated(
        sequence,
        parent,
        RevisionActor::Recovery,
        OffsetDateTime::from_unix_timestamp(1_700_000_100).unwrap(),
        Some("test redis revision".to_string()),
        &validated,
    );
    RouteConfigRedisRevision::new(metadata, document.clone()).unwrap()
}

pub fn redis_revision_with_metadata(
    metadata: RevisionMetadata,
    document: &RouteConfigYaml,
) -> RouteConfigRedisRevision {
    RouteConfigRedisRevision::new(metadata, document.clone()).unwrap()
}

pub struct StagedTransaction {
    pub record: TransactionRecord,
    pub redis_revision: RouteConfigRedisRevision,
}

pub fn stage_transaction(
    harness: &RuntimeHarness,
    document: &RouteConfigYaml,
    phase: TransactionPhase,
) -> StagedTransaction {
    let validated =
        neuro_gateway::console::document::validate_route_document(document.clone()).unwrap();
    let metadata = RevisionMetadata::from_validated(
        harness.store.snapshot().revision().sequence() + 1,
        Some(harness.store.snapshot().revision().id().to_string()),
        RevisionActor::Recovery,
        OffsetDateTime::from_unix_timestamp(1_700_000_200).unwrap(),
        Some("staged recovery".to_string()),
        &validated,
    );
    let canonical = canonicalize_route_document(validated.document()).unwrap();
    harness
        .persistence
        .archive_revision(&metadata, &canonical)
        .unwrap();
    let previous_yaml = fs::read(&harness.routes).unwrap();
    let previous_digest = sha256(&previous_yaml);
    let mut record = TransactionRecord::new_prepared(
        format!("tx-stage-{}", uuid::Uuid::new_v4()),
        Some(harness.store.snapshot().revision().id()),
        metadata.id(),
        format!("revisions/{}", metadata.id()),
        true,
        Some(&previous_digest),
        validated.yaml_digest(),
        OffsetDateTime::from_unix_timestamp(1_700_000_200).unwrap(),
    )
    .unwrap();
    harness.journal.persist(&record).unwrap();
    if matches!(
        phase,
        TransactionPhase::YamlReplaced
            | TransactionPhase::RedisActivated
            | TransactionPhase::Committed
    ) {
        let receipt = harness
            .persistence
            .replace_routes_yaml_for_transaction(&record, validated.canonical_yaml())
            .unwrap();
        record = harness
            .journal
            .record_yaml_replaced(record.tx_id(), &receipt, OffsetDateTime::now_utc())
            .unwrap();
    }
    if matches!(
        phase,
        TransactionPhase::RedisActivated | TransactionPhase::Committed
    ) {
        record = harness
            .journal
            .transition(
                record.tx_id(),
                TransactionPhase::RedisActivated,
                OffsetDateTime::now_utc(),
                None,
            )
            .unwrap();
    }
    if phase == TransactionPhase::Committed {
        record = harness
            .journal
            .transition(
                record.tx_id(),
                TransactionPhase::Committed,
                OffsetDateTime::now_utc(),
                None,
            )
            .unwrap();
    }
    StagedTransaction {
        record,
        redis_revision: RouteConfigRedisRevision::new(metadata, document.clone()).unwrap(),
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
