//! A cancelled coordinator cannot roll back while its accepted remote command can still activate.
use super::*;
use crate::console::TransactionPhase;
use std::time::Duration;
#[path = "transaction_fence_fixture.rs"]
mod fixture;
use fixture::{document, Harness, Reply, Stage};

async fn cancel_after_acceptance(harness: &Harness) {
    let revision = harness
        .runtime
        .route_config()
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let commit = tokio::time::timeout(
        Duration::from_millis(250),
        harness.runtime.commit_document(
            &revision,
            document("rotated"),
            Some("late command fixture".into()),
        ),
    );
    let (result, ()) = tokio::join!(commit, harness.backend.wait_accepted());
    assert!(
        result.is_err(),
        "caller must drop a command that is already accepted remotely"
    );
    drop(harness.persistence.try_writer_lock().unwrap());
}

#[tokio::test]
async fn recovery_fences_accepted_activation_before_rollback_even_when_caller_is_gone() {
    let harness = Harness::new(Stage::Activate);
    let original = harness.backend.active().metadata().id().to_owned();
    cancel_after_acceptance(&harness).await;
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::YamlReplaced
    );
    let recovery = harness.runtime.recover_startup().await;
    let late = harness.backend.release_and_join().await;
    recovery.unwrap();
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Aborted
    );
    assert_eq!(
        std::fs::read(&harness.routes).unwrap(),
        harness.original_yaml
    );
    assert!(
        matches!(
            late,
            Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
        ),
        "the accepted activation must be rejected after recovery declares Aborted; got {late:?}"
    );
    assert_eq!(harness.backend.active().metadata().id(), original);
}

#[tokio::test]
async fn recovery_tombstone_blocks_prepared_command_accepted_before_caller_drop() {
    let harness = Harness::new(Stage::Prepare);
    let original = harness.backend.active().metadata().id().to_owned();
    cancel_after_acceptance(&harness).await;
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Prepared
    );
    let recovery = harness.runtime.recover_startup().await;
    let late = harness.backend.release_and_join().await;
    recovery.unwrap();
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Aborted
    );
    assert!(
        matches!(late, Reply::Prepared(false)),
        "a late SET NX must not recreate revoked Prepared; got {late:?}"
    );
    assert_eq!(harness.backend.active().metadata().id(), original);
    assert_eq!(
        std::fs::read(&harness.routes).unwrap(),
        harness.original_yaml
    );
}

#[tokio::test]
async fn recovery_finishes_activation_that_executed_before_settlement() {
    let harness = Harness::new(Stage::Activate);
    cancel_after_acceptance(&harness).await;
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::Activated)
    ));
    harness.runtime.recover_startup().await.unwrap();
    let record = &harness.journal.load_transactions().unwrap()[0];
    assert_eq!(record.phase(), TransactionPhase::Committed);
    assert_eq!(
        harness.runtime.route_config().snapshot().revision().id(),
        record.new_revision()
    );
}

#[tokio::test]
async fn revoked_transaction_is_not_credited_for_successor_with_same_revision() {
    let harness = Harness::new(Stage::Activate);
    cancel_after_acceptance(&harness).await;
    let record = harness.journal.load_transactions().unwrap().remove(0);
    let tombstone = record.revocation_tombstone().unwrap();
    harness.backend.set_transaction(tombstone.clone());
    let successor = harness.backend.candidate(record.new_revision());
    harness.backend.set_active(successor.clone());
    harness.runtime.recover_startup().await.unwrap();
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Aborted
    );
    assert_eq!(harness.backend.transaction(record.tx_id()), tombstone);
    assert_eq!(
        harness.backend.active().to_json().unwrap(),
        successor.to_json().unwrap()
    );
    assert_ne!(
        std::fs::read(&harness.routes).unwrap(),
        harness.original_yaml
    );
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
    ));
}

#[tokio::test]
async fn lost_settlement_response_does_not_roll_back_until_retry_confirms_tombstone() {
    let harness = Harness::new(Stage::Activate);
    cancel_after_acceptance(&harness).await;
    let replaced = std::fs::read(&harness.routes).unwrap();
    harness.backend.lose_settle_response();
    assert!(tokio::time::timeout(
        Duration::from_millis(250),
        harness.runtime.recover_startup()
    )
    .await
    .is_err());
    let record = harness.journal.load_transactions().unwrap().remove(0);
    assert_eq!(record.phase(), TransactionPhase::YamlReplaced);
    assert_eq!(std::fs::read(&harness.routes).unwrap(), replaced);
    assert_eq!(
        harness.backend.transaction(record.tx_id()),
        record.revocation_tombstone().unwrap()
    );
    harness.runtime.recover_startup().await.unwrap();
    assert_eq!(
        std::fs::read(&harness.routes).unwrap(),
        harness.original_yaml
    );
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
    ));
}

#[tokio::test]
async fn authority_advancing_after_fence_leaves_local_mirror_unmodified() {
    let harness = Harness::new(Stage::Activate);
    cancel_after_acceptance(&harness).await;
    let record = harness.journal.load_transactions().unwrap().remove(0);
    let candidate = harness.backend.candidate(record.new_revision());
    let document = document("unrelated");
    let validated = crate::console::document::validate_route_document(document.clone()).unwrap();
    let metadata = crate::console::revision::RevisionMetadata::from_validated(
        candidate.metadata().sequence() + 1,
        Some(candidate.metadata().id().into()),
        RevisionActor::Recovery,
        time::OffsetDateTime::now_utc(),
        None,
        &validated,
    );
    let successor = RouteConfigRedisRevision::new(metadata, document).unwrap();
    harness.backend.advance_after_settle(successor.clone());
    let replaced = std::fs::read(&harness.routes).unwrap();
    let error = harness.runtime.recover_startup().await.unwrap_err();
    assert_eq!(error.code(), "console_recovery_required");
    assert_eq!(std::fs::read(&harness.routes).unwrap(), replaced);
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::YamlReplaced
    );
    assert_eq!(
        harness.backend.active().to_json().unwrap(),
        successor.to_json().unwrap()
    );
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
    ));
}

#[path = "transaction_fence_unsupported_tests.rs"]
mod unsupported;

#[tokio::test]
async fn normal_commit_gate_mismatch_preserves_successor_authority_instead_of_rolling_back() {
    let harness = Harness::new(Stage::Activate);
    let runtime = harness.runtime.clone();
    let commit = tokio::spawn(async move {
        let expected = runtime.route_config().snapshot().revision().id().to_owned();
        runtime
            .commit_document(&expected, document("rotated"), None)
            .await
    });
    harness.backend.wait_accepted().await;
    let record = harness.journal.load_transactions().unwrap().remove(0);
    let successor = harness.backend.candidate(record.new_revision());
    harness
        .backend
        .set_transaction(record.revocation_tombstone().unwrap());
    harness.backend.set_active(successor.clone());
    assert!(matches!(
        harness.backend.release_and_join().await,
        Reply::Activated(RouteConfigRedisActivationOutcome::PreparedTransactionMismatch)
    ));
    let error = commit.await.unwrap().unwrap_err();
    assert_eq!(error.code(), "console_redis_invalid_state");
    assert_eq!(
        harness.journal.load_transactions().unwrap()[0].phase(),
        TransactionPhase::Aborted
    );
    assert_eq!(
        harness.runtime.route_config().snapshot().revision().id(),
        successor.metadata().id()
    );
    assert_eq!(
        std::fs::read(&harness.routes).unwrap(),
        crate::console::document::canonicalize_route_document(successor.document())
            .unwrap()
            .canonical_yaml()
    );
}
