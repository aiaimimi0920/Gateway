//! Management admission, configuration fencing and the shared send loop, without external services.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use fixture::{candidate, context, TestState, Upstream, JSON_REPLY};
use neuro_gateway::{
    console::ConsoleAuthRuntime,
    http::{extractors::OptionalBearerToken, routes::internal_site_health},
    pipeline::{stage_send, PipelineOutput},
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::json;
use std::{sync::Arc, time::Duration};

fn document(base: &str, mode: &str) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"site","adapter":"openai_compatible","base_url":base,"api_key":"fixture",
        "site_health":{"mode":mode,"unknown_policy":"block","allow_private_network":true}}]})).unwrap()
}
fn configured(base: &str, mode: &str) -> TestState {
    let mut fixture = TestState::new();
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.gateway_management_token = Some("fixture-management".into());
    state.console_auth = Arc::new(
        ConsoleAuthRuntime::new(&state.config.console, Some("fixture-management".into())).unwrap(),
    );
    state.route_config = Arc::new(RouteConfigStore::from_document(document(base, mode)).unwrap());
    fixture
}
fn token() -> OptionalBearerToken {
    OptionalBearerToken(Some("fixture-management".into()))
}

#[tokio::test]
async fn restriction_skips_before_attempt_for_both_send_modes_but_advisory_does_not() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = configured(&upstream.base_url, "restrict");
    for streaming in [false, true] {
        let mut selected = candidate(&upstream, "restricted");
        selected.provider_account_id = "site".into();
        selected.upstream_model = Some(String::new());
        selected.payload.default_model = Some("upstream-model".into());
        let mut ctx = context(streaming, vec![selected]);
        ctx.canonical_req.requested_model = None;
        let error = stage_send::run(&mut ctx, &fixture.state)
            .await
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some("site_health_route_restricted"));
        assert_eq!(ctx.route_attempt_count(), 0);
    }
    assert!(upstream.requests().is_empty());
    fixture
        .state
        .route_config
        .replace_document(document(&upstream.base_url, "advisory"))
        .unwrap();
    let mut selected = candidate(&upstream, "advisory");
    selected.provider_account_id = "site".into();
    let mut ctx = context(false, vec![selected]);
    assert!(matches!(
        stage_send::run(&mut ctx, &fixture.state).await.unwrap(),
        PipelineOutput::Json(_)
    ));
    assert_eq!(upstream.requests().len(), 1);
    fixture.finish().await;
    upstream.finish().await;
}

#[tokio::test]
async fn denied_primary_keeps_fallback_candidate_indices_and_does_not_charge_an_attempt() {
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = configured(&upstream.base_url, "restrict");
    let mut denied = candidate(&upstream, "denied");
    denied.provider_account_id = "site".into();
    let backup = candidate(&upstream, "allowed");
    let mut ctx = context(false, vec![denied, backup]);
    fixture::authorize_candidates(&mut ctx);
    stage_send::run(&mut ctx, &fixture.state).await.unwrap();
    assert_eq!(ctx.route_attempt_count(), 1);
    assert_eq!(upstream.requests().len(), 1);
    fixture.finish().await;
    upstream.finish().await;
}

#[tokio::test]
async fn management_auth_and_config_change_fence_site_observations() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (notify, wait) = (started.clone(), release.clone());
    let app=Router::new().route("/api/perf-metrics/summary",get(move || {
        let (notify,wait)=(notify.clone(),wait.clone()); async move {
            notify.notify_one(); wait.notified().await;
            Json(json!({"success":true,"data":{"models":[{"model_name":"upstream-model","success_rate":20}]}}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let fixture = configured(&base, "restrict");
    assert!(internal_site_health::list(
        State(fixture.state.clone()),
        OptionalBearerToken(None),
        HeaderMap::new()
    )
    .await
    .is_err());
    let state = fixture.state.clone();
    let pending = tokio::spawn(async move {
        internal_site_health::refresh(State(state), Path("site".into()), token(), HeaderMap::new())
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    fixture
        .state
        .route_config
        .replace_document(document(&base, "advisory"))
        .unwrap();
    release.notify_one();
    let error = pending.await.unwrap().err().unwrap();
    assert_eq!(error.http_status, Some(409));
    let response =
        internal_site_health::list(State(fixture.state.clone()), token(), HeaderMap::new())
            .await
            .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["items"][0]["snapshot"].is_null());
    server.abort();
    fixture.finish().await;
}

#[tokio::test]
async fn published_health_restricts_the_actual_model_without_account_or_probe_attempts() {
    let probe_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = probe_count.clone();
    let app = Router::new().route(
        "/api/perf-metrics/summary",
        get(move || {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async {
                Json(json!({"success":true,"data":{"models":[
                    {"model_name":"upstream-model","success_rate":20},
                    {"model_name":"healthy","success_rate":100}
                ]}}))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let fixture = configured(&upstream.base_url, "restrict");
    let mut config = serde_json::to_value(document(&upstream.base_url, "restrict")).unwrap();
    config["providers"][0]["site_health"]["site_url"] = json!(base);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(config).unwrap())
        .unwrap();
    let response = internal_site_health::refresh(
        State(fixture.state.clone()),
        Path("site".into()),
        token(),
        HeaderMap::new(),
    )
    .await
    .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value["snapshot"]["selected_template"],
        "new-api-perf-summary"
    );
    assert_eq!(
        value["snapshot"]["models"]["upstream-model"].as_f64(),
        Some(20.0)
    );
    assert!(
        upstream.requests().is_empty(),
        "site refresh must not test an account"
    );

    // Empty route override must resolve to the same provider default used by the send owner.
    let mut denied = candidate(&upstream, "denied-by-published-health");
    denied.provider_account_id = "site".into();
    denied.upstream_model = Some(String::new());
    denied.payload.default_model = Some("upstream-model".into());
    let mut ctx = context(false, vec![denied]);
    ctx.canonical_req.requested_model = None;
    let error = stage_send::run(&mut ctx, &fixture.state)
        .await
        .err()
        .unwrap();
    assert_eq!(error.code.as_deref(), Some("site_health_route_restricted"));
    assert_eq!(ctx.route_attempt_count(), 0);
    assert!(upstream.requests().is_empty());

    let mut allowed = candidate(&upstream, "healthy-from-cache");
    allowed.provider_account_id = "site".into();
    allowed.upstream_model = Some("healthy".into());
    let mut ctx = context(false, vec![allowed]);
    stage_send::run(&mut ctx, &fixture.state).await.unwrap();
    assert_eq!(ctx.route_attempt_count(), 1);
    assert_eq!(upstream.requests().len(), 1);
    assert_eq!(upstream.requests()[0].body["model"], "healthy");
    let error = internal_site_health::refresh(
        State(fixture.state.clone()),
        Path("site".into()),
        token(),
        HeaderMap::new(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.http_status, Some(429));
    assert_eq!(probe_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.abort();
    fixture.finish().await;
    upstream.finish().await;
}
