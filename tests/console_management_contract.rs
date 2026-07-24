use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use neuro_gateway::concurrency::aimd::AimdConfig;
use neuro_gateway::concurrency::registry::ConcurrencyRegistry;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::console::{
    RouteConfigRedisActivationOutcome, RouteConfigRedisBackend, RouteConfigRedisRevision,
    RouteConfigRedisStoreError, RouteConfigRuntime,
};
use neuro_gateway::credential_store::CredentialMemoryCache;
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use neuro_gateway::state::{
    AppState, GatewayLifecycleState, GatewayShutdownHandle, ProviderCredentialFolderSyncRuntime,
};
use neuro_gateway::upstream::client::UpstreamClient;
use tower::ServiceExt;

const MANAGEMENT_TOKEN: &str = "console-management-contract-token";

fn document(provider_id: &str, model: &str, api_key: &str) -> RouteConfigYaml {
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

fn test_config() -> Config {
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
        splitter_worker_executable_path: None,
        splitter_initial_worker_port: 1,
        splitter_ready_timeout_secs: 1,
        splitter_ready_poll_interval_millis: 10,
        splitter_reload_shutdown_timeout_secs: 1,
    }
}

#[tokio::test]
async fn route_config_management_route_requires_management_authentication() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn route_config_management_route_returns_redacted_active_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(fixture.state.route_config.snapshot().revision().id())
    );
    assert_eq!(body["routeConfig"]["source"].as_str(), Some("database"));
    assert_eq!(body["routeConfig"]["mutationSupported"], true);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_console_alias_returns_no_store_and_etag() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok()),
        Some(format!("\"{revision_id}\"").as_str())
    );
}

#[tokio::test]
async fn route_config_management_commit_applies_secret_patches_and_updates_runtime() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["new-model".to_string()];
    draft.model_routes[0].pattern = "new-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ],
                        "message": "update route config"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["document"]["aliases"]["answer"].as_str(),
        Some("new-model")
    );
    assert_eq!(
        fixture
            .state
            .route_config
            .snapshot()
            .resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
    assert_eq!(
        fixture.state.route_config.snapshot().source(),
        neuro_gateway::routing::config::ActiveConfigSource::Redis
    );
}

#[tokio::test]
async fn route_config_console_validate_returns_redacted_candidate_document() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].supported_models = vec!["candidate-model".to_string()];
    draft.model_routes[0].pattern = "candidate-model".to_string();
    draft
        .aliases
        .insert("answer".to_string(), "candidate-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/route-config/validate")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["validation"]["document"]["aliases"]["answer"].as_str(),
        Some("candidate-model")
    );
    assert_eq!(body["validation"]["requiresRepair"], false);
    assert_ne!(
        body["validation"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
}

#[tokio::test]
async fn route_config_management_commit_rejects_stale_revision_with_conflict() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "different-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": "r0-deadbeefcafe",
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_conflict")
    );
}

#[tokio::test]
async fn route_config_management_commit_reports_runtime_unavailable_when_not_configured() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active, false);
    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": fixture.state.route_config.snapshot().revision().id(),
                        "document": document("managed", "new-model", "fresh-secret")
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_runtime_unavailable")
    );
}

#[tokio::test]
async fn route_config_console_put_rejects_if_match_mismatch() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let current_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft
        .aliases
        .insert("answer".to_string(), "new-model".to_string());

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::put("/v1/internal/gateway/console/route-config")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("if-match", "\"r99-deadbeefcafe\"")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "expectedRevision": current_revision,
                        "document": draft,
                        "secretPatches": [
                            { "path": "/providers/0/api_key", "operation": "keep" }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_if_match_mismatch")
    );
}

#[tokio::test]
async fn route_config_management_revisions_list_returns_archived_history() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);
    let snapshot = fixture
        .commit_route_document(
            document("managed", "new-model", "live-secret"),
            Some("archive history"),
        )
        .await;

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    let revisions = body["revisions"].as_array().expect("revisions array");
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        revisions[0]["revision"]["id"].as_str(),
        Some(snapshot.revision().id())
    );
    assert_eq!(revisions[0]["active"], true);
    assert_eq!(revisions[0]["hasArchive"], true);
    assert_eq!(
        revisions[0]["revision"]["message"].as_str(),
        Some("archive history")
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_redacted_current_snapshot() {
    let active = document("managed", "old-model", "live-secret");
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let revision_id = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get(format!(
                "/v1/internal/gateway/route-config/revisions/{revision_id}"
            ))
            .header("x-management-token", MANAGEMENT_TOKEN)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(
        body["routeConfig"]["revision"]["id"].as_str(),
        Some(revision_id.as_str())
    );
    assert_eq!(body["active"], true);
    assert_eq!(body["hasArchive"], false);
    assert_ne!(
        body["routeConfig"]["document"]["providers"][0]["api_key"].as_str(),
        Some("live-secret")
    );
    assert_eq!(
        redact_route_document(&active).unwrap().secrets.len(),
        body["routeConfig"]["secrets"].as_array().unwrap().len()
    );
}

#[tokio::test]
async fn route_config_management_revision_detail_returns_not_found_for_unknown_revision() {
    let fixture = ConsoleStateFixture::new(document("managed", "old-model", "live-secret"), true);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/route-config/revisions/r99-deadbeefcafe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_revision_not_found")
    );
}

async fn parse_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response must be JSON")
}

struct ConsoleStateFixture {
    _temp: TestDirectory,
    state: Arc<AppState>,
}

impl ConsoleStateFixture {
    fn new(active: RouteConfigYaml, with_runtime: bool) -> Self {
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

    async fn commit_route_document(
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

fn build_state(
    route_config: Arc<RouteConfigStore>,
    route_config_runtime: Option<Arc<RouteConfigRuntime>>,
) -> Arc<AppState> {
    let config = test_config();
    let redis_pool = deadpool_redis::Config::from_url(config.redis_url.clone())
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .expect("create lazy Redis test pool");
    let upstream_timeout_secs = config.upstream_timeout_secs;
    let provider_credential_folder_sync_enabled = config.provider_credential_folder_sync_enabled;

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
        credential_cache: CredentialMemoryCache::new(30),
        lifecycle: GatewayLifecycleState::default(),
        shutdown: GatewayShutdownHandle::default(),
        provider_credential_folder_sync: ProviderCredentialFolderSyncRuntime::new(
            provider_credential_folder_sync_enabled,
        ),
    })
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
