//! Smoke tests for the gateway.
//!
//! These tests start the gateway with a test config and verify the key
//! endpoints work correctly. They do NOT require external services (no Redis,
//! no upstream providers).

use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt; // for `oneshot`

use neuro_gateway::config::Config;
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigStore;

mod support;
use support::build_test_app_state;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build an axum Router backed by a test AppState.
///
/// - `gateway_api_key` is set to `"test-key"` so auth is enforced.
/// - No Redis-dependent features are exercised (pool is lazy).
/// - No route providers are configured (model requests will fail at route stage).
fn test_app() -> axum::Router {
    test_app_with_config(1024 * 1024, 512 * 1024, Vec::new())
}

fn test_app_with_limits(chat_limit: usize, search_limit: usize) -> axum::Router {
    test_app_with_config(chat_limit, search_limit, Vec::new())
}

fn test_app_with_aliases(aliases: Vec<&str>) -> axum::Router {
    test_app_with_config(
        1024 * 1024,
        512 * 1024,
        aliases.into_iter().map(str::to_ascii_lowercase).collect(),
    )
}

fn test_app_with_config(
    chat_limit: usize,
    search_limit: usize,
    inbound_aliases: Vec<String>,
) -> axum::Router {
    let config = Config {
        console: Default::default(),
        runtime_role: neuro_gateway::config::GatewayRuntimeRole::Standalone,
        port: 0,
        redis_url: std::env::var("GATEWAY_SMOKE_TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
        database_url: None,
        upstream_timeout_secs: 30,
        max_request_body_bytes: chat_limit,
        max_body_chat_completions_bytes: chat_limit,
        max_body_completions_bytes: chat_limit,
        max_body_messages_bytes: chat_limit,
        max_body_responses_bytes: chat_limit,
        max_body_embeddings_bytes: chat_limit,
        max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
        max_body_audio_speech_bytes: chat_limit,
        max_body_search_bytes: search_limit,
        max_body_fetch_bytes: search_limit,
        max_body_research_bytes: search_limit,
        max_body_images_generations_bytes: chat_limit,
        max_body_images_edits_bytes: chat_limit,
        max_body_music_bytes: chat_limit,
        max_body_videos_bytes: chat_limit,
        response_cache_ttl_secs: 300,
        response_cache_max_size_bytes: 512 * 1024,
        quota_pre_deduct_estimate_ratio: 1.2,
        usage_report_batch_size: 100,
        provider_probe_interval_secs: 30,
        log_level: "info".to_string(),
        gateway_api_key: Some("test-key".to_string()),
        gateway_api_key_secret: None,
        gateway_management_token: None,
        gateway_keepalive_bearer_token: None,
        default_project_id: "platform-default-project".to_string(),
        gateway_inbound_api_key_header_aliases: inbound_aliases,
        provider_credential_folder_sync_enabled: false,
        provider_credential_folder_sync_root_dir: None,
        provider_credential_folder_sync_interval_secs: 30,
        provider_credential_folder_sync_watch_enabled: true,
        provider_credential_folder_sync_watch_debounce_millis: 1500,
        provider_credential_folder_sync_import_enabled: true,
        provider_credential_folder_sync_export_enabled: true,
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
        splitter_ready_timeout_secs: 120,
        splitter_ready_poll_interval_millis: 500,
        splitter_reload_shutdown_timeout_secs: 600,
    };

    let state = build_test_app_state(config, RouteConfigStore::new(), None);

    build_router(state)
}

fn oversized_chat_request_bytes(target_len: usize) -> Vec<u8> {
    serde_json::json!({
        "model": "test",
        "messages": [{
            "role": "user",
            "content": "x".repeat(target_len)
        }]
    })
    .to_string()
    .into_bytes()
}

/// Parse a response body as JSON.
async fn parse_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let text = String::from_utf8_lossy(&bytes);
        panic!("Response body is not valid JSON: {}", text);
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn healthz_returns_json_200() {
    let app = test_app();
    let resp = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp).await;
    assert_eq!(body["status"], "ok");
}

#[test]
fn binary_help_does_not_require_runtime_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_gateway"))
        .arg("--help")
        .env_remove("GATEWAY_REDIS_URL")
        .env_remove("PORT")
        .env_remove("GATEWAY_RUNTIME_ROLE")
        .env_remove("GATEWAY_API_KEY")
        .output()
        .expect("run gateway --help");

    assert!(
        output.status.success(),
        "--help should exit successfully, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage") || stdout.contains("USAGE"),
        "--help should print usage text, stdout={stdout}"
    );
}

#[tokio::test]
async fn healthz_no_auth_required() {
    // healthz should work without any auth header.
    let app = test_app();
    let resp = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn models_returns_list() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/v1/models")
                .header("authorization", "Bearer test-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = parse_body(resp).await;
    assert_eq!(body["object"], "list");
}

#[tokio::test]
async fn models_accepts_api_key_header() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::get("/v1/models")
                .header("api-key", "test-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn models_accepts_configured_alias_header() {
    let app = test_app_with_aliases(vec!["x-auth-token"]);
    let resp = app
        .oneshot(
            Request::get("/v1/models")
                .header("x-auth-token", "test-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "model": "test",
                        "messages": [{"role": "user", "content": "hi"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = parse_body(resp).await;
    assert!(
        body["error"].is_object(),
        "error response should be an object"
    );
}

#[tokio::test]
async fn legacy_completions_route_exists_and_accepts_alias_auth() {
    let app = test_app_with_aliases(vec!["x-auth-token"]);
    let resp = app
        .oneshot(
            Request::post("/v1/completions")
                .header("content-type", "application/json")
                .header("x-auth-token", "test-key")
                .body(Body::from(
                    serde_json::json!({
                        "model": "test",
                        "prompt": "hello"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn embeddings_route_exists_and_accepts_api_key_auth() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/embeddings")
                .header("content-type", "application/json")
                .header("api-key", "test-key")
                .body(Body::from(
                    serde_json::json!({
                        "model": "test",
                        "input": "hello"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn audio_speech_route_exists_and_accepts_api_key_auth() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/audio/speech")
                .header("content-type", "application/json")
                .header("api-key", "test-key")
                .body(Body::from(
                    serde_json::json!({
                        "model": "tts-1",
                        "input": "hello world"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn wrong_auth_returns_401() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", "Bearer wrong-key")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "model": "test",
                        "messages": [{"role": "user", "content": "hi"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn valid_auth_but_no_routes_returns_error() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", "Bearer test-key")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "model": "nonexistent",
                        "messages": [{"role": "user", "content": "hi"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Should get 400 or 503 (no providers for model), not 500.
    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 503,
        "expected 400 or 503, got {}",
        status
    );
    let body = parse_body(resp).await;
    assert!(
        body["error"].is_object(),
        "error response should be an object"
    );
}

#[tokio::test]
async fn x_api_key_auth_works() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/messages")
                .header("x-api-key", "test-key")
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "model": "test",
                        "max_tokens": 10,
                        "messages": [{"role": "user", "content": "hi"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Should NOT be 401 (auth should pass via x-api-key).
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "x-api-key auth should succeed"
    );
}

#[tokio::test]
async fn error_response_is_json_not_html() {
    let app = test_app();
    let resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from("not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("json"),
        "error should be JSON, got content-type: {}",
        ct
    );
}

#[tokio::test]
async fn oversized_request_returns_413_with_request_too_large_code() {
    let app = test_app_with_limits(256, 128);
    let resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(oversized_chat_request_bytes(1024)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = parse_body(resp).await;
    assert_eq!(body["error"]["code"], "request_too_large");
}

#[tokio::test]
async fn search_uses_smaller_body_limit_than_chat() {
    let app = test_app_with_limits(4096, 256);
    let request_body = serde_json::json!({
        "model": "test",
        "messages": [{
            "role": "user",
            "content": "x".repeat(600)
        }]
    })
    .to_string();

    let search_resp = app
        .clone()
        .oneshot(
            Request::post("/v1/search")
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search_resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let chat_resp = app
        .oneshot(
            Request::post("/v1/chat/completions")
                .header("authorization", "Bearer test-key")
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(chat_resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn response_has_x_request_id() {
    let app = test_app();
    let resp = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(
        resp.headers().get("x-request-id").is_some(),
        "response should include X-Request-Id header"
    );
}
