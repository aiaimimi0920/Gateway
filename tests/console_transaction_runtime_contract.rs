//! Runtime transactions preserve YAML, Redis and journal outcomes through recovery.

use neuro_gateway::console::document::canonicalize_route_document;
use neuro_gateway::console::TransactionPhase;
use neuro_gateway::routing::config::ActiveConfigSource;
use std::fs;
use std::sync::Arc;
#[path = "console_transaction_support/mod.rs"]
mod support;
use support::*;

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
async fn coordinator_bootstraps_redis_when_yaml_revision_has_no_active_key() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "bootstrap-missing-redis-active",
        old,
        FakeRedisMode::BootstrapOnly,
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();

    let snapshot = harness
        .runtime
        .commit_document(
            &expected_revision,
            new,
            Some("bootstrap redis active revision".to_string()),
        )
        .await
        .expect("a YAML-only runtime should bootstrap Redis on first commit");

    assert_eq!(snapshot.source(), ActiveConfigSource::Redis);
    assert_eq!(
        harness.backend.active_revision_id(),
        Some(snapshot.revision().id().to_string())
    );
}

#[tokio::test]
async fn coordinator_retry_reuses_aborted_revision_archive_after_bootstrap_conflict() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "retry-aborted-bootstrap",
        old,
        FakeRedisMode::RevisionConflict { actual: None },
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();
    let message = "bootstrap attempt";

    let first_error = harness
        .runtime
        .commit_document(&expected_revision, new.clone(), Some(message.to_string()))
        .await
        .expect_err("the simulated Redis conflict should abort the first attempt");
    assert_eq!(first_error.code(), "console_revision_conflict");

    harness.backend.set_mode(FakeRedisMode::BootstrapOnly);
    let snapshot = harness
        .runtime
        .commit_document(&expected_revision, new, Some(message.to_string()))
        .await
        .expect("retry should reuse the immutable archive left by the aborted attempt");

    assert_eq!(snapshot.source(), ActiveConfigSource::Redis);
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions.len(), 2);
    assert_eq!(
        transactions
            .iter()
            .filter(|record| record.phase() == TransactionPhase::Aborted)
            .count(),
        1
    );
    assert_eq!(
        transactions
            .iter()
            .filter(|record| record.phase() == TransactionPhase::Committed)
            .count(),
        1
    );
}

#[tokio::test]
async fn coordinator_retry_rejects_changed_message_for_aborted_revision_archive() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "retry-aborted-message-collision",
        old,
        FakeRedisMode::RevisionConflict { actual: None },
        true,
    );
    let expected_revision = harness.store.snapshot().revision().id().to_string();

    let first_error = harness
        .runtime
        .commit_document(
            &expected_revision,
            new.clone(),
            Some("first bootstrap attempt".to_string()),
        )
        .await
        .expect_err("the simulated Redis conflict should abort the first attempt");
    assert_eq!(first_error.code(), "console_revision_conflict");

    harness.backend.set_mode(FakeRedisMode::BootstrapOnly);
    let retry_error = harness
        .runtime
        .commit_document(
            &expected_revision,
            new,
            Some("changed retry message".to_string()),
        )
        .await
        .expect_err("a changed message must not silently reuse immutable audit metadata");

    assert_eq!(retry_error.code(), "console_revision_collision");
    assert!(retry_error
        .to_string()
        .contains("does not match the retry candidate"));
    assert_eq!(harness.backend.active_revision_id(), None);
    let transactions = harness.journal.load_transactions().unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].phase(), TransactionPhase::Aborted);
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

#[tokio::test]
async fn startup_recovery_commits_prepared_transaction_when_redis_already_points_to_new_revision() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "startup-prepared-redis-new",
        old.clone(),
        FakeRedisMode::Activated,
        true,
    );
    let staged = stage_transaction(&harness, &new, TransactionPhase::Prepared);
    harness
        .backend
        .set_active_revision(staged.redis_revision.clone());

    harness.runtime.recover_startup().await.unwrap();

    let recovered = harness
        .journal
        .load_transaction(staged.record.tx_id())
        .unwrap();
    assert_eq!(recovered.phase(), TransactionPhase::Committed);
    assert_eq!(
        harness.store.snapshot().resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        harness.store.snapshot().source(),
        ActiveConfigSource::Recovered
    );
    assert_eq!(
        fs::read(&harness.routes).unwrap(),
        canonicalize_route_document(&new).unwrap().canonical_yaml()
    );
}

#[tokio::test]
async fn startup_recovery_finishes_redis_activated_transaction_and_repairs_yaml() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "startup-redis-activated",
        old.clone(),
        FakeRedisMode::Activated,
        true,
    );
    let staged = stage_transaction(&harness, &new, TransactionPhase::RedisActivated);
    harness
        .backend
        .set_active_revision(staged.redis_revision.clone());
    fs::write(
        &harness.routes,
        canonicalize_route_document(&old).unwrap().canonical_yaml(),
    )
    .unwrap();

    harness.runtime.recover_startup().await.unwrap();

    let recovered = harness
        .journal
        .load_transaction(staged.record.tx_id())
        .unwrap();
    assert_eq!(recovered.phase(), TransactionPhase::Committed);
    assert_eq!(
        fs::read(&harness.routes).unwrap(),
        canonicalize_route_document(&new).unwrap().canonical_yaml()
    );
}

#[tokio::test]
async fn startup_recovery_uses_latest_local_committed_revision_when_redis_is_empty() {
    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let harness = RuntimeHarness::new(
        "startup-local-committed",
        old.clone(),
        FakeRedisMode::Activated,
        true,
    );
    let staged = stage_transaction(&harness, &new, TransactionPhase::Committed);
    fs::write(
        &harness.routes,
        canonicalize_route_document(&old).unwrap().canonical_yaml(),
    )
    .unwrap();

    harness.runtime.recover_startup().await.unwrap();

    assert_eq!(
        harness.store.snapshot().revision().id(),
        staged.record.new_revision()
    );
    assert_eq!(
        harness.store.snapshot().source(),
        ActiveConfigSource::Recovered
    );
    assert_eq!(
        harness.store.snapshot().resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        fs::read(&harness.routes).unwrap(),
        canonicalize_route_document(&new).unwrap().canonical_yaml()
    );
}
