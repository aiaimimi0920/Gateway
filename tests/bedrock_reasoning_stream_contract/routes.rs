//! Normal public auth/routing/send/tracking, with a genuine TCP synthetic AWS upstream.
use super::*;
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
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
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
fn native_events(mut bytes: &[u8]) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    while !bytes.is_empty() {
        let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        events.push(native_wire::decode(&Bytes::copy_from_slice(&bytes[..size])));
        bytes = &bytes[size..];
    }
    events
}
async fn verify() {
    let server = Server::new().await;
    let fixture=super::support::ConsoleStateFixture::new(serde_json::from_value(json!({
        "providers":[{"id":"bedrock-reasoning-stream","adapter":"bedrock_converse_compatible","protocol_profile":"aws_bedrock",
            "base_url":server.base,"supported_models":["fixture-model"],"credentials":[{"id":"exact-account","api_key":"synthetic-only","supported_models":["fixture-model"]}]}],
        "model_routes":[{"pattern":"fixture-model","provider_ids":["bedrock-reasoning-stream"]}],"aliases":{}})).unwrap(),false);
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
            json!({"model":"fixture-model","stream":true,"messages":[{"role":"user","content":"x"}]}),
        ),
        (
            "/v1/messages",
            json!({"model":"fixture-model","stream":true,"max_tokens":32,"messages":[{"role":"user","content":"x"}]}),
        ),
        (
            "/v1/responses",
            json!({"model":"fixture-model","stream":true,"input":"x"}),
        ),
        (
            "/model/fixture-model/converse-stream",
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
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let bytes = timeout(Duration::from_secs(10), response.into_body().collect())
            .await
            .unwrap()
            .unwrap()
            .to_bytes();
        if index == 3 {
            assert!(content_type.starts_with("application/vnd.amazon.eventstream"));
            let expected = native_events(&fixtures::sequence().concat());
            assert_eq!(native_events(&bytes), expected);
        } else {
            assert!(content_type.starts_with("text/event-stream"));
            let frames = values(&bytes);
            assert_eq!(
                frames
                    .iter()
                    .filter(|v| v.get("gateway_bedrock_event").is_some())
                    .count(),
                fixtures::sequence().len()
            );
            let terminal = frames
                .iter()
                .find_map(|v| {
                    let body = v.get("response").unwrap_or(v);
                    body.get("gateway_bedrock_terminal").map(|_| body)
                })
                .unwrap();
            assert_eq!(
                terminal["gateway_bedrock_turn"]["content"],
                fixtures::content()
            );
            assert_eq!(
                terminal["gateway_bedrock_terminal"]["events"],
                fixtures::sequence().len()
            );
        }
        timeout(Duration::from_secs(2), async {
            while controller.snapshot().consecutive_successes != index + 1
                || controller.snapshot().active_count != 0
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(server.calls.load(Ordering::SeqCst), index + 1);
    }
    server.finish().await;
}
#[test]
fn normal_public_sse_and_native_routes_keep_reasoning_and_report_success_once() {
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
