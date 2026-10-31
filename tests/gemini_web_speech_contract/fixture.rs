//! Synthetic speech export frames; these containers are not real generated speech.
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use base64::Engine;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub const RPC_PATH: &str = "/_/BardChatUi/data/batchexecute";
pub const APP: &str = r#"<html><script>window.WIZ_global_data = {"SNlM0e":"synthetic-at","cfb2h":"synthetic-build","FdrFJe":"synthetic-session"};</script></html>"#;

pub struct Capture {
    pub gets: usize,
    pub calls: Vec<(HeaderMap, HashMap<String, String>)>,
    pub status: StatusCode,
    pub reply: String,
    pub app_status: StatusCode,
    pub app_reply: String,
}

pub struct Fixture {
    pub base: String,
    pub capture: Arc<Mutex<Capture>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn app(State(capture): State<Arc<Mutex<Capture>>>) -> (StatusCode, HeaderMap, String) {
    let mut state = capture.lock().unwrap();
    state.gets += 1;
    (
        state.app_status,
        [(
            "location".parse().unwrap(),
            "/redirect-target".parse().unwrap(),
        )]
        .into_iter()
        .collect(),
        state.app_reply.clone(),
    )
}

async fn export(
    State(capture): State<Arc<Mutex<Capture>>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, HeaderMap, String) {
    let mut state = capture.lock().unwrap();
    state.calls.push((headers, form));
    (
        state.status,
        [(
            "location".parse().unwrap(),
            "/redirect-target".parse().unwrap(),
        )]
        .into_iter()
        .collect(),
        state.reply.clone(),
    )
}

pub async fn fixture(reply: String) -> Fixture {
    let capture = Arc::new(Mutex::new(Capture {
        gets: 0,
        calls: vec![],
        status: StatusCode::OK,
        reply,
        app_status: StatusCode::OK,
        app_reply: APP.into(),
    }));
    let router = Router::new()
        .route("/app", get(app))
        .route(RPC_PATH, post(export))
        .route("/redirect-target", get(app).post(export))
        .with_state(capture.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    Fixture {
        base,
        capture,
        task,
    }
}

pub fn wav() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(24_000u32.to_le_bytes());
    bytes.extend(48_000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([0, 0, 0, 0]);
    bytes
}

pub fn frame(rpc: &str, bytes: &[u8]) -> String {
    let audio = base64::engine::general_purpose::STANDARD.encode(bytes);
    let frame = json!([[
        "wrb.fr",
        rpc,
        json!([audio]).to_string(),
        null,
        null,
        null,
        "generic"
    ]])
    .to_string();
    format!(")]}}'\n{}\n{frame}\n", frame.encode_utf16().count())
}

pub fn inner(form: &HashMap<String, String>) -> Value {
    let outer: Value = serde_json::from_str(&form["f.req"]).unwrap();
    assert_eq!(outer[0][0][0], "XqA3Ic");
    serde_json::from_str(outer[0][0][1].as_str().unwrap()).unwrap()
}

pub fn ogg() -> Vec<u8> {
    // Three checksum-bound pages: OpusHead, empty OpusTags and one silence packet.
    // Generated locally, not a successful upstream or intelligibility receipt.
    base64::engine::general_purpose::STANDARD.decode("T2dnUwACAAAAAAAAAAAqAAAAAAAAABBer3ABE09wdXNIZWFkAQEAAMBdAAAAAABPZ2dTAAAAAAAAAAAAACoAAAABAAAAkuw1hQEQT3B1c1RhZ3MAAAAAAAAAAE9nZ1MABAAAAAAAAAAAKgAAAAIAAACSSyWZAQP4//4=").unwrap()
}
