//! Loopback-only proof of shared budgets, large-pool scope and pinned route revisions.
use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    http::router::build_router, local_runtime::LocalRuntime,
    provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once,
};
use serde_json::{json, Value};
use std::future::IntoFuture;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Notify;
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

fn plan(models: Value, enabled: bool) -> Value {
    json!({"automaticEnabled":enabled,"intervalMinutes":15,"modelSelection":"selected","models":models,
        "cases":[{"id":"ok","name":"ok","prompt":"Reply OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}
fn reply() -> Value {
    json!({"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1}})
}
async fn local_fixture(document: Value) -> ConsoleStateFixture {
    let mut fixture = ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), false);
    let root = std::env::temp_dir().join(format!(
        "gateway-test-policy-bounds-{}",
        uuid::Uuid::new_v4()
    ));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    fixture
}

#[tokio::test]
async fn exhausted_sweep_does_not_claim_or_overwrite_the_next_account() {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/v1/chat/completions",
                    post(|State(count): State<Arc<AtomicUsize>>| async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Json(reply())
                    }),
                )
                .with_state(calls.clone()),
        )
        .into_future(),
    );
    let models: Vec<_> = (0..128).map(|index| format!("m{index}")).collect();
    let fixture = local_fixture(json!({"providers":[{"id":"pool","preset":"openai","base_url":format!("http://{address}"),"supported_models":models,
        "credentials":[{"id":"first","api_key":"first-secret","test_policy":plan(json!(models),true)},
            {"id":"second","api_key":"second-secret","test_policy":plan(json!(["m0"]),true)}]}]})).await;
    let first = sweep_scheduled_provider_credentials_once(&fixture.state)
        .await
        .unwrap();
    assert_eq!(first.due_count, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 128);
    let next = sweep_scheduled_provider_credentials_once(&fixture.state)
        .await
        .unwrap();
    assert_eq!(next.due_count, 1);
    assert_eq!(next.passed_count, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 129);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn account_scope_is_filtered_before_the_large_pool_limit_and_empty_body_uses_saved_policy() {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/v1/chat/completions",
                    post(|State(count): State<Arc<AtomicUsize>>| async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Json(reply())
                    }),
                )
                .with_state(calls.clone()),
        )
        .into_future(),
    );
    let credentials: Vec<_> = (0..130)
        .map(|index| json!({"id":format!("c{index}"),"api_key":"fixture-secret"}))
        .collect();
    let mut document = json!({"providers":[{"id":"pool","preset":"openai","base_url":format!("http://{address}"),"supported_models":["a"],"test_policy":plan(json!(["a"]),false),"credentials":credentials}]});
    let fixture = local_fixture(document.clone()).await;
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/pool/probe",
        json!({"testPlan":plan(json!(["a"]),false),"scope":{"kind":"account","id":"c129"}}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_json(response).await["result"]["totalCount"], 1);
    let stored = send_console_json(
        &fixture.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/pool/probe/results",
        json!({"scope":{"kind":"account","id":"c129"}}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(stored.status(), StatusCode::OK);
    assert_eq!(parse_json(stored).await["result"]["totalCount"], 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    document["providers"][0]["credentials"] = json!([{"id":"c129","api_key":"fixture-secret"}]);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/internal/gateway/console/providers/pool/probe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        parse_json(response).await["result"]["results"][0]["assessment"]["policySource"],
        "pool"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn changing_revision_during_a_call_stops_later_accounts_without_sending() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = (calls.clone(), entered.clone(), release.clone());
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/v1/chat/completions",
                    post(
                        |State((count, entered, release)): State<(
                            Arc<AtomicUsize>,
                            Arc<Notify>,
                            Arc<Notify>,
                        )>| async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            entered.notify_one();
                            release.notified().await;
                            Json(reply())
                        },
                    ),
                )
                .with_state(state),
        )
        .into_future(),
    );
    let mut document = json!({"providers":[{"id":"pool","preset":"openai","base_url":format!("http://{address}"),"supported_models":["a"],"test_policy":plan(json!(["a"]),true),
        "credentials":[{"id":"one","api_key":"one-secret"},{"id":"two","api_key":"two-secret"}]}]});
    let fixture = local_fixture(document.clone()).await;
    let run_state = fixture.state.clone();
    let sweep = tokio::spawn(async move {
        sweep_scheduled_provider_credentials_once(&run_state)
            .await
            .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    document["providers"][0]["test_policy"]["automaticEnabled"] = json!(false);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    release.notify_one();
    let summary = sweep.await.unwrap();
    assert_eq!(summary.due_count, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}
