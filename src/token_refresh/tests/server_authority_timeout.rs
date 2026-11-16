//! Server authority awaits are bounded without releasing the rotated-material ticket.
use super::{
    authority_fixture::{Authority, Stall},
    cache_wire::Cache,
    grok_wire::{rotated, Mock},
    server_authority::assert_cache_matches_authority,
    *,
};
use crate::console::{journal::TransactionJournal, TransactionPhase};
use crate::state::GatewayShutdownHandle;

async fn refresh(
    authority: &Authority,
    mock: &Mock,
    cache: &Cache,
    pending: &mut Pending,
    shutdown: Option<&GatewayShutdownHandle>,
) {
    // The outer watchdog is deliberately longer; it exposes a missing production deadline.
    tokio::time::timeout(
        Duration::from_secs(3),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_millis(250),
            super::super::grok::TEST_ISSUER.scope(
                mock.issuer.clone(),
                refresh_once(
                    authority.runtime.route_config(),
                    &client(),
                    Some(&authority.runtime),
                    Some(&cache.pool),
                    pending,
                    Duration::from_secs(300),
                    shutdown,
                ),
            ),
        ),
    )
    .await
    .expect("Server authority wait exceeded its attempt budget");
}

fn phases(authority: &Authority) -> Vec<TransactionPhase> {
    TransactionJournal::new(authority.persistence.clone())
        .load_transactions()
        .unwrap()
        .into_iter()
        .map(|record| record.phase())
        .collect()
}

async fn assert_retained(authority: &Authority, mock: &Mock, cache: &Cache, pending: &Pending) {
    assert_eq!(pending.len(), 1);
    assert_eq!(
        cache.commits(),
        0,
        "cache must never get ahead of authority"
    );
    assert_eq!(mock.count(), 1);
    let ticket = pending.values().next().unwrap().as_result().unwrap();
    assert!(ticket.is_current(authority.runtime.route_config()));
    assert_eq!(ticket.token.access_token, "rotated-build-access");
    drop(authority.persistence.try_writer_lock().unwrap());
    let mut independent = Pending::new();
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                authority.runtime.route_config(),
                &client(),
                Some(&authority.runtime),
                Some(&cache.pool),
                &mut independent,
                Duration::MAX,
                None,
            ),
        )
        .await;
    assert!(independent.is_empty());
    assert_eq!(mock.count(), 1, "timeout must not release single-flight");
}

#[tokio::test]
async fn server_authority_load_stall_keeps_ticket_and_recovery_retry_is_also_bounded() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, rotated(), 0).await;
    authority.backend.stall(Stall::Load);
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending, None).await;
    assert_retained(&authority, &mock, &cache, &pending).await;
    assert!(phases(&authority).is_empty());
    // The first failure marks recovery required; that recovery's own load must also be bounded.
    refresh(&authority, &mock, &cache, &mut pending, None).await;
    assert_retained(&authority, &mock, &cache, &pending).await;
    assert!(phases(&authority).is_empty());
    authority.backend.resume();
    refresh(&authority, &mock, &cache, &mut pending, None).await;
    assert!(pending.is_empty());
    assert_eq!(mock.count(), 1);
    assert_cache_matches_authority(&authority, &cache);
    assert_eq!(cache.commits(), 1);
    assert_eq!(phases(&authority), [TransactionPhase::Committed]);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_authority_timeout_before_activation_recovers_yaml_then_reuses_rotation() {
    let authority = Authority::new();
    let initial = authority.active().metadata().id().to_owned();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, rotated(), 0).await;
    authority.backend.stall(Stall::BeforeActivation);
    let mut pending = Pending::new();
    refresh(&authority, &mock, &cache, &mut pending, None).await;
    assert_retained(&authority, &mock, &cache, &pending).await;
    assert_eq!(authority.active().metadata().id(), initial);
    assert_eq!(phases(&authority), [TransactionPhase::YamlReplaced]);
    authority.backend.resume();
    refresh(&authority, &mock, &cache, &mut pending, None).await;
    assert!(pending.is_empty());
    let recovered_phases = phases(&authority);
    assert_eq!(recovered_phases.len(), 2);
    assert!(recovered_phases.contains(&TransactionPhase::Aborted));
    assert!(recovered_phases.contains(&TransactionPhase::Committed));
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(mock.count(), 1);
    assert_cache_matches_authority(&authority, &cache);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn server_authority_timeout_after_activation_recovers_same_revision_and_cache() {
    for rotate in [true, false] {
        let authority = Authority::new();
        let cache = Cache::new(true).await;
        authority.backend.bind_cache(&cache.pool);
        let mut reply = rotated();
        if !rotate {
            reply["refresh_token"] = serde_json::json!("synthetic-refresh");
        }
        reply["scope"] = serde_json::json!("profile");
        let mock = Mock::new(200, reply, 0).await;
        authority.backend.stall(Stall::AfterActivation);
        let mut pending = Pending::new();
        refresh(&authority, &mock, &cache, &mut pending, None).await;
        assert_retained(&authority, &mock, &cache, &pending).await;
        assert_eq!(phases(&authority), [TransactionPhase::YamlReplaced]);
        assert_eq!(
            authority.saved().api_key.as_deref(),
            Some("synthetic-build-token")
        );
        let active = authority.active();
        authority.backend.resume();
        refresh(&authority, &mock, &cache, &mut pending, None).await;
        assert!(pending.is_empty());
        assert_eq!(
            authority.runtime.route_config().snapshot().revision().id(),
            active.metadata().id()
        );
        assert_eq!(phases(&authority), [TransactionPhase::Committed]);
        assert_eq!(authority.backend.activations(), 1);
        assert_eq!(mock.count(), 1);
        assert_cache_matches_authority(&authority, &cache);
        let mirror: RouteConfigYaml =
            serde_yaml::from_str(&std::fs::read_to_string(&authority.routes).unwrap()).unwrap();
        assert_eq!(
            mirror.providers[0].credentials[0].grok_oauth,
            authority.saved().grok_oauth
        );
        let restarted = authority.restart().await;
        assert_eq!(
            restarted.route_config().snapshot().revision().id(),
            active.metadata().id()
        );
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn server_authority_stall_during_shutdown_returns_with_pending_for_final_flush() {
    let authority = Authority::new();
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, rotated(), 0).await;
    let shutdown = GatewayShutdownHandle::default();
    authority.backend.stall(Stall::AfterActivation);
    let mut pending = Pending::new();
    let request_shutdown = async {
        authority.backend.wait_stalled().await;
        shutdown.request("synthetic authority stall");
    };
    tokio::join!(
        refresh(&authority, &mock, &cache, &mut pending, Some(&shutdown)),
        request_shutdown
    );
    assert!(shutdown.is_requested());
    assert_retained(&authority, &mock, &cache, &pending).await;
    // A requested shutdown skips new work; the final flush may only persist this existing ticket.
    refresh(&authority, &mock, &cache, &mut pending, Some(&shutdown)).await;
    assert_eq!(pending.len(), 1);
    authority.backend.resume();
    assert!(
        pending
            .values_mut()
            .next()
            .unwrap()
            .write_to(Some(&authority.runtime), Some(&cache.pool))
            .await
    );
    assert_cache_matches_authority(&authority, &cache);
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 1);
    assert_eq!(phases(&authority), [TransactionPhase::Committed]);
    let owner = server_policy::owner(authority.runtime.route_config());
    pending.clear();
    assert!(owner
        .begin_refresh(Duration::MAX, "rotated-build-access")
        .is_some());
    mock.stop().await;
    cache.stop().await;
}
