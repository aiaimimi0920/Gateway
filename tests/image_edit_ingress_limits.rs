//! Real image-edit router limits, stopping at validation before provider execution.
use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use bytes::Bytes;
use futures::StreamExt;
use neuro_gateway::config::{Config, GatewayRuntimeRole};
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigStore;
use serde_json::Value;
use std::convert::Infallible;
use tower::ServiceExt;

mod support;

const DEFAULT_LIMIT: usize = 2 * 1024 * 1024;
const ROUTES: [&str; 2] = ["/v1/images/edits", "/v1/new-api/images/edits"];
const BOUNDARY: &str = "gateway-ingress-fixture";

fn app(image_limit: usize) -> axum::Router {
    let limit = 4 * 1024 * 1024;
    let config = Config {
        console: Default::default(),
        runtime_role: GatewayRuntimeRole::Standalone,
        port: 0,
        redis_url: "redis://127.0.0.1:1".to_string(),
        database_url: None,
        upstream_timeout_secs: 1,
        max_request_body_bytes: limit,
        max_body_chat_completions_bytes: limit,
        max_body_completions_bytes: limit,
        max_body_messages_bytes: limit,
        max_body_responses_bytes: limit,
        max_body_embeddings_bytes: limit,
        max_body_audio_transcriptions_bytes: limit,
        max_body_audio_speech_bytes: limit,
        max_body_search_bytes: limit,
        max_body_fetch_bytes: limit,
        max_body_research_bytes: limit,
        max_body_images_generations_bytes: limit,
        max_body_images_edits_bytes: image_limit,
        max_body_music_bytes: limit,
        max_body_videos_bytes: limit,
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
        default_project_id: "ingress-fixture".to_string(),
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
        provider_credential_refresh_before_secs: 86400,
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
    build_router(support::build_test_app_state(
        config,
        RouteConfigStore::new(),
        None,
    ))
}

fn request(path: &str, content_type: &str, bytes: Bytes, streamed: bool) -> Request<Body> {
    let size = bytes.len();
    let body = if streamed {
        let split = size / 2;
        let chunks = futures::stream::iter([
            Ok::<_, Infallible>(bytes.slice(..split)),
            Ok(bytes.slice(split..)),
        ]);
        // Make headers ready immediately, then stop prefetch before the next chunk.
        Body::from_stream(chunks.enumerate().then(|(index, chunk)| async move {
            if index != 0 {
                tokio::task::yield_now().await;
            }
            chunk
        }))
    } else {
        Body::from(bytes)
    };
    let mut request = Request::post(path)
        .header(header::AUTHORIZATION, "Bearer test-key")
        .header(header::CONTENT_TYPE, content_type);
    if !streamed {
        request = request.header(header::CONTENT_LENGTH, size.to_string());
    }
    request.body(body).unwrap()
}

fn json_body(size: usize) -> Bytes {
    let mut bytes = b"{\"padding\":\"".to_vec();
    bytes.resize(size - 2, b'x');
    bytes.extend_from_slice(b"\"}");
    bytes.into()
}

fn multipart_body(field: &str, size: usize) -> Bytes {
    let mut bytes =
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"\r\n\r\n")
            .into_bytes();
    bytes.resize(bytes.len() + size, b'x');
    bytes.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    bytes.into()
}

async fn error_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn image_edit_json_routes_keep_the_extractor_default_ceiling() {
    let app = app(4 * 1024 * 1024);
    for path in ROUTES {
        for streamed in [false, true] {
            for oversized in [false, true] {
                let size = DEFAULT_LIMIT + if oversized { 1024 } else { 0 } - 512;
                let response = app
                    .clone()
                    .oneshot(request(path, "application/json", json_body(size), streamed))
                    .await
                    .unwrap();
                let expected_status = if oversized {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                };
                assert_eq!(
                    response.status(),
                    expected_status,
                    "{path} streamed={streamed}"
                );
                let error = error_body(response).await;
                let expected_code = if oversized {
                    "request_too_large"
                } else {
                    "missing_required_field"
                };
                assert_eq!(error["error"]["code"], expected_code);
            }
        }
    }
}

#[tokio::test]
async fn image_edit_multipart_limits_preserve_chunk_sensitive_error_context() {
    let app = app(4 * 1024 * 1024);
    let content_type = format!("multipart/form-data; boundary={BOUNDARY}");
    for path in ROUTES {
        for streamed in [false, true] {
            for field in ["prompt", "image"] {
                for oversized in [false, true] {
                    let size = DEFAULT_LIMIT + if oversized { 2048 } else { 0 } - 1024;
                    let body = multipart_body(field, size);
                    assert_eq!(body.len() > DEFAULT_LIMIT, oversized);
                    let response = app
                        .clone()
                        .oneshot(request(path, &content_type, body, streamed))
                        .await
                        .unwrap();
                    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                    let error = error_body(response).await;
                    if oversized {
                        let prefix = if !streamed {
                            "Invalid multipart image edit body:"
                        } else if field == "prompt" {
                            "Invalid prompt field:"
                        } else {
                            "Invalid uploaded image data:"
                        };
                        assert!(
                            error["error"]["message"]
                                .as_str()
                                .unwrap()
                                .starts_with(prefix),
                            "{path} streamed={streamed} field={field}: {error}"
                        );
                    } else {
                        let code = if field == "prompt" {
                            "missing_input_image"
                        } else {
                            "missing_required_field"
                        };
                        assert_eq!(error["error"]["code"], code);
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn image_edit_configured_route_cap_remains_enforced_below_default() {
    let limit = 64 * 1024;
    let app = app(limit);
    for path in ROUTES {
        for streamed in [false, true] {
            let response = app
                .clone()
                .oneshot(request(
                    path,
                    "application/json",
                    json_body(limit + 1),
                    streamed,
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        }
    }
}
