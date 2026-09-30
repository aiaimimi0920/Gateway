use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use neuro_gateway::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

mod support;
use support::build_test_app_state;

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

fn test_config(state_dir: &Path) -> Config {
    Config {
        console: neuro_gateway::console::ConsoleConfig::from_values(
            neuro_gateway::console::ConsoleConfigValues {
                state_dir: Some(state_dir.to_path_buf()),
                routes_file: Some(state_dir.join("routes.yaml")),
                ..Default::default()
            },
        )
        .unwrap(),
        runtime_role: GatewayRuntimeRole::Standalone,
        storage_mode: Default::default(),
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
        gateway_management_token: None,
        gateway_keepalive_bearer_token: Some("keepalive-contract-secret".to_string()),
        default_project_id: "console-session-project".to_string(),
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

#[tokio::test]
async fn bootstrap_status_requires_no_auth_and_reports_when_setup_is_needed() {
    let fixture = SessionFixture::new(None);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::get("/v1/internal/gateway/console/bootstrap/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = parse_json(response).await;
    assert_eq!(body["needsBootstrap"], true);
    assert_eq!(body["managementConfigured"], false);
    assert_eq!(body["environmentOverride"], false);
}

#[tokio::test]
async fn bootstrap_persists_argon_hash_and_session_verify_accepts_the_new_token() {
    let fixture = SessionFixture::new(None);

    let bootstrap = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "token": "development-secret" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(bootstrap.status(), StatusCode::OK);
    let admin_json = fs::read_to_string(fixture.admin_path()).expect("admin.json");
    assert!(!admin_json.contains("development-secret"));
    assert!(admin_json.contains("$argon2id$"));

    let verify = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/verify")
                .header("x-management-token", "development-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify.status(), StatusCode::OK);
    let body = parse_json(verify).await;
    assert_eq!(body["role"], "administrator");
    assert_eq!(body["secretAccessGranted"], false);
    assert_eq!(
        body["activeRevision"].as_str(),
        Some(fixture.state.route_config.snapshot().revision().id())
    );
}

#[tokio::test]
async fn bootstrap_is_loopback_only_and_rejects_remote_clients() {
    let fixture = SessionFixture::new(None);

    let response = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/bootstrap")
                .header("content-type", "application/json")
                .header("x-gateway-client-ip", "192.0.2.10")
                .body(Body::from(
                    serde_json::json!({ "token": "development-secret" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = parse_json(response).await;
    assert_eq!(
        body["error"]["code"].as_str(),
        Some("console_bootstrap_loopback_only")
    );
}

#[tokio::test]
async fn rotate_invalidates_the_old_token_and_accepts_the_new_token() {
    let fixture = SessionFixture::new(None);
    bootstrap_token(&fixture.state, "bootstrap-secret").await;

    let rotate = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/rotate")
                .header("x-management-token", "bootstrap-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "newToken": "replacement-secret" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rotate.status(), StatusCode::OK);

    let old_verify = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/verify")
                .header("x-management-token", "bootstrap-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_verify.status(), StatusCode::UNAUTHORIZED);

    let new_verify = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/verify")
                .header("x-management-token", "replacement-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_verify.status(), StatusCode::OK);
}

#[tokio::test]
async fn secret_access_confirmation_and_logout_use_the_console_session_routes() {
    let fixture = SessionFixture::new(None);
    bootstrap_token(&fixture.state, "bootstrap-secret").await;

    let confirm = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/confirm-secret-access")
                .header("x-management-token", "bootstrap-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "token": "bootstrap-secret" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(confirm.status(), StatusCode::OK);
    let confirm_body = parse_json(confirm).await;
    assert!(confirm_body["grant"].as_str().is_some());
    assert!(confirm_body["expiresAt"].as_str().is_some());

    let logout = build_router(Arc::clone(&fixture.state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/logout")
                .header("x-management-token", "bootstrap-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(logout.status(), StatusCode::OK);
    let logout_body = parse_json(logout).await;
    assert_eq!(logout_body["success"], true);
}

async fn parse_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response must be JSON")
}

struct SessionFixture {
    _temp: TestDirectory,
    state: Arc<AppState>,
}

impl SessionFixture {
    fn new(initial_token: Option<&str>) -> Self {
        let temp = TestDirectory::new("console-session");
        let config = test_config(temp.path());
        fs::write(
            &config.console.routes_file,
            b"providers: []\nmodel_routes: []\naliases: {}\n",
        )
        .unwrap();
        let route_config =
            RouteConfigStore::from_document(document("managed", "gpt-5.4", "live-secret")).unwrap();
        let state = build_test_app_state(config, route_config, None);
        let _ = initial_token;
        Self { _temp: temp, state }
    }

    fn admin_path(&self) -> PathBuf {
        self.state
            .config
            .console
            .state_dir
            .join("console")
            .join("admin.json")
    }
}

async fn bootstrap_token(state: &Arc<AppState>, token: &str) {
    let response = build_router(Arc::clone(state))
        .oneshot(
            Request::post("/v1/internal/gateway/console/bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "token": token }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
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
