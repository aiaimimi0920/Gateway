//! Crash-window proof: rebuilt runtimes must repair cache from admitted authority without a grant.
use super::{cache_wire::Cache, grok_wire::Mock, *};

pub(super) async fn refresh(
    runtime: &RouteConfigRuntime,
    mock: &Mock,
    cache: &Cache,
    pending: &mut Pending,
) {
    super::super::grok::TEST_ISSUER
        .scope(
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
        )
        .await;
}

#[tokio::test]
async fn restarted_runtime_repairs_published_rotation_cache_without_refreshing_fresh_token() {
    let authority = authority_fixture::Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending
            .values()
            .next()
            .unwrap()
            .as_result()
            .unwrap()
            .record
            .phase(),
        crate::console::refresh_grants::GrantPhase::Published
    );
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(cache.commits(), 0);
    drop(pending); // Lose the process-local recovery ticket; do not run a graceful flush.
    let restarted = authority.restart().await;
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(
        mock.count(),
        1,
        "cache repair must not exchange another grant"
    );
    assert_eq!(
        cache.commits(),
        1,
        "fresh authority must still repair old cache"
    );
    assert_eq!(cache.value()["api_key"], "rotated-build-access");
    assert_eq!(cache.value()["quota_remaining_tokens"], 42);
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(cache.commits(), 1, "repair is idempotent");
    assert_eq!(authority.backend.activations(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn restarted_runtime_recovers_activation_before_published_and_repairs_cache() {
    let authority = authority_fixture::Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    authority
        .backend
        .stall(authority_fixture::Stall::AfterActivation);
    let mut pending = Pending::new();
    SERVER_AUTHORITY_BUDGET
        .scope(
            Duration::from_millis(80),
            refresh(&authority.runtime, &mock, &cache, &mut pending),
        )
        .await;
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending
            .values()
            .next()
            .unwrap()
            .as_result()
            .unwrap()
            .record
            .phase(),
        crate::console::refresh_grants::GrantPhase::Result
    );
    assert_eq!(authority.backend.activations(), 1);
    drop(pending);
    authority.backend.resume();
    let restarted = authority.restart().await;
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(
        cache.commits(),
        1,
        "activation recovery must not lose cache obligation"
    );
    assert_eq!(
        cache.value()["account_payload"]["refresh_token"],
        "rotated-build-refresh"
    );
    assert_eq!(authority.backend.activations(), 1);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn restarted_cache_outage_defers_a_due_successor_until_repair_succeeds() {
    let authority = authority_fixture::Authority::new();
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    drop(pending);
    let restarted = authority.restart().await;
    server_policy::owner(restarted.route_config()).force_due();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(
        mock.count(),
        1,
        "cache outage must retain the predecessor CAS chain"
    );
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert_eq!(mock.count(), 2, "a due successor is allowed after repair");
    assert_eq!(cache.value()["api_key"], "rotated-build-access");
    assert_eq!(cache.value()["quota_remaining_tokens"], 42);
    assert_eq!(authority.backend.activations(), 2);
    mock.stop().await;
    cache.stop().await;
}

#[tokio::test]
async fn restarted_read_only_worker_repairs_cache_through_its_private_refresh_owner() {
    let authority = authority_fixture::Authority::with_writable(false);
    let mock = Mock::new(200, grok_wire::rotated(), 0).await;
    let cache = Cache::new(false).await;
    authority.backend.bind_cache(&cache.pool);
    let mut pending = Pending::new();
    refresh(&authority.runtime, &mock, &cache, &mut pending).await;
    assert_eq!(pending.len(), 1);
    drop(pending);
    let restarted = authority.rebuilt(false);
    assert!(restarted.coordinator().is_none());
    cache.allow_writes();
    refresh(&restarted, &mock, &cache, &mut Pending::new()).await;
    assert!(restarted.coordinator().is_none());
    assert_eq!(mock.count(), 1);
    assert_eq!(cache.commits(), 1);
    assert_eq!(cache.value()["api_key"], "rotated-build-access");
    mock.stop().await;
    cache.stop().await;
}
