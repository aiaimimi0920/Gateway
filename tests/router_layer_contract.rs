//! Route assembly must retain body-limit boundaries and outer middleware order.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use neuro_gateway::config::Config;
use neuro_gateway::http::router::build_router;
use neuro_gateway::routing::config::RouteConfigStore;
use tower::ServiceExt;

#[path = "console_contract_support/mod.rs"]
mod console_support;
mod support;

use console_support::test_config;
use support::build_test_app_state;

fn sized_request(path: &str, size: usize) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .header("content-length", size.to_string())
        .header("x-request-id", "router-layer-contract")
        .body(Body::from(format!("{{}}{}", " ".repeat(size - 2))))
        .unwrap()
}

async fn assert_limit_boundary(app: &Router, path: &str, limit: usize) {
    let accepted = app
        .clone()
        .oneshot(sized_request(path, limit))
        .await
        .unwrap();
    assert!(
        matches!(
            accepted.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::UNPROCESSABLE_ENTITY
        ),
        "{path}: an exactly bounded unauthenticated request must reach extraction/admission, got {}",
        accepted.status()
    );

    let rejected = app
        .clone()
        .oneshot(sized_request(path, limit + 1))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::PAYLOAD_TOO_LARGE, "{path}");
    assert_eq!(rejected.headers()["x-request-id"], "router-layer-contract");
}

#[tokio::test]
async fn family_limits_cover_public_routes_and_new_api_aliases() {
    type SetLimit = fn(&mut Config, usize);
    let families: &[(&str, SetLimit)] = &[
        ("chat/completions", |c, v| {
            c.max_body_chat_completions_bytes = v
        }),
        ("completions", |c, v| c.max_body_completions_bytes = v),
        ("messages", |c, v| c.max_body_messages_bytes = v),
        ("responses", |c, v| c.max_body_responses_bytes = v),
        ("embeddings", |c, v| c.max_body_embeddings_bytes = v),
        ("audio/transcriptions", |c, v| {
            c.max_body_audio_transcriptions_bytes = v
        }),
        ("audio/speech", |c, v| c.max_body_audio_speech_bytes = v),
        ("images/generations", |c, v| {
            c.max_body_images_generations_bytes = v
        }),
        ("images/edits", |c, v| c.max_body_images_edits_bytes = v),
        ("search", |c, v| c.max_body_search_bytes = v),
        ("fetch", |c, v| c.max_body_fetch_bytes = v),
        ("research", |c, v| c.max_body_research_bytes = v),
        ("music/generations", |c, v| c.max_body_music_bytes = v),
        ("videos/generations", |c, v| c.max_body_videos_bytes = v),
    ];
    let mut config = test_config();
    config.max_request_body_bytes = 4096;
    let mut cases = Vec::new();
    for (index, (suffix, set_limit)) in families.iter().enumerate() {
        let limit = 128 + index * 16;
        set_limit(&mut config, limit);
        for prefix in ["/v1", "/v1/new-api"] {
            cases.push((format!("{prefix}/{suffix}"), limit));
        }
    }
    for path in [
        "/v1/models/test-model:generateContent",
        "/v1beta/models/test-model:generateContent",
        "/model/test-model/converse",
        "/model/test-model/converse-stream",
        "/v2/chat",
    ] {
        cases.push((path.to_string(), config.max_body_chat_completions_bytes));
    }
    let app = build_router(build_test_app_state(config, RouteConfigStore::new(), None));
    for (path, limit) in cases {
        assert_limit_boundary(&app, &path, limit).await;
    }
}

#[tokio::test]
async fn global_limit_wraps_public_and_management_route_groups() {
    let mut config = test_config();
    config.max_request_body_bytes = 64;
    let app = build_router(build_test_app_state(config, RouteConfigStore::new(), None));
    for path in [
        "/v1/chat/completions",
        "/v1/internal/gateway/api-access/resolve",
        "/v1/internal/gateway/console/session/verify",
        "/v1/internal/gateway/conversation-archives/export",
        "/v1/internal/gateway/credential-pool-refill/tasks/claim",
        "/v1/internal/gateway/analysis/anomaly-policies",
    ] {
        let response = app.clone().oneshot(sized_request(path, 65)).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE, "{path}");
        assert_eq!(response.headers()["x-request-id"], "router-layer-contract");
    }
}

#[tokio::test]
async fn cors_preflight_precedes_drain_and_drain_precedes_body_limits() {
    let mut config = test_config();
    config.max_request_body_bytes = 64;
    let state = build_test_app_state(config, RouteConfigStore::new(), None);
    assert!(state.lifecycle.begin_drain("router-layer-contract"));
    let app = build_router(Arc::clone(&state));

    let preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/v1/chat/completions")
                .header("origin", "https://client.example")
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "authorization,content-type",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preflight.status(), StatusCode::OK);
    assert_eq!(preflight.headers()["access-control-allow-origin"], "*");
    assert!(preflight.headers().get("x-request-id").is_none());

    let mut request = sized_request("/v1/chat/completions", 65);
    request
        .headers_mut()
        .insert("origin", "https://client.example".parse().unwrap());
    let drained = app.clone().oneshot(request).await.unwrap();
    assert_eq!(drained.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(drained.headers()["access-control-allow-origin"], "*");
    assert_eq!(drained.headers()["x-request-id"], "router-layer-contract");
    let body = drained.into_body().collect().await.unwrap().to_bytes();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"]["code"], "gateway_draining");

    let health = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert!(health.headers().get("x-request-id").is_some());
}
