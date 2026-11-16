//! Production sender with synthetic legacy-auth loopback; explicit AWS allowlists remain unchanged.
use super::*;
use axum::{
    extract::{OriginalUri, State},
    routing::post,
    Json, Router,
};
use futures::FutureExt;
use neuro_gateway::{
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::client::UpstreamClient,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
#[path = "../bedrock_upstream_eventstream_contract/fixtures.rs"]
mod frames;

type Captured = Arc<Mutex<Vec<(String, Value)>>>;
struct Server {
    base: String,
    captured: Captured,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let captured = Captured::default();
        let app = Router::new()
            .route("/model/fixture/converse", post(receive))
            .route("/model/fixture/converse-stream", post(receive))
            .with_state(captured.clone());
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
            captured,
            stop: Some(stop),
            task,
        }
    }
    fn provider(&self) -> ProviderAccountPayload {
        serde_json::from_value(
            json!({"adapter":"bedrock_converse_compatible","base_url":self.base,
            "api_key":"synthetic-only","execution_mode":"direct_http"}),
        )
        .unwrap()
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
async fn receive(
    State(captured): State<Captured>,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    captured.lock().unwrap().push((uri.path().into(), body));
    let (kind, bytes) = if uri.path().ends_with("-stream") {
        (
            "application/vnd.amazon.eventstream",
            frames::sequence().concat(),
        )
    } else {
        (
            "application/json",
            serde_json::to_vec(
                &json!({"output":{"message":{"role":"assistant","content":[{"text":"OK"}]}},
            "stopReason":"end_turn","usage":{"inputTokens":2,"outputTokens":1,"totalTokens":3}}),
            )
            .unwrap(),
        )
    };
    axum::response::IntoResponse::into_response(([("content-type", kind)], bytes))
}

#[tokio::test]
async fn bedrock_signed_history_reaches_json_buffered_stream_and_sse_senders_unchanged() {
    let server = Server::new().await;
    let provider = server.provider();
    let client = UpstreamClient::new(5);
    let body = history();
    // Erase the large sender future types at the harness boundary, not by
    // raising crate recursion limits or changing production async behavior.
    for mode in 0..3 {
        let req = normalize_converse(body.clone(), Some("fixture".into()), mode == 2).unwrap();
        timeout(Duration::from_secs(8), async {
            if mode == 0 {
                let reply = client
                    .execute_json_passthrough(
                        "fixture",
                        ProviderExecutionMode::DirectHttp,
                        &provider,
                        &req,
                        "fixture",
                        None,
                    )
                    .boxed()
                    .await
                    .unwrap();
                assert_eq!(reply["output"]["message"]["content"][0]["text"], "OK");
            } else if mode == 1 {
                assert_eq!(
                    client
                        .execute(&provider, &req, "fixture", None)
                        .boxed()
                        .await
                        .unwrap()
                        .text,
                    "你好"
                );
            } else {
                // Only request wire is claimed here; native reasoning response decoding is separate.
                drop(
                    client
                        .execute_stream(&provider, &req, "fixture", None)
                        .boxed()
                        .await
                        .unwrap(),
                );
            }
        })
        .await
        .unwrap();
        let calls = server.captured.lock().unwrap();
        assert_eq!(calls.len(), mode + 1);
        let (path, sent) = calls.last().unwrap();
        assert_eq!(
            path,
            if mode == 0 {
                "/model/fixture/converse"
            } else {
                "/model/fixture/converse-stream"
            }
        );
        assert_eq!(sent["messages"], body["messages"]);
        assert_eq!(sent["system"], body["system"]);
    }
    server.finish().await;
}

#[tokio::test]
async fn bedrock_signed_history_policy_conflict_never_reaches_any_http_sender() {
    let server = Server::new().await;
    let provider = server.provider();
    let client = UpstreamClient::new(5);
    for mode in 0..3 {
        let mut req = normalize_converse(history(), Some("fixture".into()), mode == 2).unwrap();
        req.messages[1].content.push(ContentPart::Text {
            text: "modified prefix".into(),
        });
        let error = timeout(Duration::from_secs(8), async {
            if mode == 0 {
                client
                    .execute_json_passthrough(
                        "fixture",
                        ProviderExecutionMode::DirectHttp,
                        &provider,
                        &req,
                        "fixture",
                        None,
                    )
                    .boxed()
                    .await
                    .err()
                    .unwrap()
            } else if mode == 1 {
                client
                    .execute(&provider, &req, "fixture", None)
                    .boxed()
                    .await
                    .err()
                    .unwrap()
            } else {
                client
                    .execute_stream(&provider, &req, "fixture", None)
                    .boxed()
                    .await
                    .err()
                    .unwrap()
            }
        })
        .await
        .unwrap();
        assert_eq!(
            error.code.as_deref(),
            Some("bedrock_reasoning_history_changed")
        );
        assert!(server.captured.lock().unwrap().is_empty());
    }
    server.finish().await;
}
