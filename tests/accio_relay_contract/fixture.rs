//! Synthetic Accio upstream; never opens a real provider or imports local credentials.
#![allow(dead_code)]
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    routing::post,
    Json, Router,
};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle};

pub const MODEL: &str = "claude-sonnet-4-6";
pub type Calls = Arc<Mutex<Vec<(String, HeaderMap, Value)>>>;
pub fn event(raw: Value) -> Vec<u8> {
    format!("data: {}\n\n", json!({"raw_response_json":raw.to_string()})).into_bytes()
}
pub fn text(value: &str) -> Vec<u8> {
    event(
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":value}}),
    )
}
pub fn end() -> Vec<u8> {
    [event(json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{
        "input_tokens":5,"output_tokens":2,"total_tokens":9,"cache_creation_input_tokens":3,"cache_read_input_tokens":4
    }})), event(json!({"type":"message_stop"}))].concat()
}
pub fn error() -> Vec<u8> {
    b"data: {\"turn_complete\":true,\"error_code\":\"5015\",\"error_message\":\"user not activated synthetic-private-upstream\"}\n\n".to_vec()
}
pub fn chunks(case: &str) -> Vec<Vec<u8>> {
    match case {
        "empty" => vec![],
        "garbage" => vec![b"garbage".to_vec()],
        "truncated" => vec![text("partial")],
        "mid-error" => vec![text("partial"), error()],
        "split-error" => {
            let e = error();
            vec![e[..23].to_vec(), e[23..].to_vec()]
        }
        "tail" => vec![text("OK"), end(), b"data: {broken\n\n".to_vec()],
        "usage-only" => vec![event(json!({"metadata":{"usage":{"inputTokens":5}}}))],
        "done-only" => vec![b"data: [DONE]\n\n".to_vec()],
        "orphan-tool" => vec![
            event(
                json!({"contentBlockDelta":{"contentBlockIndex":9,"delta":{"partial_json":"{\"x\":1}"}}}),
            ),
            b"data: [DONE]\n\n".to_vec(),
        ],
        "length" => vec![
            text("partial"),
            event(json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"}})),
            event(json!({"type":"message_stop"})),
        ],
        "tool" => vec![
            event(
                json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call-1","name":"weather"}}),
            ),
            event(
                json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"city\":\"杭州\"}"}}),
            ),
            event(json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}})),
            event(json!({"type":"message_stop"})),
        ],
        "echo" => vec![
            text("OK echo-secret echo-device echo-sg echo-refresh"),
            end(),
        ],
        "two-tools" => vec![
            event(json!({"candidates":[{"content":{"parts":[
            {"functionCall":{"id":"call-1","name":"first","args":{"a":1}}},
            {"functionCall":{"id":"call-2","name":"second","args":{"b":2}}}
        ]},"finishReason":"tool_use"}]})),
            b"data: [DONE]\n\n".to_vec(),
        ],
        _ => vec![text("OK 你好"), end()],
    }
}
#[derive(Clone)]
struct ServerState {
    calls: Calls,
    case: Option<String>,
    dropped: Arc<AtomicBool>,
}
struct DropSignal(Arc<AtomicBool>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
async fn upstream(
    State(state): State<ServerState>,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, [(&'static str, &'static str); 1], Body) {
    let token = body["token"].as_str().unwrap_or_default();
    let case = state
        .case
        .as_deref()
        .unwrap_or_else(|| token.strip_suffix("-secret").unwrap_or(token));
    let status = if case == "unauthorized" {
        StatusCode::UNAUTHORIZED
    } else {
        StatusCode::OK
    };
    let parts = chunks(case);
    let hang = case == "hang";
    state
        .calls
        .lock()
        .unwrap()
        .push((uri.path().to_string(), headers, body));
    let signal = DropSignal(state.dropped);
    let stream = futures::stream::unfold(
        (parts.into_iter(), signal),
        move |(mut parts, signal)| async move {
            let next = parts.next();
            if next.is_none() && hang {
                futures::future::pending::<()>().await;
            }
            let bytes = next?;
            tokio::time::sleep(Duration::from_millis(15)).await;
            Some((
                Ok::<_, Infallible>(bytes::Bytes::from(bytes)),
                (parts, signal),
            ))
        },
    );
    (
        status,
        [("content-type", "text/event-stream")],
        Body::from_stream(stream),
    )
}
pub struct Upstream {
    pub base: String,
    pub calls: Calls,
    pub dropped: Arc<AtomicBool>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Upstream {
    pub async fn start(case: Option<&str>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let calls = Calls::default();
        let dropped = Arc::new(AtomicBool::new(false));
        let app = Router::new()
            .route("/api/adk/llm/generateContent", post(upstream))
            .fallback(upstream)
            .with_state(ServerState {
                calls: calls.clone(),
                case: case.map(str::to_owned),
                dropped: dropped.clone(),
            });
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap()
        });
        Self {
            base,
            calls,
            dropped,
            stop: Some(stop),
            task,
        }
    }
    pub fn document(&self, ids: &[&str]) -> RouteConfigYaml {
        serde_json::from_value(json!({"providers":[{"id":"accio-test","preset":"accio","base_url":self.base,
            "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"accio"},
            "explicit_credentials_only":true,"supported_models":[MODEL],"model_map":{"probe-alias":MODEL},
            "credentials":ids.iter().map(|id| json!({"id":id,"api_key":format!("{id}-secret"),"enabled":*id!="disabled",
                "headers":{"utdid":format!("{id}-device")},"extra_body":{"token":"stale-legacy-token"},"accio_sgcookie":format!("{id}-sg"),
                "refresh_token":format!("{id}-refresh"),"supported_models":[MODEL]})).collect::<Vec<_>>()
        }],"model_routes":[{"pattern":MODEL,"provider_ids":["accio-test"]}],"aliases":{}})).unwrap()
    }
    pub async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        if tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            let _ = (&mut self.task).await;
        }
    }
}
impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn on_runtime<F: Future<Output = ()>>(work: impl FnOnce() -> F + Send + 'static) {
    // Match main.rs's 8 MiB default worker budget, not Windows test threads' 2 MiB default.
    // Construct the future on its owner thread; current-thread SQLite work need not be Send.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(work());
        })
        .unwrap()
        .join()
        .unwrap();
}
