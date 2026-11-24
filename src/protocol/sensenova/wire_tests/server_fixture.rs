//! Shared isolated PostgreSQL/Redis bootstrap for SenseNova scheduled and public traffic tests.
use super::*;

pub(super) const PUBLIC: &str = "sensenova-server-public-synthetic";

pub(super) async fn build(
    scope: &str,
    instance: &str,
    peer: &peer::Peer,
    document: Value,
) -> Arc<AppState> {
    let root = std::path::PathBuf::from(std::env::var("SENSENOVA_TEST_EVIDENCE_ROOT").unwrap());
    assert!(root.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"));
    let root = root.join(scope).join(instance);
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Server;
    let database_url = std::env::var("SENSENOVA_TEST_DATABASE_URL").unwrap();
    let db = url::Url::parse(&database_url).unwrap();
    assert_eq!(db.host_str(), Some("127.0.0.1"));
    assert_eq!(db.path(), "/gateway_fixture");
    assert!(db.port().is_some_and(|port| port != 5432));
    config.database_url = Some(database_url);
    config.redis_url = std::env::var("SENSENOVA_TEST_REDIS_URL").unwrap();
    let redis = url::Url::parse(&config.redis_url).unwrap();
    assert_eq!(redis.host_str(), Some("127.0.0.1"));
    assert!(redis.port().is_some_and(|port| port != 6379));
    config.gateway_api_key = Some(PUBLIC.into());
    config.gateway_api_key_secret = None;
    config.default_project_id = "platform-default-project".into();
    config.console.routes_file = root.join("routes.yaml");
    config.console.state_dir = root.join("state");
    config.console.redis_namespace = scope.into();
    std::fs::write(
        &config.console.routes_file,
        serde_yaml::to_string(&document).unwrap(),
    )
    .unwrap();
    let mut state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(state.local_runtime.is_none() && state.pg_pool.is_some());
    Arc::get_mut(&mut state).unwrap().upstream_client.http = peer.client();
    state
}

pub(super) fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
    // Server bootstrap and the worker futures need a bounded larger stack on Windows.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(Box::pin(test()))
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

pub(super) async fn close(state: Arc<AppState>) {
    state.shutdown.request("sensenova-server-fixture");
    state.pg_pool.as_ref().unwrap().close().await;
    state
        .upstream_client
        .freebuff
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
}
