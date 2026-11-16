//! Full public requests consume production-issued turns, not hand-crafted continuation hashes.
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
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
use tower::ServiceExt;

struct Server {
    base: String,
    bodies: Arc<Mutex<Vec<Value>>>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route("/model/fixture-model/converse-stream", post(respond))
            .with_state(bodies.clone());
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
            bodies,
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
    State(bodies): State<Arc<Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl axum::response::IntoResponse {
    assert_eq!(headers["authorization"], "Bearer synthetic-only");
    bodies.lock().unwrap().push(body);
    (
        [("content-type", "application/vnd.amazon.eventstream")],
        fixture::sequence().concat(),
    )
}
fn request(protocol: u8, turns: &[Value], stream: bool) -> Value {
    let mut body = if turns.is_empty() {
        first(protocol)
    } else {
        followup(protocol, turns[0].clone())
    };
    if turns.len() > 1 {
        let key = if protocol == 2 { "input" } else { "messages" };
        let suffix = body[key].as_array().unwrap()[1..].to_vec();
        for _ in &turns[1..] {
            body[key].as_array_mut().unwrap().extend(suffix.clone());
        }
    }
    if !turns.is_empty() {
        body["gateway_bedrock_history"] = json!(turns
            .iter()
            .enumerate()
            .map(|(i, turn)| json!({"assistant_turn":i,"turn":turn}))
            .collect::<Vec<_>>());
    }
    body["model"] = json!("fixture-model");
    body["stream"] = json!(stream);
    body
}
fn path(protocol: u8) -> &'static str {
    match protocol {
        0 => "/v1/chat/completions",
        1 => "/v1/messages",
        _ => "/v1/responses",
    }
}
async fn send(
    state: Arc<neuro_gateway::state::AppState>,
    protocol: u8,
    body: Value,
) -> (StatusCode, Vec<u8>) {
    let response = timeout(
        Duration::from_secs(10),
        build_router(state)
            .oneshot(
                Request::post(path(protocol))
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
        .to_bytes()
        .to_vec();
    (status, bytes)
}
fn issued_turn(bytes: &[u8], stream: bool) -> Value {
    let frames: Vec<Value> = if stream {
        std::str::from_utf8(bytes)
            .unwrap()
            .split("\n\n")
            .filter_map(|f| f.lines().find_map(|l| l.strip_prefix("data: ")))
            .filter(|s| *s != "[DONE]")
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    } else {
        vec![serde_json::from_slice(bytes).unwrap()]
    };
    frames
        .iter()
        .find_map(|v| v.get("response").unwrap_or(v).get("gateway_bedrock_turn"))
        .expect("production-issued turn")
        .clone()
}
async fn verify(protocol: u8, stream: bool) {
    let server = Server::new().await;
    let fixture=super::support::ConsoleStateFixture::new(serde_json::from_value(json!({
        "providers":[{"id":"bedrock-history","adapter":"bedrock_converse_compatible","protocol_profile":"aws_bedrock",
          "base_url":server.base,"supported_models":["fixture-model"],"extra_body":{"system":[{"text":"policy-secret"}]},
          "credentials":[{"id":"exact-account","api_key":"synthetic-only","supported_models":["fixture-model"]}]}],
        "model_routes":[{"pattern":"fixture-model","provider_ids":["bedrock-history"]}],"aliases":{}})).unwrap(),false);
    let selected = fixture
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("fixture-model"));
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected[0].provider_account_id);
    let mut turns = Vec::new();
    for round in 0..3 {
        // Also exercise JSON-to-SSE and SSE-to-JSON handoff without changing the signature prefix.
        let stream = if round == 2 { !stream } else { stream };
        let (status, bytes) = send(
            fixture.state.clone(),
            protocol,
            request(protocol, &turns, stream),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "protocol={protocol}, round={round}: {}",
            String::from_utf8_lossy(&bytes)
        );
        assert!(!String::from_utf8_lossy(&bytes).contains("policy-secret"));
        let turn = issued_turn(&bytes, stream);
        assert_eq!(turn["content"], fixture::content());
        assert_eq!(turn["continuation"]["source"], "aws_bedrock_converse");
        assert_eq!(turn["continuation"]["model"], "fixture-model");
        let bodies = server.bodies.lock().unwrap();
        assert_eq!(bodies.len(), round + 1, "exactly one HTTP send per round");
        let body = &bodies[round];
        assert!(
            body.get("gateway_bedrock_history").is_none()
                && body.get("gateway_bedrock_turn").is_none()
        );
        assert_eq!(body["system"], json!([{"text":"policy-secret"}]));
        assert_eq!(body["messages"].as_array().unwrap().len(), round * 2 + 1);
        for i in 0..round {
            assert_eq!(body["messages"][i * 2 + 1]["content"], fixture::content());
            assert_eq!(
                body["messages"][i * 2 + 2]["content"][0]["toolResult"],
                json!({"toolUseId":"call-a","content":[{"text":"done"}]})
            );
        }
        let prefix = json!({"messages":body["messages"],"system":body.get("system")});
        assert_eq!(
            turn["continuation"]["request_sha256"],
            format!("{:x}", Sha256::digest(serde_json::to_vec(&prefix).unwrap()))
        );
        drop(bodies);
        timeout(Duration::from_secs(2), async {
            while controller.snapshot().consecutive_successes != round + 1
                || controller.snapshot().active_count != 0
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        turns.push(turn);
    }
    for mode in 0..3 {
        let mut changed = request(protocol, &turns, true);
        match mode {
            0 => {
                let key = if protocol == 2 { "input" } else { "messages" };
                changed[key][0]["content"] = json!("changed old user");
            }
            1 => changed["gateway_bedrock_history"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1),
            _ => {
                changed["gateway_bedrock_history"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }
        }
        let (status, _) = send(fixture.state.clone(), protocol, changed).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            server.bodies.lock().unwrap().len(),
            3,
            "mismatched history must fail before HTTP"
        );
    }
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(controller.snapshot().consecutive_successes, 3);
    server.finish().await;
}
fn run(protocol: u8, stream: bool) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    timeout(Duration::from_secs(50), verify(protocol, stream))
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn chat_json_issued_turn_continues_for_three_rounds() {
    run(0, false);
}
#[test]
fn messages_json_issued_turn_continues_for_three_rounds() {
    run(1, false);
}
#[test]
fn responses_json_issued_turn_continues_for_three_rounds() {
    run(2, false);
}
#[test]
fn chat_sse_issued_turn_continues_for_three_rounds() {
    run(0, true);
}
#[test]
fn messages_sse_issued_turn_continues_for_three_rounds() {
    run(1, true);
}
#[test]
fn responses_sse_issued_turn_continues_for_three_rounds() {
    run(2, true);
}
