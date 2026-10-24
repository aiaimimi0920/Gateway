//! Credential-group scoped hard admission, shared probes, and secret-free management responses.
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
    pipeline::stage_send,
    routing::config::RouteConfigStore,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn token() -> OptionalBearerToken {
    OptionalBearerToken(Some("fixture-management".into()))
}

#[tokio::test]
async fn pausing_a_plan_cancels_inflight_detection_and_discards_late_results() {
    let started = Arc::new(tokio::sync::Notify::new());
    let observed = started.clone();
    let app = Router::new().route(
        "/api/perf-metrics/summary",
        get(move || {
            let observed = observed.clone();
            async move {
                observed.notify_one();
                std::future::pending::<Json<serde_json::Value>>().await
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let monitor = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut fixture = TestState::new();
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.gateway_management_token = Some("fixture-management".into());
    state.console_auth = Arc::new(
        ConsoleAuthRuntime::new(&state.config.console, Some("fixture-management".into())).unwrap(),
    );
    let mut document = json!({"providers":[{"id":"site","adapter":"openai_compatible",
        "base_url":"https://fixture.invalid","site_health":{"profiles":[{"id":"p","name":"Plan",
        "policy":{"adapter":"new-api-perf-summary","site_url":monitor,"allow_private_network":true}}]}}]});
    state.route_config = Arc::new(
        RouteConfigStore::from_document(serde_json::from_value(document.clone()).unwrap()).unwrap(),
    );
    let state = fixture.state.clone();
    let refresh = tokio::spawn(async move {
        internal_site_health::refresh_plan(
            State(state),
            Path(("site".into(), "p".into())),
            token(),
            HeaderMap::new(),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), started.notified())
        .await
        .unwrap();
    document["providers"][0]["site_health"]["profiles"][0]["policy"]["enabled"] = json!(false);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(2), refresh)
        .await
        .unwrap()
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.http_status, Some(409));
    let response =
        internal_site_health::list(State(fixture.state.clone()), token(), HeaderMap::new())
            .await
            .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["items"][0]["profiles"][0]["snapshot"].is_null());
    server.abort();
    fixture.finish().await;
}
#[inline(never)]
fn route<'a>(
    ctx: &'a mut neuro_gateway::pipeline::PipelineContext,
    state: &'a Arc<neuro_gateway::state::AppState>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), neuro_gateway::error::GatewayError>> + 'a>,
> {
    Box::pin(neuro_gateway::pipeline::stage_route::run(ctx, state))
}
#[tokio::test]
async fn shared_plan_runs_once_but_independent_plan_does_not_inherit_its_result() {
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let app=Router::new().route("/api/perf-metrics/summary",get(move |headers:HeaderMap| {
        assert_eq!(headers["x-api-key"],"monitor-secret");
        calls.fetch_add(1,Ordering::SeqCst);
        async {Json(json!({"success":true,"data":{"models":[{"model_name":"upstream-model","success_rate":40}]}}))}
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let monitor = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let upstream = Upstream::start(StatusCode::OK, false, JSON_REPLY).await;
    let mut fixture = TestState::new();
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.gateway_management_token = Some("fixture-management".into());
    state.console_auth = Arc::new(
        ConsoleAuthRuntime::new(&state.config.console, Some("fixture-management".into())).unwrap(),
    );
    let policy = |threshold| {
        json!({"adapter":"new-api-perf-summary","mode":"restrict","unknown_policy":"block",
        "minimum_success_rate":threshold,"site_url":monitor,"allow_private_network":true,
        "auth":{"kind":"api_key","header":"x-api-key","token":"monitor-secret"}})
    };
    let document: neuro_gateway::routing::config::RouteConfigYaml=serde_json::from_value(json!({"providers":[{"id":"site","adapter":"openai_compatible","base_url":upstream.base_url,
        "model_map":{"caller-alias":"upstream-model"},
        "credentials":[{"id":"a","api_key":"fixture"},{"id":"b","api_key":"fixture"},{"id":"c","api_key":"fixture"},{"id":"manual-disabled","api_key":"fixture","enabled":false}],
        "site_health":{"profiles":[
            {"id":"plus","name":"Plus","account_group_ids":["local-plus","local-shared"],"policy":policy(95)},
            {"id":"pro","name":"Pro","account_group_ids":["local-pro"],"policy":policy(30)}
        ]}}],"account_groups":[
            {"id":"local-plus","name":"Plus","provider_credential_ids":["a"]},
            {"id":"local-shared","name":"Shared","provider_credential_ids":["b"]},
            {"id":"local-pro","name":"Pro","provider_credential_ids":["c","manual-disabled"]}
        ]})).unwrap();
    state.route_config = Arc::new(RouteConfigStore::from_document(document.clone()).unwrap());
    for plan in ["plus", "pro"] {
        internal_site_health::refresh_plan(
            State(fixture.state.clone()),
            Path(("site".into(), plan.into())),
            token(),
            HeaderMap::new(),
        )
        .await
        .unwrap();
    }
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "one request per plan, not per bound group"
    );
    for _ in 0..3 {
        let mut ctx = context(false, vec![]);
        route(&mut ctx, &fixture.state).await.unwrap();
        assert_eq!(ctx.candidates.len(), 1);
        assert_eq!(
            ctx.candidates[0].payload.credential_id.as_deref(),
            Some("c"),
            "unhealthy groups must be removed before pool round-robin"
        );
        assert_eq!(ctx.route_attempt_count(), 0);
    }
    for credential in ["a", "b", "c"] {
        let mut selected = candidate(&upstream, credential);
        selected.provider_account_id = "site".into();
        selected.payload.credential_id = Some(credential.into());
        let mut ctx = context(false, vec![selected]);
        let result = stage_send::run(&mut ctx, &fixture.state).await;
        if credential == "c" {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result.err().unwrap().code.as_deref(),
                Some("site_health_route_restricted")
            );
            assert_eq!(ctx.route_attempt_count(), 0);
        }
    }
    assert_eq!(upstream.requests().len(), 1);
    let response =
        internal_site_health::list(State(fixture.state.clone()), token(), HeaderMap::new())
            .await
            .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(!text.contains("monitor-secret"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["items"][0]["profiles"].as_array().unwrap().len(), 2);
    assert!(value["items"][0]["snapshot"].is_null());
    let providers = fixture.state.route_config.get_providers();
    assert!(!providers[0].credential_pool[3].enabled);
    let error = internal_site_health::refresh_plan(
        State(fixture.state.clone()),
        Path(("site".into(), "plus".into())),
        token(),
        HeaderMap::new(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.http_status, Some(429));
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let mut paused = serde_json::to_value(&document).unwrap();
    paused["providers"][0]["site_health"]["profiles"][0]["policy"]["enabled"] = json!(false);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(paused).unwrap())
        .unwrap();
    let error = internal_site_health::refresh_plan(
        State(fixture.state.clone()),
        Path(("site".into(), "plus".into())),
        token(),
        HeaderMap::new(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "disabled plans never send a manual probe"
    );
    let _router = neuro_gateway::http::router::build_router(fixture.state.clone());
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "worker skips paused plans and leaves the other plan on its own interval"
    );
    let mut ctx = context(false, vec![]);
    route(&mut ctx, &fixture.state).await.unwrap();
    assert!(!ctx.candidates.is_empty());
    assert!(ctx
        .candidates
        .iter()
        .all(|c| c.payload.credential_id.as_deref() != Some("manual-disabled")));
    for credential in ["a", "b"] {
        let mut selected = candidate(&upstream, credential);
        selected.provider_account_id = "site".into();
        selected.payload.credential_id = Some(credential.into());
        let mut ctx = context(false, vec![selected]);
        Box::pin(stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap();
    }
    let response =
        internal_site_health::list(State(fixture.state.clone()), token(), HeaderMap::new())
            .await
            .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["items"][0]["profiles"][0]["enabled"], false);
    assert!(value["items"][0]["profiles"][0]["snapshot"].is_null());
    assert_eq!(value["items"][0]["profiles"][1]["enabled"], true);
    assert!(!value["items"][0]["profiles"][1]["snapshot"].is_null());
    assert!(!String::from_utf8(bytes.to_vec())
        .unwrap()
        .contains("monitor-secret"));
    fixture
        .state
        .route_config
        .replace_document(document)
        .unwrap();
    let mut ctx = context(false, vec![]);
    route(&mut ctx, &fixture.state).await.unwrap();
    assert_eq!(
        ctx.candidates.len(),
        1,
        "resume restores this plan's independent restriction"
    );
    let error = internal_site_health::refresh_plan(
        State(fixture.state.clone()),
        Path(("site".into(), "plus".into())),
        token(),
        HeaderMap::new(),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        error.http_status,
        Some(429),
        "pause/resume does not reset the probe throttle"
    );
    drop(_router);
    server.abort();
    fixture.finish().await;
    upstream.finish().await;
}
