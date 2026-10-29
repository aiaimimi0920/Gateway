//! Public Cohere bridges must negotiate native SSE, not mistake native JSON for a stream.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use http_body_util::BodyExt;
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

type Calls = Arc<Mutex<Vec<Value>>>;
async fn upstream(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> String {
    assert_eq!(headers["authorization"], "Bearer synthetic-cohere-only");
    assert_eq!(body["model"], "command-r7b-12-2024");
    assert_eq!(body["max_tokens"], 32);
    assert_eq!(body["stream"], true);
    assert!(body.get("input").is_none() && body.get("max_output_tokens").is_none());
    calls.lock().unwrap().push(body);
    if headers.get("accept").and_then(|value| value.to_str().ok()) != Some("text/event-stream") {
        return json!({"message":{"content":[{"type":"text","text":"OK"}]},"finish_reason":"COMPLETE"}).to_string();
    }
    concat!("event: content-delta\ndata: {\"type\":\"content-delta\",\"delta\":{\"message\":{\"content\":{\"text\":\"OK\"}}}}\n\n",
        "event: message-end\ndata: {\"type\":\"message-end\",\"delta\":{\"finish_reason\":\"COMPLETE\",\"usage\":{\"tokens\":{\"input_tokens\":8,\"output_tokens\":2}}}}\n\n").into()
}

async fn verify_async(endpoint: &str, streaming: bool) {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v2/chat", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"cohere","preset":"cohere-chat","base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "supported_models":["command-r7b-12-2024"],"credentials":[{
            "id":"exact-account","api_key":"synthetic-cohere-only","supported_models":["command-r7b-12-2024"]
        }]
    }],"model_routes":[{"pattern":"command-r7b-12-2024","provider_ids":["cohere"]}],"aliases":{}})).unwrap(), false);
    let mut body = json!({"model":"command-r7b-12-2024","stream":streaming});
    if endpoint == "responses" {
        body["input"] = json!("Reply with only OK.");
        body["max_output_tokens"] = json!(32);
    } else {
        body["messages"] = json!([{"role":"user","content":"Reply with only OK."}]);
        body["max_tokens"] = json!(32);
    }
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post(format!("/v1/{endpoint}"))
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                // An inbound preference must not defeat a forced-streaming upstream owner.
                .header("accept", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    server.abort();
    let _ = server.await;
    assert_eq!(status, StatusCode::OK, "{endpoint} {streaming}: {text}");
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "No request may require a hidden retry"
    );
    if streaming {
        assert!(text.contains("OK"), "{text}");
        assert!(
            text.contains(if endpoint == "responses" {
                "response.completed"
            } else {
                "[DONE]"
            }),
            "{text}"
        );
    } else {
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        if endpoint == "responses" {
            assert_eq!(body["status"], "completed");
            assert_eq!(body["output"][0]["content"][0]["text"], "OK");
        } else {
            assert_eq!(body["choices"][0]["message"]["content"], "OK");
            assert_eq!(body["choices"][0]["finish_reason"], "stop");
        }
    }
}

fn verify(endpoint: &'static str, streaming: bool) {
    // Match the real pipeline's stack budget, not Windows test threads' 2 MiB default.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(verify_async(endpoint, streaming));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn cohere_chat_buffered_bridge_negotiates_native_sse() {
    verify("chat/completions", false);
}
#[test]
fn cohere_chat_stream_bridge_negotiates_native_sse() {
    verify("chat/completions", true);
}
#[test]
fn cohere_responses_buffered_bridge_routes_to_native_chat() {
    verify("responses", false);
}
#[test]
fn cohere_responses_stream_bridge_routes_to_native_chat() {
    verify("responses", true);
}
