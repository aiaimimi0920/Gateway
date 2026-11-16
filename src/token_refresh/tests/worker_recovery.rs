//! Durable admission proofs delimit Worker recovery even after the in-memory ticket is lost.
use super::{
    authority_fixture::{Authority, Stall},
    worker_authority::{refresh, reply},
    *,
};
use crate::console::{journal::TransactionJournal, TransactionPhase};

fn records(authority: &Authority) -> Vec<crate::console::TransactionRecord> {
    TransactionJournal::new(authority.persistence.clone())
        .load_transactions()
        .unwrap()
}

pub(super) async fn stalled_refresh(
    authority: &Authority,
    mock: &grok_wire::Mock,
    cache: &cache_wire::Cache,
    pending: &mut Pending,
) {
    // Consume this exact stage notification. Leaving its Notify permit behind can
    // make a later transaction's waiter return before that later future is polled.
    tokio::join!(
        authority.backend.wait_stalled(),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_secs(1),
            refresh(authority, mock, cache, pending)
        ),
    );
    assert_eq!(pending.len(), 1);
    assert!(records(authority)
        .iter()
        .any(|record| record.phase() == TransactionPhase::YamlReplaced));
}

async fn foreign_pending(authority: &Authority, automation: bool) {
    let previous_count = records(authority).len();
    let writer = authority.rebuilt(true);
    let snapshot = writer.route_config().snapshot();
    let mut document = snapshot.document().clone();
    document.providers[0].credentials[0].api_key = Some("ordinary-automation-or-management".into());
    authority.backend.stall(Stall::BeforeActivation);
    let commit = async {
        if automation {
            writer
                .commit_automation_document(
                    snapshot.revision().id(),
                    document,
                    Some("Refresh OAuth credential".into()),
                )
                .await
        } else {
            writer
                .commit_document(snapshot.revision().id(), document, None)
                .await
        }
    };
    let mut commit = Box::pin(commit);
    tokio::select! {
        _ = authority.backend.wait_stalled() => {}
        result = &mut commit => panic!("expected suspended commit, got {result:?}"),
    }
    drop(commit);
    assert_eq!(
        records(authority).len(),
        previous_count + 1,
        "the new transaction actually reached its own stage"
    );
    authority.backend.resume();
}

#[tokio::test]
async fn worker_timeout_before_or_after_activation_recovers_without_repeating_grant() {
    for stage in [Stall::BeforeActivation, Stall::AfterActivation] {
        let authority = Authority::with_writable(false);
        let cache = cache_wire::Cache::new(true).await;
        authority.backend.bind_cache(&cache.pool);
        let mock = grok_wire::Mock::new(200, reply(), 0).await;
        authority.backend.stall(stage);
        let mut pending = Pending::new();
        stalled_refresh(&authority, &mock, &cache, &mut pending).await;
        assert_eq!(cache.commits(), 0);
        authority.backend.resume();
        refresh(&authority, &mock, &cache, &mut pending).await;
        assert!(pending.is_empty());
        assert_eq!(mock.count(), 1);
        assert_eq!(authority.backend.activations(), 1);
        assert!(records(&authority).iter().all(|record| matches!(
            record.phase(),
            TransactionPhase::Committed | TransactionPhase::Aborted
        )));
        server_authority::assert_cache_matches_authority(&authority, &cache);
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn worker_restart_recovers_only_its_accepted_transaction_without_an_in_memory_ticket() {
    let authority = Authority::with_writable(false);
    let cache = cache_wire::Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = grok_wire::Mock::new(200, reply(), 0).await;
    authority.backend.stall(Stall::AfterActivation);
    let mut pending = Pending::new();
    stalled_refresh(&authority, &mock, &cache, &mut pending).await;
    drop(pending);
    authority.backend.resume();
    let runtime = authority.rebuilt(false);
    runtime.recover_startup().await.unwrap();
    assert_eq!(
        records(&authority)[0].phase(),
        TransactionPhase::YamlReplaced,
        "public Worker startup remains read-only"
    );
    runtime.recover_refresh_authority().await.unwrap();
    assert_eq!(records(&authority)[0].phase(), TransactionPhase::Committed);
    assert_eq!(
        runtime.route_config().snapshot().revision().id(),
        authority.active().metadata().id()
    );
    runtime.recover_refresh_authority().await.unwrap();
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(mock.count(), 1);
    assert!(runtime.coordinator().is_none());
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn worker_refuses_old_management_or_automation_journals_before_any_fence_or_grant() {
    for automation in [false, true] {
        let authority = Authority::with_writable(false);
        foreign_pending(&authority, automation).await;
        let yaml = std::fs::read(&authority.routes).unwrap();
        let journal = std::fs::read(authority.persistence.journal_path()).unwrap();
        let original = records(&authority);
        authority.runtime.recover_startup().await.unwrap();
        assert_eq!(
            authority
                .runtime
                .recover_refresh_authority()
                .await
                .unwrap_err()
                .code(),
            "console_recovery_required"
        );
        let cache = cache_wire::Cache::new(true).await;
        authority.backend.bind_cache(&cache.pool);
        let mock = grok_wire::Mock::new(200, reply(), 0).await;
        refresh(&authority, &mock, &cache, &mut Pending::new()).await;
        assert_eq!(mock.count(), 0);
        assert_eq!(cache.commits(), 0);
        assert_eq!(authority.backend.settlements(), 0);
        assert_eq!(records(&authority), original);
        assert_eq!(std::fs::read(&authority.routes).unwrap(), yaml);
        assert_eq!(
            std::fs::read(authority.persistence.journal_path()).unwrap(),
            journal
        );
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn worker_validates_all_pending_before_settling_an_earlier_refresh() {
    let authority = Authority::with_writable(false);
    let cache = cache_wire::Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = grok_wire::Mock::new(200, reply(), 0).await;
    authority.backend.stall(Stall::BeforeActivation);
    let mut pending = Pending::new();
    stalled_refresh(&authority, &mock, &cache, &mut pending).await;
    foreign_pending(&authority, false).await;
    assert_eq!(records(&authority).len(), 2);
    let before = records(&authority);
    assert_eq!(
        authority
            .runtime
            .recover_refresh_authority()
            .await
            .unwrap_err()
            .code(),
        "console_recovery_required"
    );
    assert_eq!(
        authority.backend.settlements(),
        0,
        "no partial settlement before rejecting the foreign transaction"
    );
    assert_eq!(records(&authority), before);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn worker_missing_authority_or_partial_journal_never_bootstraps_or_repairs() {
    for partial in [false, true] {
        let authority = Authority::with_writable(false);
        if partial {
            std::fs::write(authority.persistence.journal_path(), b"{\"incomplete").unwrap();
        } else {
            authority.backend.set_absent();
        }
        let cache = cache_wire::Cache::new(true).await;
        authority.backend.bind_cache(&cache.pool);
        let mock = grok_wire::Mock::new(200, reply(), 0).await;
        refresh(&authority, &mock, &cache, &mut Pending::new()).await;
        assert_eq!(mock.count(), 0);
        assert_eq!(authority.backend.activations(), 0);
        assert_eq!(cache.commits(), 0);
        if partial {
            assert_eq!(
                std::fs::read(authority.persistence.journal_path()).unwrap(),
                b"{\"incomplete"
            );
        }
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn worker_corrupt_or_wrong_identity_proof_does_not_settle_the_transaction() {
    for field in ["version", "prepared", "parentDocumentDigest", "unknown"] {
        let authority = Authority::with_writable(false);
        let cache = cache_wire::Cache::new(true).await;
        authority.backend.bind_cache(&cache.pool);
        let mock = grok_wire::Mock::new(200, reply(), 0).await;
        authority.backend.stall(Stall::AfterActivation);
        let mut pending = Pending::new();
        stalled_refresh(&authority, &mock, &cache, &mut pending).await;
        authority.backend.resume();
        let record = records(&authority).remove(0);
        let path = authority
            .persistence
            .state_root()
            .join("refresh-intents")
            .join(format!("{}.json", record.tx_id()));
        let mut proof: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        proof[field] = serde_json::json!("not-the-accepted-value");
        std::fs::write(path, serde_json::to_vec(&proof).unwrap()).unwrap();
        assert!(authority.runtime.recover_refresh_authority().await.is_err());
        assert_eq!(authority.backend.settlements(), 0);
        assert_eq!(
            records(&authority)[0].phase(),
            TransactionPhase::YamlReplaced
        );
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn worker_existing_pending_with_absent_authority_has_no_settlement_side_effect() {
    let authority = Authority::with_writable(false);
    let cache = cache_wire::Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = grok_wire::Mock::new(200, reply(), 0).await;
    authority.backend.stall(Stall::BeforeActivation);
    stalled_refresh(&authority, &mock, &cache, &mut Pending::new()).await;
    authority.backend.resume();
    authority.backend.set_absent();
    let before = records(&authority);
    let yaml = std::fs::read(&authority.routes).unwrap();
    let journal = std::fs::read(authority.persistence.journal_path()).unwrap();
    assert!(authority.runtime.recover_refresh_authority().await.is_err());
    assert_eq!(authority.backend.settlements(), 0);
    assert_eq!(records(&authority), before);
    assert_eq!(std::fs::read(&authority.routes).unwrap(), yaml);
    assert_eq!(
        std::fs::read(authority.persistence.journal_path()).unwrap(),
        journal
    );
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn worker_invalid_rotation_is_an_error_not_success_or_cache_permission() {
    let authority = Authority::with_writable(false);
    let cache = cache_wire::Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = grok_wire::Mock::new(200, reply(), 0).await;
    authority.backend.stall(Stall::BeforeActivation);
    let mut pending = Pending::new();
    stalled_refresh(&authority, &mock, &cache, &mut pending).await;
    authority.backend.resume();
    let ticket = pending
        .values_mut()
        .next()
        .unwrap()
        .as_result_mut()
        .unwrap();
    ticket.token.access_token.clear();
    assert_eq!(
        authority
            .runtime
            .commit_refresh_ticket(ticket)
            .await
            .unwrap_err()
            .code(),
        "console_refresh_projection_invalid"
    );
    assert!(
        !ticket
            .write_to(Some(&authority.runtime), Some(&cache.pool))
            .await
    );
    assert_eq!(cache.commits(), 0);
    assert_eq!(authority.backend.activations(), 0);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    cache.stop().await;
}
