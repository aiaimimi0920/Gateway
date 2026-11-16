//! Real public HTTP SSE must expose a delta before real EOF, but never success before original FINISH.
use super::*;
mod controls;
mod failures;
use futures::StreamExt;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
#[derive(Default)]
pub(super) struct Gate {
    pub eof: tokio::sync::Notify,
    pub dropped: AtomicBool,
}
struct BodyOwner {
    body:
        Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, std::convert::Infallible>> + Send>>,
    gate: Arc<Gate>,
}
impl futures::Stream for BodyOwner {
    type Item = Result<bytes::Bytes, std::convert::Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.body.as_mut().poll_next(cx)
    }
}
impl Drop for BodyOwner {
    fn drop(&mut self) {
        self.gate.dropped.store(true, Ordering::SeqCst);
    }
}
pub(super) fn stream(mode: &str, gate: Arc<Gate>) -> Option<axum::response::Response> {
    use axum::{body::Body, http::header::CONTENT_TYPE, response::IntoResponse};
    if !mode.starts_with("public_sse") {
        return None;
    }
    let mut value = json!({"id":"native-id","model":"fbm1.open","choices":[{"index":0,"delta":{"role":"assistant","content":"partial","reasoning_content":"native thought","reasoning_details":[{"opaque":1}]},"finish_reason":"stop"}]});
    if mode != "public_sse_unknown" {
        value["usage"] =
            json!({"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0.125});
    }
    if mode == "public_sse_cost_only" {
        value["usage"] = json!({"cost":0.125});
    }
    if mode == "public_sse_length" {
        value["choices"][0]["finish_reason"] = json!("length");
    }
    if mode == "public_sse_filter" {
        value["choices"][0]["finish_reason"] = json!("content_filter");
    }
    let frame = if mode == "public_sse_tools" {
        value["choices"][0]["finish_reason"] = Value::Null;
        value["choices"][0]["delta"]["tool_calls"] = json!([{"index":0,"id":"tool-1","type":"function","function":{"name":"caller_only","arguments":"{\"x\":"},"extra_content":{"opaque":"keep"}}]);
        let next = json!({"id":"native-id","model":"fbm1.open","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0.25}});
        format!("data: {value}\n\ndata: {next}\n\ndata: [DONE]\n\n")
    } else {
        format!("data: {value}\n\ndata: [DONE]\n\n")
    };
    let wait = gate.clone();
    let tail = mode.to_owned();
    let body = futures::stream::once(async move { Ok(bytes::Bytes::from(frame)) }).chain(
        futures::stream::once(async move {
            wait.eof.notified().await;
            Ok(bytes::Bytes::from(if tail == "public_sse_bad" {
                "data: {bad}\n\n"
            } else {
                ""
            }))
        }),
    );
    Some(
        (
            [(CONTENT_TYPE, "text/event-stream")],
            Body::from_stream(BodyOwner {
                body: Box::pin(body),
                gate,
            }),
        )
            .into_response(),
    )
}
pub(super) fn request_stream(host: &Host) -> tokio::task::JoinHandle<rquest::Response> {
    let client = host.client.clone();
    let token = host.token.clone();
    let origin = host.server.origin.clone();
    tokio::spawn(async move {
        let mut body = request();
        body["stream"] = json!(true);
        client
            .post(format!("{origin}/v1/chat/completions"))
            .bearer_auth(token)
            .header("cookie", "forged-caller-cookie")
            .json(&body)
            .send()
            .await
            .unwrap()
    })
}
pub(super) async fn approve<T>(host: &Host, task: &tokio::task::JoinHandle<T>) {
    let grant = host.grant().await;
    let view = host.pending(&grant, task).await;
    assert_eq!(host.decision(&grant, &view, "approve").await, 200);
}
#[tokio::test]
async fn freebuff_public_chat_sse_delta_precedes_eof_but_done_waits_for_original_finish_ack() {
    let peer = peer::Peer::new().await;
    *peer.mode.lock().unwrap() = "public_sse_known".into();
    let host = Box::pin(Host::new(&peer)).await;
    host.seed_device(&peer).await;
    let pause = peer.pause("/api/v1/agent-runs", 2);
    let task = request_stream(&host);
    approve(&host, &task).await;
    let response = task.await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let mut bytes = response.bytes_stream();
    let first = tokio::time::timeout(Duration::from_secs(3), bytes.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let first = String::from_utf8(first.to_vec()).unwrap();
    assert!(first.contains("partial"));
    assert!(first.contains("native thought"));
    assert!(!first.contains("\"finish_reason\":\"stop\""));
    assert!(!first.contains("[DONE]"));
    assert!(peer.finishes().is_empty());
    assert_eq!(
        host.state
            .concurrency_registry
            .get_or_create("pool")
            .snapshot()
            .active_count,
        1
    );
    peer.live.eof.notify_one();
    // Reading the public body co-polls the original owner; there is no detached generator.
    let read = async {
        let mut rest = Vec::new();
        while let Some(chunk) = bytes.next().await {
            rest.extend_from_slice(&chunk.unwrap());
        }
        String::from_utf8(rest).unwrap()
    };
    tokio::pin!(read);
    tokio::select! { _=&mut read=>panic!("public terminal preceded FINISH ack"), _=pause.reached()=>{} }
    assert_eq!(peer.finishes()[0]["totalCredits"], 13);
    pause.resume();
    let rest = tokio::time::timeout(Duration::from_secs(3), read)
        .await
        .unwrap();
    assert!(rest.contains("\"finish_reason\":\"stop\""));
    assert!(rest.contains("[DONE]"));
    assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
    let sent = peer
        .hits
        .lock()
        .unwrap()
        .iter()
        .find(|(_, path, _, _)| path == "/api/v1/chat/completions")
        .unwrap()
        .3
        .clone();
    assert_eq!(
        sent["stream"], true,
        "the original signed request must actually request native SSE"
    );
    settled(&host, "completed", 9).await;
    host.close().await;
    peer.server.close().await;
}
