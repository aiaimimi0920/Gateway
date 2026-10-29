//! All credentials are synthetic; fixtures use TEMP and loopback-only OAuth endpoints.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigPersistence};
use crate::routing::config::RouteConfigYaml;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

mod lifecycle;
mod transport;

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
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let mut pending = Pending::new();
    fixture.refresh(&mut pending).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    let candidate = fixture
        .runtime
        .route_config()
        .resolve_candidates(Some("test-model"));
    assert_eq!(candidate[0].payload.api_key, "new-access");
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
