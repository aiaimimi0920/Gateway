//! Lost ACKs never reopen grants; the witness drops the owned future after applying its synthetic CAS.
use super::{
    authority_fixture::{Authority, Stall},
    cache_wire::Cache,
    grok_wire::{rotated, Mock},
    *,
};

async fn refresh(runtime: &RouteConfigRuntime, mock: &Mock, cache: &Cache, pending: &mut Pending) {
    tokio::time::timeout(
        Duration::from_secs(3),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_millis(100),
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
async fn reserve_ack_loss_never_sends_and_restart_cannot_reclaim_it() {
    let authority = Authority::new();
    let mock = Mock::new(200, rotated(), 0).await;
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    authority.backend.stall(Stall::AfterGrantReservation);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(mock.count(), 0);
    assert!(pending.is_empty());
    authority.backend.resume();
    let restarted = authority.restart().await;
    let snapshot = restarted.route_config().snapshot();
    assert_eq!(snapshot.revision().id(), authority.active().metadata().id());
    assert_ne!(
        snapshot.revision().timestamp(),
        authority.active().metadata().timestamp(),
        "a fresh bootstrap can retain its local timestamp under the same content-derived r0 ID"
    );
    refresh(&restarted, &mock, &cache, &mut pending).await;
    assert_eq!(
        mock.count(),
        0,
        "unacknowledged reserve is not a sender permit"
    );
    assert_eq!(cache.commits(), 0);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn result_ack_loss_cannot_apply_but_restart_can_recover_the_stored_result() {
    let authority = Authority::new();
    let mock = Mock::new(200, rotated(), 0).await;
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    authority.backend.stall(Stall::AfterGrantResult);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(pending.len(), 1);
    assert!(
        !pending
            .values()
            .next()
            .unwrap()
            .as_result()
            .unwrap()
            .applied
    );
    assert_eq!(
        server_policy::owner(authority.runtime.route_config())
            .runtime_override()
            .0,
        None
    );
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("synthetic-build-token")
    );
    assert_eq!(cache.commits(), 0);
    pending.clear(); // A new runtime owns a new in-process authorization/guard after restart.
    authority.backend.resume();
    let restarted = authority.restart().await;
    let snapshot = restarted.route_config().snapshot();
    assert_eq!(snapshot.revision().id(), authority.active().metadata().id());
    assert_ne!(
        snapshot.revision().timestamp(),
        authority.active().metadata().timestamp()
    );
    refresh(&restarted, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        mock.count(),
        1,
        "recover, do not reuse the consumed refresh token"
    );
    assert_eq!(cache.commits(), 1);
    assert_eq!(
        authority.active().document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("rotated-build-access")
    );
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn publication_ack_loss_retries_evidence_and_cache_without_another_grant_or_revision() {
    let authority = Authority::new();
    let mock = Mock::new(200, rotated(), 0).await;
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    authority.backend.stall(Stall::AfterGrantPublication);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(pending.len(), 1);
    assert_eq!(cache.commits(), 0);
    let revision = authority.active().metadata().id().to_owned();
    authority.backend.resume();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(authority.active().metadata().id(), revision);
    assert_eq!(cache.commits(), 1);
    server_authority::assert_cache_matches_authority(&authority, &cache);
    mock.stop().await;
    cache.stop().await;
}
