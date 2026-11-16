//! Random loopback, synthetic tokens and real AWS framing; no AWS audience exception.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Uri},
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
#[path = "../bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

pub struct Upstream {
    pub base: String,
    pub calls: Arc<Mutex<Vec<(String, String, Value)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Upstream {
    pub async fn start() -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(respond).with_state(calls.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base, calls, task }
    }
    pub fn document(&self, model: &str) -> Value {
        let mut value = super::configuration::document();
        let provider = &mut value["providers"][0];
        provider["base_url"] = json!(self.base);
        provider["model_map"] = json!({"alias":model});
        provider["credentials"][0]["model_selection"] = json!({"manual":[model],"fetched":[]});
        value["model_routes"] = json!([{"pattern":"alias","provider_ids":["bedrock"]}]);
        value
    }
}
impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn respond(
    State(calls): State<Arc<Mutex<Vec<(String, String, Value)>>>>,
    uri: Uri,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    let path = uri
        .path()
        .strip_prefix("/model/")
        .unwrap()
        .strip_suffix("/converse-stream")
        .unwrap();
    let model = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .unwrap()
        .into_owned();
    calls.lock().unwrap().push((
        headers["authorization"].to_str().unwrap().into(),
        model,
        body,
    ));
    let frames = [
        wire::event("messageStart", json!({"role":"assistant"})),
        wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":"OK"}}),
        ),
        wire::event("contentBlockStop", json!({"contentBlockIndex":0})),
        wire::event("messageStop", json!({"stopReason":"end_turn"})),
        wire::event(
            "metadata",
            json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}),
        ),
    ];
    axum::response::Response::builder()
        .header("content-type", "application/vnd.amazon.eventstream")
        .body(Body::from(frames.concat()))
        .unwrap()
}
