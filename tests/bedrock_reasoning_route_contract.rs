//! Real public router/auth/send/packing over synthetic native AWS TCP; no provider credentials.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_reasoning_response_contract/fixtures.rs"]
mod fixtures;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use futures::FutureExt;
use http_body_util::BodyExt;
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
use tower::ServiceExt;

struct Server {
    base: String,
    calls: Arc<AtomicUsize>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/model/fixture-model/converse-stream", post(respond))
            .with_state(calls.clone());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            base,
            calls,
            stop: Some(stop),
            task,
        }
    }
    async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn respond(
    State(calls): State<Arc<AtomicUsize>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl axum::response::IntoResponse {
    assert_eq!(headers["authorization"], "Bearer synthetic-only");
    assert_eq!(body["messages"][0]["role"], "user");
    calls.fetch_add(1, Ordering::SeqCst);
    (
        [("content-type", "application/vnd.amazon.eventstream")],
        fixtures::sequence().concat(),
    )
}

async fn verify() {
    let server = Server::new().await;
    let fixture=support::ConsoleStateFixture::new(serde_json::from_value(json!({
        "providers":[{"id":"bedrock-reasoning","adapter":"bedrock_converse_compatible","protocol_profile":"aws_bedrock",
            "base_url":server.base,"supported_models":["fixture-model"],"credentials":[{"id":"exact-account","api_key":"synthetic-only","supported_models":["fixture-model"]}]}],
        "model_routes":[{"pattern":"fixture-model","provider_ids":["bedrock-reasoning"]}],"aliases":{}})).unwrap(),false);
    let selected = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("fixture-model"));
    assert_eq!(selected.len(), 1);
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected[0].provider_account_id);
    let cases = [
        (
            "/v1/chat/completions",
            json!({"model":"fixture-model","messages":[{"role":"user","content":"x"}]}),
        ),
        (
            "/v1/messages",
            json!({"model":"fixture-model","max_tokens":32,"messages":[{"role":"user","content":"x"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"fixture-model","input":"x"}),
        ),
        (
            "/model/fixture-model/converse",
            json!({"messages":[{"role":"user","content":[{"text":"x"}]}]}),
        ),
    ];
    for (index, (path, body)) in cases.into_iter().enumerate() {
        let response = timeout(
            Duration::from_secs(10),
            build_router(fixture.state.clone())
                .oneshot(
                    Request::post(path)
                        .header("authorization", "Bearer public-api-contract-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .boxed(),
        )
        .await
        .unwrap()
        .unwrap();
        let status = response.status();
        let bytes = timeout(Duration::from_secs(10), response.into_body().collect())
            .await
            .unwrap()
            .unwrap()
            .to_bytes();
        assert_eq!(
            status,
            StatusCode::OK,
            "path={path}, calls={}, body={}",
            server.calls.load(Ordering::SeqCst),
            String::from_utf8_lossy(&bytes)
        );
        assert!(bytes.len() < 64 * 1024);
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["model"], "fixture-model");
        if index < 3 {
            assert_eq!(body["gateway_bedrock_turn"]["content"], fixtures::content());
            assert!(body.get("gateway_vertex_turn").is_none());
            let canonical = match index {
                0 => neuro_gateway::protocol::openai::unpack_openai_response(&body),
                1 => neuro_gateway::protocol::anthropic::unpack_anthropic_response(&body),
                _ => neuro_gateway::protocol::responses::unpack_responses_response(&body),
            }
            .unwrap();
            assert_eq!(canonical.text, "beforeafter");
            assert_eq!(canonical.tool_calls.len(), 1);
            assert_eq!(canonical.usage.unwrap().total_tokens, 7);
        } else {
            assert_eq!(body["output"]["message"]["content"], fixtures::content());
            assert!(body.get("gateway_bedrock_turn").is_none());
            assert_eq!(body["usage"]["totalTokens"], 7);
        }
        assert_eq!(controller.snapshot().consecutive_successes, index + 1);
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(server.calls.load(Ordering::SeqCst), index + 1);
    }
    let response = timeout(
        Duration::from_secs(10),
        build_router(fixture.state.clone())
            .oneshot(
                Request::post("/v1/completions")
                    .header("authorization", "Bearer public-api-contract-secret")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"model":"fixture-model","prompt":"x"}).to_string(),
                    ))
                    .unwrap(),
            )
            .boxed(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(server.calls.load(Ordering::SeqCst), 4);
    assert_eq!(controller.snapshot().consecutive_successes, 4);
    server.finish().await;
}

#[test]
fn public_three_protocol_json_and_native_converse_keep_order_and_finish_once() {
    // Match the real runtime's stack allowance, not the Windows test thread's small default.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    timeout(Duration::from_secs(55), verify()).await.unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
