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
    RouteConfigRedisKeys, RouteConfigRedisRevision, RouteConfigRedisStoreError, RouteConfigRuntime,
    TransactionPhase, TransactionRecord,
};
use neuro_gateway::redis::keys::{
    console_route_config_active_document_key, console_route_config_active_revision_key,
    console_route_config_events_key, console_route_config_revision_key,
    console_route_config_transaction_key, LEGACY_ROUTE_CONFIG_DOCUMENT_KEY,
};
use neuro_gateway::routing::config::{ActiveConfigSource, RouteConfigStore, RouteConfigYaml};
use time::OffsetDateTime;

#[test]
fn default_namespace_uses_the_console_route_config_key_family() {
    let keys = RouteConfigRedisKeys::new("default").unwrap();

    assert_eq!(keys.namespace(), "default");
    assert_eq!(
        keys.active_revision_key(),
        "gw:console:route-config:default:active_revision"
    );
    assert_eq!(
        keys.active_document_key(),
        "gw:console:route-config:default:active_document"
    );
    assert_eq!(
        keys.revision_key("r19-123456abcdef"),
        "gw:console:route-config:default:revisions:r19-123456abcdef"
    );
    assert_eq!(
        keys.transaction_key("tx-123"),
        "gw:console:route-config:default:transactions:tx-123"
    );
    assert_eq!(keys.events_key(), "gw:console:route-config:default:events");
    assert_eq!(
        keys.legacy_document_key(),
        Some(LEGACY_ROUTE_CONFIG_DOCUMENT_KEY)
    );
}

#[test]
fn non_default_namespace_does_not_update_the_global_legacy_mirror() {
    let keys = RouteConfigRedisKeys::new("dev_a").unwrap();

    assert!(!keys.updates_legacy_mirror());
    assert_eq!(keys.legacy_document_key(), None);
    assert_eq!(
        keys.active_revision_key(),
        "gw:console:route-config:dev_a:active_revision"
    );
}

#[test]
fn redis_route_config_namespace_rejects_colons_and_other_invalid_characters() {
    let error = RouteConfigRedisKeys::new("dev:blue").unwrap_err();

    assert_eq!(error.code(), "console_invalid_redis_namespace");
}

#[test]
fn low_level_console_route_key_builders_follow_the_specified_layout() {
    assert_eq!(
        console_route_config_active_revision_key("worker"),
        "gw:console:route-config:worker:active_revision"
    );
    assert_eq!(
        console_route_config_active_document_key("worker"),
        "gw:console:route-config:worker:active_document"
    );
    assert_eq!(
        console_route_config_revision_key("worker", "r7-abcdef123456"),
        "gw:console:route-config:worker:revisions:r7-abcdef123456"
    );
    assert_eq!(
        console_route_config_transaction_key("worker", "tx_9"),
        "gw:console:route-config:worker:transactions:tx_9"
    );
    assert_eq!(
        console_route_config_events_key("worker"),
        "gw:console:route-config:worker:events"
    );
}

#[tokio::test]
async fn coordinator_commit_persists_yaml_installs_redis_snapshot_and_commits_transaction() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "commit-success",
        old.clone(),
        FakeRedisMode::Activated,
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();

    let snapshot = harness
        .runtime
        .commit_document(
            &expected_revision,
            new.clone(),
            Some("apply update".to_string()),
        )
        .await
        .unwrap();

    assert!(Arc::ptr_eq(harness.runtime.route_config(), &harness.store));
    assert_eq!(snapshot.source(), ActiveConfigSource::Redis);
    assert_eq!(
        snapshot.resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        harness.store.snapshot().revision().id(),
        snapshot.revision().id()
    );
    let canonical = canonicalize_route_document(&new).unwrap();
    assert_eq!(
        fs::read(&harness.routes).unwrap(),
        canonical.canonical_yaml()
    );
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].phase(), TransactionPhase::Committed);
    assert_eq!(
        harness.backend.active_revision_id(),
        Some(snapshot.revision().id().to_string())
    );
}

#[tokio::test]
async fn replica_runtime_rejects_mutations() {
    let old = document("old-provider", "old-model");
    let harness = RuntimeHarness::new("replica-read-only", old, FakeRedisMode::Activated, false);
    let expected_revision = harness.store.snapshot().revision().id().to_string();

    let error = harness
        .runtime
        .commit_document(
            &expected_revision,
            document("new-provider", "new-model"),
            Some("should fail".to_string()),
        )
        .await
        .unwrap_err();

    assert_eq!(error.code(), "console_mutation_not_supported");
}

#[tokio::test]
async fn coordinator_commit_rolls_back_yaml_when_redis_reports_revision_conflict() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "redis-conflict",
        old.clone(),
        FakeRedisMode::RevisionConflict {
            actual: Some("r77-deadbeefcafe".to_string()),
        },
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();
    let old_yaml = canonicalize_route_document(&old)
        .unwrap()
        .canonical_yaml()
        .to_vec();

    let error = harness
        .runtime
        .commit_document(&expected_revision, new, Some("conflict".to_string()))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "console_revision_conflict");
    assert_eq!(fs::read(&harness.routes).unwrap(), old_yaml);
    assert_eq!(
        harness.store.snapshot().resolve_alias(Some("answer")),
        Some("old-model".to_string())
    );
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].phase(), TransactionPhase::Aborted);
    assert_eq!(
        transactions[0].abort_code(),
        Some("redis_revision_conflict")
    );
}

#[tokio::test]
async fn indeterminate_activation_finishes_when_new_revision_is_already_authoritative() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "indeterminate-success",
        old,
        FakeRedisMode::IndeterminateNewRevision,
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();

    let snapshot = harness
        .runtime
        .commit_document(&expected_revision, new, Some("indeterminate".to_string()))
        .await
        .unwrap();

    assert_eq!(snapshot.source(), ActiveConfigSource::Redis);
    assert_eq!(
        harness.backend.active_revision_id(),
        Some(snapshot.revision().id().to_string())
    );
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions[0].phase(), TransactionPhase::Committed);
}

#[tokio::test]
async fn replica_reconcile_installs_newer_redis_revision() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new("replica-reconcile", old, FakeRedisMode::Activated, false);
    let redis_revision = redis_revision_from_document(5, Some("r4-aaaaaaaaaaaa".to_string()), &new);
    harness.backend.set_active_revision(redis_revision.clone());

    let snapshot = harness
        .runtime
        .reconcile_from_redis()
        .await
        .unwrap()
        .expect("replica should install the newer Redis revision");

    assert_eq!(snapshot.source(), ActiveConfigSource::Redis);
    assert_eq!(snapshot.revision().id(), redis_revision.metadata().id());
    assert_eq!(
        harness.store.snapshot().resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
}

#[tokio::test]
async fn indeterminate_activation_rolls_back_when_old_revision_remains_active() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "indeterminate-rollback",
        old.clone(),
        FakeRedisMode::IndeterminateOldRevision,
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();
    harness
        .backend
        .set_active_revision(redis_revision_with_metadata(
            harness.store.snapshot().revision().clone(),
            &old,
        ));
    let old_yaml = canonicalize_route_document(&old)
        .unwrap()
        .canonical_yaml()
        .to_vec();

    let error = harness
        .runtime
        .commit_document(&expected_revision, new, Some("indeterminate".to_string()))
        .await
        .unwrap_err();

    assert_eq!(error.code(), "console_redis_indeterminate");
    assert_eq!(fs::read(&harness.routes).unwrap(), old_yaml);
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions[0].phase(), TransactionPhase::Aborted);
    assert_eq!(
        transactions[0].abort_code(),
        Some("redis_indeterminate_rolled_back")
    );
}

struct RuntimeHarness {
    _temp: TestDirectory,
    routes: PathBuf,
    store: Arc<RouteConfigStore>,
    runtime: RouteConfigRuntime,
    backend: FakeRedisBackend,
    journal: TransactionJournal,
}

impl RuntimeHarness {
    fn new(label: &str, document: RouteConfigYaml, mode: FakeRedisMode, writable: bool) -> Self {
        let temp = TestDirectory::new(label);
        let routes = temp.path().join("routes.yaml");
        let canonical = canonicalize_route_document(&document).unwrap();
        fs::write(&routes, canonical.canonical_yaml()).unwrap();
        let store = Arc::new(RouteConfigStore::from_document(document).unwrap());
        let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
        let journal = TransactionJournal::new(persistence.clone());
        let backend = FakeRedisBackend::new(mode);
        let runtime = RouteConfigRuntime::with_backend(
            Arc::clone(&store),
            persistence,
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
        }
    }
}

#[derive(Clone, Debug)]
struct FakeRedisBackend {
    state: Arc<Mutex<FakeRedisState>>,
}

impl FakeRedisBackend {
    fn new(mode: FakeRedisMode) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeRedisState {
                mode,
                ..FakeRedisState::default()
            })),
        }
    }

    fn active_revision_id(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .active_revision
            .as_ref()
            .map(|revision| revision.metadata().id().to_string())
    }

    fn set_active_revision(&self, revision: RouteConfigRedisRevision) {
        self.state.lock().unwrap().active_revision = Some(revision);
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
enum FakeRedisMode {
    #[default]
    Activated,
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

fn document(provider_id: &str, model: &str) -> RouteConfigYaml {
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

fn redis_revision_from_document(
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

fn redis_revision_with_metadata(
    metadata: RevisionMetadata,
    document: &RouteConfigYaml,
) -> RouteConfigRedisRevision {
    RouteConfigRedisRevision::new(metadata, document.clone()).unwrap()
}
