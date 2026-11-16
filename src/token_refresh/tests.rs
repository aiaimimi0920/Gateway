//! All credentials are synthetic; fixtures use TEMP and loopback-only OAuth endpoints.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigPersistence};
use crate::routing::config::RouteConfigYaml;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(feature = "line-accio-web-reverse-api")]
mod accio_cache;
#[cfg(feature = "line-accio-web-reverse-api")]
mod accio_fixture;
mod accio_guard;
#[cfg(feature = "line-accio-web-reverse-api")]
mod accio_lifecycle;
mod accio_material;
#[cfg(feature = "line-accio-web-reverse-api")]
mod accio_recovery;
mod authority_fixture;
mod cache_recovery;
mod cache_recovery_real_redis;
mod cache_recovery_safety;
mod cache_target_tests;
mod cache_wire;
mod dim_cache;
mod dim_fixture;
mod dim_lifecycle;
mod dim_material;
mod durable_ack;
mod durable_backend;
mod durable_gate;
mod durable_real_redis;
mod durable_recovery;
mod grok_context;
mod grok_lifecycle;
mod grok_wire;
#[cfg(feature = "line-kiro-official-vendor-api")]
mod kiro_cache;
#[cfg(feature = "line-kiro-official-vendor-api")]
mod kiro_fixture;
mod kiro_guard;
#[cfg(feature = "line-kiro-official-vendor-api")]
mod kiro_lifecycle;
mod kiro_material;
#[cfg(feature = "line-kiro-official-vendor-api")]
mod kiro_runtime;
mod lifecycle;
mod lobsterai_fixture;
mod lobsterai_lifecycle;
mod lobsterai_material;
mod lobsterai_reauthorization;
mod lobsterai_recovery;
mod nous_cache;
mod nous_config;
mod nous_fixture;
mod nous_lifecycle;
mod nous_material;
#[cfg(feature = "line-nous-portal-oauth")]
mod nous_probe;
mod nous_recovery;
mod nous_scheduler_fixture;
mod nous_server;
mod nous_target;
mod server_authority;
mod server_authority_recovery_owner;
mod server_authority_timeout;
mod server_persistence;
mod server_policy;
mod transport;
#[cfg(feature = "line-google-agent-platform-official")]
mod vertex_cache;
#[cfg(feature = "line-google-agent-platform-official")]
mod vertex_fixture;
#[cfg(feature = "line-google-agent-platform-official")]
mod vertex_lifecycle;
#[cfg(feature = "line-google-agent-platform-official")]
mod vertex_material;
mod worker_authority;
mod worker_projection;
mod worker_recovery;
mod worker_scheduler;
mod worker_state_fixture;

tokio::task_local! {
    pub(super) static SERVER_AUTHORITY_BUDGET: Duration;
}

struct MockOAuth {
    url: String,
    calls: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for MockOAuth {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MockOAuth {
    async fn new(status: u16, body: &str, delay_ms: u64) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let hits = calls.clone();
        let body = body.to_string();
        let router = axum::Router::new().route(
            "/token",
            axum::routing::post(move || {
                let calls = hits.clone();
                let body = body.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    (axum::http::StatusCode::from_u16(status).unwrap(), body)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { url, calls, task }
    }

    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    async fn wait_called(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}

struct LocalFixture {
    config: ConsoleConfig,
    runtime: RouteConfigRuntime,
    root: PathBuf,
}

impl LocalFixture {
    async fn new(endpoint: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gateway-oauth-refresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        let document: RouteConfigYaml = serde_json::from_value(serde_json::json!({
            "providers": [{"id":"oauth", "base_url":"https://example.com/v1", "credentials":[{
                "id":"account", "api_key":"old-access", "refresh_token":"old-refresh",
                "refresh_endpoint":endpoint, "refresh_client_id":"fixture-client",
                "expires_at":"2020-01-01T00:00:00Z", "token_expires_in_secs":3600,
                "headers":{"Chatgpt-Account-Id":"fixture-account"}, "supported_models":["test-model"]
            }]}], "model_routes":[{"pattern":"test-model", "provider_ids":["oauth"]}]
        })).unwrap();
        std::fs::write(&routes, serde_yaml::to_string(&document).unwrap()).unwrap();
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(routes),
            ..Default::default()
        })
        .unwrap();
        let runtime = RouteConfigRuntime::new_local(&config).unwrap();
        runtime.recover_startup().await.unwrap();
        Self {
            config,
            runtime,
            root,
        }
    }

    async fn refresh(&self, pending: &mut Pending) {
        refresh_once(
            self.runtime.route_config(),
            &client(),
            Some(&self.runtime),
            None,
            pending,
            Duration::from_secs(300),
            None,
        )
        .await;
    }

    fn saved(&self) -> crate::routing::config::ProviderCredentialYaml {
        self.runtime.route_config().snapshot().document().providers[0].credentials[0].clone()
    }

    fn writer_lock(&self) -> crate::console::WriterLockGuard {
        RouteConfigPersistence::new(&self.config.state_dir, &self.config.routes_file)
            .unwrap()
            .try_writer_lock()
            .unwrap()
    }
}

fn client() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}
const ROTATED: &str =
    r#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#;

#[tokio::test]
async fn local_refresh_rotates_persists_and_survives_restart_without_redis() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let mut pending = Pending::new();
    fixture.refresh(&mut pending).await;
    assert_eq!(mock.count(), 1);
    assert!(pending.is_empty());
    let saved = fixture.saved();
    assert_eq!(saved.api_key.as_deref(), Some("new-access"));
    assert_eq!(saved.refresh_token.as_deref(), Some("new-refresh"));
    assert_eq!(saved.headers["Chatgpt-Account-Id"], "fixture-account");
    let restarted = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    restarted.recover_startup().await.unwrap();
    assert_eq!(
        restarted.route_config().snapshot().document().providers[0].credentials[0].expires_at,
        saved.expires_at
    );
    let candidate = restarted
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(candidate[0].payload.api_key, "new-access");
    fixture.refresh(&mut pending).await;
    assert_eq!(mock.count(), 1, "not due after reload");
}

#[tokio::test]
async fn rotated_material_retries_disk_only_after_writer_lock_conflict() {
    let mock = MockOAuth::new(200, ROTATED, 100).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(fixture.refresh(&mut pending), async {
        mock.wait_called().await;
        fixture.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    let candidate = fixture
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(
        candidate[0].payload.api_key, "old-access",
        "Result must be durable before apply"
    );
    fixture.refresh(&mut pending).await;
    assert_eq!(mock.count(), 1, "do not reuse a rotated refresh token");
    drop(guard);
    fixture.refresh(&mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(
        fixture.saved().refresh_token.as_deref(),
        Some("new-refresh")
    );
    assert_eq!(mock.count(), 1);
    assert!(fixture.root.exists());
}
