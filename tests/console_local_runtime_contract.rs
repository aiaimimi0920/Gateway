//! Local desktop configuration must survive restarts without any Redis service.
use neuro_gateway::console::{
    ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime, TransactionPhase,
};
use neuro_gateway::routing::config::{ActiveConfigSource, RouteConfigStore};
use std::fs;
#[path = "console_transaction_support/mod.rs"]
mod support;
use support::*;

fn config(harness: &RuntimeHarness) -> ConsoleConfig {
    ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(
            harness
                .persistence
                .state_root()
                .parent()
                .unwrap()
                .to_path_buf(),
        ),
        routes_file: Some(harness.routes.clone()),
        ..Default::default()
    })
    .unwrap()
}

#[tokio::test]
async fn local_commit_restart_and_conflict_without_redis() {
    let harness = RuntimeHarness::new(
        "local-commit",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    runtime.recover_startup().await.unwrap();
    let previous = runtime
        .route_config()
        .snapshot()
        .revision()
        .id()
        .to_string();
    let next = runtime
        .commit_document(&previous, document("new", "new-model"), None)
        .await
        .unwrap();
    assert_eq!(next.source(), ActiveConfigSource::Recovered);
    assert_eq!(next.resolve_alias(Some("answer")), Some("new-model".into()));
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Committed
    );
    let stale = runtime
        .commit_document(&previous, document("stale", "stale-model"), None)
        .await
        .unwrap_err();
    assert_eq!(stale.code(), "console_revision_conflict");
    let restarted = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        restarted.route_config().snapshot().revision().id(),
        next.revision().id()
    );
    let again = restarted
        .commit_document(next.revision().id(), document("third", "third-model"), None)
        .await
        .unwrap();
    assert_eq!(again.revision().sequence(), next.revision().sequence() + 1);
}

#[tokio::test]
async fn local_bootstrap_materializes_credential_ids_before_archiving() {
    let harness = RuntimeHarness::new(
        "local-credential-ids",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    let yaml = r#"providers:
  - id: example
    preset: openai
    base_url: https://example.invalid
    credentials:
      - api_key: fixture-first
      - id: preserved-account
        api_key: fixture-second
    supported_models: [example-model]
model_routes:
  - pattern: example-model
    provider_ids: [example]
"#;
    fs::write(&harness.routes, yaml).unwrap();
    let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    runtime.recover_startup().await.unwrap();
    let snapshot = runtime.route_config().snapshot();
    let archived = harness
        .persistence
        .load_revision(snapshot.revision().id())
        .unwrap()
        .unwrap();
    let credentials = &archived.document().providers[0].credentials;
    assert_eq!(credentials[0].id.as_deref(), Some("example-cred-0"));
    assert_eq!(credentials[1].id.as_deref(), Some("preserved-account"));
    let restarted = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        restarted.route_config().snapshot().revision(),
        snapshot.revision()
    );
    // Bootstrap must not rewrite the user's YAML merely to materialize IDs.
    assert_eq!(fs::read_to_string(&harness.routes).unwrap(), yaml);
}

#[tokio::test]
async fn local_recovers_yaml_replacement_before_authority_publication() {
    let harness = RuntimeHarness::new(
        "local-rollback",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    runtime.recover_startup().await.unwrap();
    stage_transaction(
        &harness,
        &document("pending", "pending-model"),
        TransactionPhase::YamlReplaced,
    );
    let restarted = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        restarted
            .route_config()
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("old-model".into())
    );
    assert_eq!(
        RouteConfigStore::load_from_yaml(&harness.routes)
            .unwrap()
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("old-model".into())
    );
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Aborted
    );
}

#[tokio::test]
async fn local_recovers_authority_publication_before_journal_completion() {
    let harness = RuntimeHarness::new(
        "local-recover",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    runtime.recover_startup().await.unwrap();
    let staged = stage_transaction(
        &harness,
        &document("pending", "pending-model"),
        TransactionPhase::YamlReplaced,
    );
    fs::write(
        harness.persistence.state_root().join("local-active.json"),
        serde_json::to_vec(staged.redis_revision.metadata().id()).unwrap(),
    )
    .unwrap();
    let restarted = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        restarted
            .route_config()
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("pending-model".into())
    );
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Committed
    );
}

#[test]
fn local_rejects_corrupt_authority_instead_of_resetting_state() {
    let harness = RuntimeHarness::new(
        "local-corrupt",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    fs::write(
        harness.persistence.state_root().join("local-active.json"),
        br#""../escape""#,
    )
    .unwrap();
    assert!(RouteConfigRuntime::new_local(&config(&harness)).is_err());
}

#[tokio::test]
async fn local_missing_authority_cannot_silently_bootstrap_a_running_store() {
    let harness = RuntimeHarness::new(
        "local-missing",
        document("old", "old-model"),
        FakeRedisMode::Activated,
        true,
    );
    let runtime = RouteConfigRuntime::new_local(&config(&harness)).unwrap();
    let before = fs::read(&harness.routes).unwrap();
    fs::remove_file(harness.persistence.state_root().join("local-active.json")).unwrap();
    let expected = runtime
        .route_config()
        .snapshot()
        .revision()
        .id()
        .to_string();
    let error = runtime
        .commit_document(&expected, document("new", "new-model"), None)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "console_recovery_required");
    assert_eq!(fs::read(&harness.routes).unwrap(), before);
}
