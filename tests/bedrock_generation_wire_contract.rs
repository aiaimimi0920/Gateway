//! Actual production HTTP sender, synthetic legacy-auth loopback only; no live AWS calls.
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;
use axum::{
    extract::{OriginalUri, State},
    routing::post,
    Json, Router,
};
use futures::FutureExt;
use neuro_gateway::{
    protocol::{anthropic, openai, responses},
    routing::candidate::{ProviderAccountPayload, ProviderExecutionMode},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

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
        let captured = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route("/model/fixture/converse", post(receive))
            .route("/model/fixture/converse-stream", post(receive))
            .layer(axum::extract::DefaultBodyLimit::max(65536))
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
    let (kind, body) = if uri.path().ends_with("-stream") {
        (
            "application/vnd.amazon.eventstream",
            wire::sequence().concat(),
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
    axum::response::IntoResponse::into_response(([("content-type", kind)], body))
}

#[tokio::test]
async fn three_source_protocols_send_nested_limits_on_buffered_and_stream_wire() {
    let server = Server::new().await;
    let provider: ProviderAccountPayload =
        serde_json::from_value(json!({"adapter":"bedrock_converse_compatible",
        "base_url":server.base,"api_key":"synthetic-only","execution_mode":"direct_http"}))
        .unwrap();
    let client = UpstreamClient::new(5);
    for source in 0..3 {
        for mode in 0..3 {
            let streaming = mode == 2;
            let req=match source {
                0=>openai::normalize_chat_completions(json!({"model":"fixture","messages":[{"role":"user","content":"hi"}],
                    "max_tokens":17,"temperature":0,"top_p":0.75,"stop":["终"],"stream":streaming})).unwrap(),
                1=>responses::normalize_responses(json!({"model":"fixture","input":"hi","max_output_tokens":17,
                    "temperature":0,"top_p":0.75,"stream":streaming})).unwrap(),
                _=>anthropic::normalize_messages(json!({"model":"fixture","messages":[{"role":"user","content":"hi"}],
                    "max_tokens":17,"temperature":0,"top_p":0.75,"stop_sequences":["终"],"stream":streaming})).unwrap(),
            };
            timeout(Duration::from_secs(8), async {
                if streaming {
                    // The returned stream type is intentionally private. This contract checks
                    // the actual request body; existing EventStream contracts own response EOF.
                    drop(
                        client
                            .execute_stream(&provider, &req, "fixture", None)
                            .boxed()
                            .await
                            .unwrap(),
                    );
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
                }
            })
            .await
            .unwrap();
            let captured = server.captured.lock().unwrap();
            assert_eq!(captured.len(), source * 3 + mode + 1);
            let (path, body) = captured.last().unwrap();
            assert_eq!(
                path,
                if mode != 0 {
                    "/model/fixture/converse-stream"
                } else {
                    "/model/fixture/converse"
                }
            );
            assert_eq!(body["inferenceConfig"]["maxTokens"], 17);
            assert_eq!(body["inferenceConfig"]["temperature"], 0);
            assert_eq!(body["inferenceConfig"]["topP"], 0.75);
            if source != 1 {
                assert_eq!(body["inferenceConfig"]["stopSequences"], json!(["终"]));
            }
            for key in [
                "max_tokens",
                "max_output_tokens",
                "temperature",
                "top_p",
                "stop",
                "stop_sequences",
            ] {
                assert!(body.get(key).is_none());
            }
        }
    }
    server.finish().await;
}

#[tokio::test]
async fn invalid_generation_and_provider_defaults_are_rejected_before_http_send() {
    let server = Server::new().await;
    let mut provider: ProviderAccountPayload =
        serde_json::from_value(json!({"adapter":"bedrock_converse_compatible",
        "base_url":server.base,"api_key":"synthetic-only","execution_mode":"direct_http"}))
        .unwrap();
    let client = UpstreamClient::new(5);
    for defaults in [false, true] {
        provider.extra_body = if defaults {
            Some(serde_json::from_value(json!({"inferenceConfig":{"maxTokens":0}})).unwrap())
        } else {
            None
        };
        let mut body = json!({"model":"fixture","messages":[{"role":"user","content":"hi"}]});
        if !defaults {
            body["max_tokens"] = json!(0);
        }
        let req = openai::normalize_chat_completions(body).unwrap();
        let error = timeout(
            Duration::from_secs(8),
            client.execute(&provider, &req, "fixture", None).boxed(),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
        let error = timeout(
            Duration::from_secs(8),
            client
                .execute_json_passthrough(
                    "fixture",
                    ProviderExecutionMode::DirectHttp,
                    &provider,
                    &req,
                    "fixture",
                    None,
                )
                .boxed(),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
        let error = timeout(
            Duration::from_secs(8),
            client
                .execute_stream(&provider, &req, "fixture", None)
                .boxed(),
        )
        .await
        .unwrap()
        .err()
        .expect("Invalid stream config");
        assert_eq!(error.code.as_deref(), Some("bedrock_generation_parameters"));
    }
    assert!(server.captured.lock().unwrap().is_empty());
    server.finish().await;
}
