use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use regex::Regex;
use tower::ServiceExt;

mod support;
use support::build_test_app_state;

fn test_config() -> Config {
    Config {
        console: Default::default(),
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
        gateway_management_token: Some("console-ui-assets-contract-token".to_string()),
        gateway_keepalive_bearer_token: Some("keepalive-contract-secret".to_string()),
        default_project_id: "console-ui-project".to_string(),
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

fn test_state() -> Arc<AppState> {
    build_test_app_state(test_config(), RouteConfigStore::new(), None)
}

fn test_app() -> axum::Router {
    build_router(test_state())
}

#[tokio::test]
async fn ui_root_redirects_to_trailing_slash() {
    let response = test_app()
        .oneshot(Request::get("/ui").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/ui/");
}

#[tokio::test]
async fn ui_index_returns_embedded_html_shell() {
    let response = test_app()
        .oneshot(Request::get("/ui/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .starts_with("text/html"),);
    let body = response_text(response).await;
    assert!(body.to_ascii_lowercase().contains("<!doctype html"));
    assert!(body.contains("/ui/static/"));
}

#[tokio::test]
async fn ui_known_hashed_asset_is_served_with_immutable_caching() {
    let index = test_app()
        .oneshot(Request::get("/ui/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = response_text(index).await;
    let asset_path = first_static_asset_path(&html);

    let response = test_app()
        .oneshot(Request::get(&asset_path).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let cache_control = response
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(cache_control.contains("immutable"));
    assert!(cache_control.contains("max-age="));
    assert_eq!(
        response
            .headers()
            .get("x-content-type-options")
            .and_then(|value| value.to_str().ok()),
        Some("nosniff")
    );
}

#[tokio::test]
async fn ui_client_route_falls_back_to_index_html() {
    let response = test_app()
        .oneshot(Request::get("/ui/routes").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .starts_with("text/html"),);
}

#[tokio::test]
async fn ui_missing_static_asset_returns_not_found() {
    let response = test_app()
        .oneshot(
            Request::get("/ui/static/js/definitely-missing.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ui_does_not_expose_workspace_or_state_files() {
    for path in [
        "/routes.yaml",
        "/ui/../routes.yaml",
        "/ui/.gateway-state/transactions/test.json",
        "/ui/release/Gateway.zip",
    ] {
        let response = test_app()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::OK,
            "path should not be downloadable: {path}"
        );
    }
}

async fn response_text(response: axum::response::Response) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("response must be valid UTF-8")
}

fn first_static_asset_path(html: &str) -> String {
    let pattern = Regex::new(r#"(?:src|href)=\"(?P<path>/ui/static/[^\"]+)\""#)
        .expect("asset regex must compile");
    pattern
        .captures(html)
        .and_then(|captures| captures.name("path"))
        .map(|value| value.as_str().to_string())
        .expect("index must reference a static asset")
}
