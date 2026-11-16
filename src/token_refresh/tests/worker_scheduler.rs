//! Exercise the real periodic task and shutdown flush, with explicit notifications and awaited exit.
use super::{
    authority_fixture::{Authority, Stall},
    cache_wire::Cache,
    grok_wire::Mock,
    worker_authority::reply,
    *,
};

#[tokio::test]
async fn worker_real_scheduler_saves_authority_then_flushes_cache_on_shutdown() {
    for writable_cache in [true, false] {
        let authority = Authority::with_writable(false);
        let cache = Cache::new(writable_cache).await;
        authority.backend.bind_cache(&cache.pool);
        let mock = Mock::new(200, reply(), 0).await;
        let state = worker_state_fixture::state(&authority, &cache);
        assert!(Arc::ptr_eq(
            &state.route_config,
            authority.runtime.route_config()
        ));
        let controller = async {
            cache.wait_write_result().await;
            cache.allow_writes();
            state.shutdown.request("synthetic first-cycle completion");
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            super::super::grok::TEST_ISSUER.scope(mock.issuer.clone(), async {
                tokio::join!(
                    start_token_refresh_task(state.clone(), 3600, 300),
                    controller
                );
            }),
        )
        .await
        .expect("Worker scheduler did not exit");
        assert!(authority.runtime.coordinator().is_none());
        assert_eq!(
            authority.saved().api_key.as_deref(),
            Some("rotated-build-access")
        );
        assert_eq!(authority.backend.activations(), 1);
        assert_eq!(mock.count(), 1);
        assert_eq!(cache.commits(), 1);
        server_authority::assert_cache_matches_authority(&authority, &cache);
        mock.stop().await;
        cache.stop().await;
    }
}

#[tokio::test]
async fn worker_shutdown_flush_retries_authority_not_only_cache_after_interrupted_commit() {
    let authority = Authority::with_writable(false);
    let cache = Cache::new(true).await;
    authority.backend.bind_cache(&cache.pool);
    let mock = Mock::new(200, reply(), 0).await;
    let state = worker_state_fixture::state(&authority, &cache);
    authority.backend.stall(Stall::BeforeActivation);
    let controller = async {
        authority.backend.wait_stalled().await;
        // The in-flight future remains pending until its budget expires; later attempts may proceed.
        authority.backend.resume();
        state
            .shutdown
            .request("synthetic interrupted authority commit");
    };
    tokio::time::timeout(
        Duration::from_secs(5),
        SERVER_AUTHORITY_BUDGET.scope(
            Duration::from_millis(100),
            super::super::grok::TEST_ISSUER.scope(mock.issuer.clone(), async {
                tokio::join!(
                    start_token_refresh_task(state.clone(), 3600, 300),
                    controller
                );
            }),
        ),
    )
    .await
    .expect("Worker final authority flush did not exit");
    assert!(authority.runtime.coordinator().is_none());
    assert_eq!(
        authority.saved().api_key.as_deref(),
        Some("rotated-build-access")
    );
    assert_eq!(mock.count(), 1);
    assert_eq!(authority.backend.activations(), 1);
    assert_eq!(cache.commits(), 1);
    server_authority::assert_cache_matches_authority(&authority, &cache);
    mock.stop().await;
    cache.stop().await;
}
