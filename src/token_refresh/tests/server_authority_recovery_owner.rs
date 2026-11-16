//! Dropping a superseded ticket cannot discard the shared authority's recovery obligation.
use super::{
    authority_fixture::{Authority, Stall},
    cache_wire::Cache,
    grok_wire::{rotated, Mock},
    *,
};
use crate::console::{journal::TransactionJournal, TransactionPhase};

async fn refresh(runtime: &RouteConfigRuntime, mock: &Mock, cache: &Cache, pending: &mut Pending) {
    tokio::time::timeout(
        Duration::from_secs(3),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_millis(250),
            super::super::grok::TEST_ISSUER.scope(
                mock.issuer.clone(),
                refresh_once(
                    runtime.route_config(),
                    &client(),
                    Some(runtime),
                    Some(&cache.pool),
                    pending,
                    Duration::from_secs(300),
                    None,
                ),
            ),
        ),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn superseded_pending_leaves_recovery_obligation_shared_by_runtime_clones() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, rotated(), 0).await;
    authority.backend.stall(Stall::BeforeActivation);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    authority.backend.resume();

    // A normal management write can supersede the ticket while its older journal remains unresolved.
    let snapshot = authority.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let credential = &mut document.providers[0].credentials[0];
    credential.api_key = Some("explicit-reauthorized-access".into());
    credential.refresh_token = Some("explicit-reauthorized-refresh".into());
    authority
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    let revision = authority.active().metadata().id().to_owned();
    assert!(!pending
        .values()
        .next()
        .unwrap()
        .as_result()
        .unwrap()
        .is_current(authority.runtime.route_config()));
    let mut reply = rotated();
    reply["access_token"] = serde_json::json!("new-authorization-rotation");
    reply["refresh_token"] = serde_json::json!("new-authorization-refresh");
    let next_mock = Mock::new(200, reply, 0).await;
    let cloned = authority.runtime.clone();
    refresh(&cloned, &next_mock, &cache, &mut pending).await;
    assert_eq!(
        pending.len(),
        1,
        "new authority write must not bypass unresolved prior journal"
    );
    assert_eq!(
        pending
            .values()
            .next()
            .unwrap()
            .as_result()
            .unwrap()
            .token
            .access_token,
        "new-authorization-rotation"
    );
    assert_eq!(authority.active().metadata().id(), revision);
    assert_eq!(
        authority.backend.activations(),
        1,
        "only explicit reauthorization was activated"
    );
    assert_eq!(cache.commits(), 0);
    let records = TransactionJournal::new(authority.persistence.clone())
        .load_transactions()
        .unwrap();
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .any(|record| record.phase() == TransactionPhase::YamlReplaced));
    refresh(&authority.runtime, &next_mock, &cache, &mut pending).await;
    assert_eq!(
        next_mock.count(),
        1,
        "ambiguous recovery keeps the same new ticket, not a new grant"
    );
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    next_mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn shared_authority_lock_wait_is_inside_budget_and_cannot_clear_another_owner() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, rotated(), 0).await;
    let cloned = authority.runtime.clone();
    let guard = cloned.refresh_authority_recovery.lock().await;
    let revision = authority.active().metadata().id().to_owned();
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(authority.active().metadata().id(), revision);
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 0);
    assert!(
        !*guard,
        "a waiter must not mutate the current owner's recovery state"
    );
    drop(guard);
    refresh(&cloned, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 1);
    assert!(!*authority.runtime.refresh_authority_recovery.lock().await);
    mock.stop().await;
    cache.stop().await;
}
