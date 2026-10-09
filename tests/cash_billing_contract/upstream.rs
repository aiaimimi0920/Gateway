use axum::{
    body::Body,
    extract::State,
    http::header::CONTENT_TYPE,
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::Semaphore;

pub struct Control {
    pub calls: AtomicUsize,
    pub block: AtomicBool,
    pub permits: Semaphore,
    pub usage: Mutex<Option<Value>>,
    pub stalled_stream: AtomicBool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            block: AtomicBool::new(false),
            permits: Semaphore::new(0),
            usage: Mutex::new(Some(
                json!({"prompt_tokens":100,"completion_tokens":50,"total_tokens":150,
                "prompt_tokens_details":{"cached_tokens":30}}),
            )),
            stalled_stream: AtomicBool::new(false),
        }
    }
}

pub async fn reply(State(control): State<Arc<Control>>, Json(body): Json<Value>) -> Response {
    control.calls.fetch_add(1, Ordering::SeqCst);
    if control.block.load(Ordering::SeqCst) {
        control.permits.acquire().await.unwrap().forget();
    }
    let usage = control.usage.lock().unwrap().clone();
    if body["stream"] == true {
        let chunk = json!({"id":"fixture", "object":"chat.completion.chunk", "model":body["model"],
            "choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":null}]});
        let first = format!("data: {chunk}\n\n");
        let response_body = if control.stalled_stream.load(Ordering::SeqCst) {
            let frames = futures::stream::iter([Ok::<_, std::io::Error>(first)])
                .chain(futures::stream::pending());
            Body::from_stream(frames)
        } else {
            let mut terminal = json!({"id":"fixture", "object":"chat.completion.chunk", "model":body["model"],
                "choices":[{"index":0,"delta":{},"finish_reason":"stop"}]});
            if let Some(usage) = usage {
                terminal["usage"] = usage;
            }
            Body::from(format!("{first}data: {terminal}\n\ndata: [DONE]\n\n"))
        };
        return ([(CONTENT_TYPE, "text/event-stream")], response_body).into_response();
    }
    let mut response = json!({"id":"fixture", "object":"chat.completion", "model":body["model"],
        "choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]});
    if let Some(usage) = usage {
        response["usage"] = usage;
    }
    Json(response).into_response()
}

pub async fn anthropic(State(control): State<Arc<Control>>, Json(body): Json<Value>) -> Response {
    control.calls.fetch_add(1, Ordering::SeqCst);
    let usage = json!({"input_tokens":70,"output_tokens":50,"cache_read_input_tokens":20,
        "cache_creation_input_tokens":10});
    if body["stream"] == true {
        let start = json!({"type":"message_start", "message":{"id":"fixture","type":"message",
            "role":"assistant","model":body["model"],"content":[],"stop_reason":null,
            "usage":{"input_tokens":70,"output_tokens":0,"cache_read_input_tokens":20,
                "cache_creation_input_tokens":10}}});
        let delta = json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},
            "usage":{"output_tokens":50}});
        return ([(CONTENT_TYPE, "text/event-stream")],
            format!("event: message_start\ndata: {start}\n\nevent: content_block_start\ndata: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\nevent: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"ok\"}}}}\n\nevent: content_block_stop\ndata: {{\"type\":\"content_block_stop\",\"index\":0}}\n\nevent: message_delta\ndata: {delta}\n\nevent: message_stop\ndata: {{\"type\":\"message_stop\"}}\n\n")
        ).into_response();
    }
    Json(
        json!({"id":"fixture", "type":"message", "role":"assistant", "model":body["model"],
        "content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":usage}),
    )
    .into_response()
}
