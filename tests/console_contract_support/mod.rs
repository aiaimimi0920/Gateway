//! Management fixtures share synthetic auth, in-memory Redis and loopback probe setup.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::routing::get;
use axum::Router;
use http_body_util::BodyExt;
use neuro_gateway::concurrency::aimd::AimdConfig;
use neuro_gateway::concurrency::registry::ConcurrencyRegistry;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::console::{
    ConsoleAuthRuntime, RouteConfigRedisActivationOutcome, RouteConfigRedisBackend,
    RouteConfigRedisRevision, RouteConfigRedisStoreError, RouteConfigRuntime,
};
use neuro_gateway::credential_store::CredentialMemoryCache;
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use neuro_gateway::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use neuro_gateway::upstream::client::UpstreamClient;
use tower::ServiceExt;

pub const MANAGEMENT_TOKEN: &str = "console-management-contract-token";

#[derive(Clone)]
pub struct ProbeServerState {
    pub accepted_authorizations: Vec<String>,
    pub status: StatusCode,
    pub body: String,
}

#[derive(Clone, Copy)]
pub enum SensitiveCommitKind {
    Raw,
    Replace,
    Clear,
}

pub fn document(provider_id: &str, model: &str, api_key: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    api_key: {api_key}
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider_id}]
aliases:
  answer: {model}
"#
    ))
    .unwrap()
}

pub fn test_config() -> Config {
    Config {
        console: Default::default(),
        runtime_role: GatewayRuntimeRole::Standalone,
        port: 0,
        redis_url: "redis://127.0.0.1:1/15".to_string(),
        database_url: None,
        upstream_timeout_secs: 1,
        max_request_body_bytes: 1024 * 1024,
        max_body_chat_completions_bytes: 1024 * 1024,
        max_body_completions_bytes: 1024 * 1024,
        max_body_messages_bytes: 1024 * 1024,
        max_body_responses_bytes: 1024 * 1024,
        max_body_embeddings_bytes: 1024 * 1024,
        max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
        max_body_audio_speech_bytes: 1024 * 1024,
        max_body_search_bytes: 512 * 1024,
        max_body_fetch_bytes: 512 * 1024,
        max_body_research_bytes: 512 * 1024,
        max_body_images_generations_bytes: 1024 * 1024,
        max_body_images_edits_bytes: 1024 * 1024,
        max_body_music_bytes: 1024 * 1024,
        max_body_videos_bytes: 1024 * 1024,
        response_cache_ttl_secs: 300,
        response_cache_max_size_bytes: 512 * 1024,
        quota_pre_deduct_estimate_ratio: 1.2,
        usage_report_batch_size: 100,
        provider_probe_interval_secs: 30,
        log_level: "info".to_string(),
        gateway_api_key: Some("public-api-contract-secret".to_string()),
        gateway_api_key_secret: Some("hmac-contract-secret".to_string()),
        gateway_management_token: Some(MANAGEMENT_TOKEN.to_string()),
        gateway_keepalive_bearer_token: Some("keepalive-contract-secret".to_string()),
        default_project_id: "console-contract-project".to_string(),
        gateway_inbound_api_key_header_aliases: Vec::new(),
        provider_credential_folder_sync_enabled: false,
        provider_credential_folder_sync_root_dir: None,
        provider_credential_folder_sync_interval_secs: 30,
        provider_credential_folder_sync_watch_enabled: false,
        provider_credential_folder_sync_watch_debounce_millis: 1500,
        provider_credential_folder_sync_import_enabled: false,
        provider_credential_folder_sync_export_enabled: false,
        provider_credential_folder_sync_delete_missing: false,
        provider_credential_refresh_enabled: false,
        provider_credential_refresh_interval_secs: 3600,
        provider_credential_refresh_before_secs: 24 * 60 * 60,
        provider_credential_refresh_batch_limit: 100,
        provider_credential_refresh_lock_ttl_secs: 300,
        credential_stock_monitor_enabled: false,
        credential_stock_monitor_interval_secs: 60,
        credential_pool_automation: Default::default(),
        splitter_worker_executable_path: None,
        splitter_initial_worker_port: 1,
        splitter_ready_timeout_secs: 1,
        splitter_ready_poll_interval_millis: 10,
        splitter_reload_shutdown_timeout_secs: 1,
    }
}

pub async fn grant_console_secret_access(state: &Arc<AppState>) -> String {
    let response = build_router(Arc::clone(state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/confirm-secret-access")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "token": MANAGEMENT_TOKEN }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    body["grant"]
        .as_str()
        .expect("secret access response must include a grant")
        .to_string()
}

pub fn sensitive_commit_body(
    fixture: &ConsoleStateFixture,
    active: &RouteConfigYaml,
    kind: &SensitiveCommitKind,
) -> serde_json::Value {
    let (draft, secret_patches) = match kind {
        SensitiveCommitKind::Raw => (
            document("managed", "old-model", "raw-request-secret"),
            serde_json::json!([]),
        ),
        SensitiveCommitKind::Replace => (
            redact_route_document(active).unwrap().document,
            serde_json::json!([
                {
                    "path": "/providers/0/api_key",
                    "operation": "replace",
                    "value": "replacement-secret"
                }
            ]),
        ),
        SensitiveCommitKind::Clear => (
            redact_route_document(active).unwrap().document,
            serde_json::json!([
                { "path": "/providers/0/api_key", "operation": "clear" }
            ]),
        ),
    };
    serde_json::json!({
        "expectedRevision": fixture.state.route_config.snapshot().revision().id(),
        "document": draft,
        "secretPatches": secret_patches,
    })
}

pub async fn send_console_json(
    state: &Arc<AppState>,
    method: Method,
    endpoint: &str,
    body: serde_json::Value,
    secret_grant: Option<&str>,
    origin: Option<&str>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(endpoint)
        .header("x-management-token", MANAGEMENT_TOKEN)
        .header("content-type", "application/json");
    if let Some(secret_grant) = secret_grant {
        request = request.header("x-secret-grant", secret_grant);
    }
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    build_router(Arc::clone(state))
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

pub async fn probe_credential(
    state: &Arc<AppState>,
    secret_grant: &str,
    credential_id: &str,
) -> axum::response::Response {
    build_router(Arc::clone(state))
        .oneshot(
            Request::post(format!(
                "/v1/internal/gateway/console/credentials/{credential_id}/probe"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .header("x-secret-grant", secret_grant)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

pub async fn probe_provider(
    state: &Arc<AppState>,
    secret_grant: &str,
    provider_id: &str,
) -> axum::response::Response {
    build_router(Arc::clone(state))
        .oneshot(
            Request::post(format!(
                "/v1/internal/gateway/console/providers/{provider_id}/probe"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .header("x-secret-grant", secret_grant)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

pub async fn spawn_probe_server(state: ProbeServerState) -> (String, tokio::task::JoinHandle<()>) {
    async fn models(
        AxumState(state): AxumState<ProbeServerState>,
        headers: HeaderMap,
    ) -> (StatusCode, String) {
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !state
            .accepted_authorizations
            .iter()
            .any(|expected| expected == authorization)
        {
            return (
                StatusCode::UNAUTHORIZED,
                "unexpected authorization".to_string(),
            );
        }
        (state.status, state.body)
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind credential probe fixture");
    let address = listener
        .local_addr()
        .expect("credential probe fixture address");
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/v1/models", get(models))
                .with_state(state),
        )
        .await
        .expect("serve credential probe fixture");
    });
    (format!("http://{address}/v1"), server)
}

pub async fn parse_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response must be JSON")
}

pub struct ConsoleStateFixture {
    _temp: TestDirectory,
    pub state: Arc<AppState>,
}

impl ConsoleStateFixture {
    pub fn new(active: RouteConfigYaml, with_runtime: bool) -> Self {
        let temp = TestDirectory::new("console-management");
        let routes = temp.path().join("routes.yaml");
        fs::write(
            &routes,
            neuro_gateway::console::document::canonicalize_route_document(&active)
                .unwrap()
                .canonical_yaml(),
        )
        .unwrap();
        let store = Arc::new(RouteConfigStore::from_document(active).unwrap());
        let runtime = with_runtime.then(|| {
            let persistence =
                neuro_gateway::console::RouteConfigPersistence::for_test(temp.path(), &routes)
                    .unwrap();
            Arc::new(RouteConfigRuntime::with_backend(
                Arc::clone(&store),
                persistence,
                Arc::new(FakeRedisBackend::default()),
                true,
            ))
        });
        Self {
            state: build_state(Arc::clone(&store), runtime),
            _temp: temp,
        }
    }

    pub async fn commit_route_document(
        &self,
        document: RouteConfigYaml,
        message: Option<&str>,
    ) -> Arc<neuro_gateway::routing::config::RouteConfigSnapshot> {
        let runtime = self
            .state
            .route_config_runtime
            .as_ref()
            .expect("runtime must be configured");
        let current_revision = self
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_string();
        runtime
            .commit_document(&current_revision, document, message.map(str::to_string))
            .await
            .expect("commit route document")
    }
}

pub fn build_state(
    route_config: Arc<RouteConfigStore>,
    route_config_runtime: Option<Arc<RouteConfigRuntime>>,
) -> Arc<AppState> {
    let config = test_config();
    let redis_pool = deadpool_redis::Config::from_url(config.redis_url.clone())
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create lazy Redis test pool");
    let upstream_timeout_secs = config.upstream_timeout_secs;
    let provider_credential_folder_sync_enabled = config.provider_credential_folder_sync_enabled;
    let console_auth = test_console_auth_runtime(config.gateway_management_token.clone());

    Arc::new(AppState {
        config,
        redis_pool,
        pg_pool: None,
        upstream_client: UpstreamClient::new(upstream_timeout_secs),
        concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
        auth_adapters: Vec::new(),
        filter_config: None,
        route_config,
        route_config_runtime,
        console_auth,
        credential_cache: CredentialMemoryCache::new(30),
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
        credential_pool_automation: Arc::new(
            neuro_gateway::credential_pool_automation::CredentialPoolAutomationRuntime::disabled(),
        ),
    })
}

pub fn test_console_auth_runtime(env_management_token: Option<String>) -> Arc<ConsoleAuthRuntime> {
    let temp = std::env::temp_dir().join(format!(
        "gateway-console-auth-management-{}",
        uuid::Uuid::new_v4()
    ));
    let console = neuro_gateway::console::ConsoleConfig::from_values(
        neuro_gateway::console::ConsoleConfigValues {
            state_dir: Some(temp.clone()),
            routes_file: Some(temp.join("routes.yaml")),
            ..Default::default()
        },
    )
    .unwrap();
    Arc::new(ConsoleAuthRuntime::new(&console, env_management_token).unwrap())
}

#[derive(Clone, Debug, Default)]
struct FakeRedisBackend {
    state: Arc<Mutex<FakeRedisState>>,
}

#[async_trait]
impl RouteConfigRedisBackend for FakeRedisBackend {
    async fn store_revision(
        &self,
        revision: &RouteConfigRedisRevision,
    ) -> Result<(), RouteConfigRedisStoreError> {
        self.state
            .lock()
            .unwrap()
            .revisions
            .insert(revision.metadata().id().to_string(), revision.clone());
        Ok(())
    }

    async fn store_prepared_transaction(
        &self,
        _record: &neuro_gateway::console::TransactionRecord,
    ) -> Result<(), RouteConfigRedisStoreError> {
        Ok(())
    }

    async fn activate_revision(
        &self,
        _expected_active_revision: Option<&str>,
        revision: &RouteConfigRedisRevision,
        _prepared_record: &neuro_gateway::console::TransactionRecord,
        _activated_record: &neuro_gateway::console::TransactionRecord,
    ) -> Result<RouteConfigRedisActivationOutcome, RouteConfigRedisStoreError> {
        self.state.lock().unwrap().active_revision = Some(revision.clone());
        Ok(RouteConfigRedisActivationOutcome::Activated)
    }

    async fn load_active_revision(
        &self,
    ) -> Result<Option<RouteConfigRedisRevision>, RouteConfigRedisStoreError> {
        Ok(self.state.lock().unwrap().active_revision.clone())
    }
}

#[derive(Clone, Debug, Default)]
struct FakeRedisState {
    revisions: BTreeMap<String, RouteConfigRedisRevision>,
    active_revision: Option<RouteConfigRedisRevision>,
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gateway-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
