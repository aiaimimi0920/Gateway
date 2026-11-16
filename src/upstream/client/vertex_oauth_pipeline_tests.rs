//! Full Vertex public protocols exercise their own typed OAuth route and local settlement owner.
use super::vertex_oauth_test_support as wire;
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
mod failures;
mod fixture;
mod preview;
mod settlement;
mod signed;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
mod tools;
use fixture::*;

const MODEL: &str = "public-vertex";
const ACCESS: &str = "synthetic-exact-vertex-access";
const PRIVATE: &str = "synthetic-private-upstream-detail";

fn body(endpoint: &str, stream: bool) -> Value {
    match endpoint {
        "responses" => json!({"model":MODEL,"input":"合成问题","stream":stream}),
        "messages" => {
            json!({"model":MODEL,"max_tokens":32,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
        _ => {
            json!({"model":MODEL,"messages":[{"role":"user","content":"合成问题"}],"stream":stream})
        }
    }
}
fn event(raw: Value) -> Vec<u8> {
    format!("data: {raw}\n\n").into_bytes()
}
fn candidate(reason: &str) -> Value {
    json!({"candidates":[{"index":0,"content":{"role":"model","parts":[{"text":"合成答案"}]},"finishReason":reason}]})
}
fn success_body() -> Vec<u8> {
    let mut value = candidate("STOP");
    value["usageMetadata"] =
        json!({"promptTokenCount":2,"candidatesTokenCount":3,"totalTokenCount":5});
    event(value)
}
async fn drain(body: Body) -> String {
    String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap()
}
fn assert_wire(server: &wire::Server, max_attempts: usize) {
    let calls = server.calls.lock().unwrap();
    assert!(
        !calls.is_empty() && calls.len() <= max_attempts,
        "{} attempts",
        calls.len()
    );
    for (path, headers, raw) in calls.iter() {
        assert_eq!(path, "/v1/projects/project-a/locations/global/publishers/google/models/gemini-contract:streamGenerateContent?alt=sse");
        assert_eq!(headers["authorization"], format!("Bearer {ACCESS}"));
        assert_eq!(headers["x-goog-user-project"], "quota-a");
        assert!(!headers.contains_key("cookie") && !headers.contains_key("x-goog-api-key"));
        assert_eq!(raw["contents"][0]["parts"][0]["text"], "合成问题");
    }
}
fn assert_identity(row: &db::GatewayRequestAuditView) {
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some("gemini-contract"));
    assert_eq!(row.provider_account_id.as_deref(), Some("vertex-pipeline"));
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
}
#[tokio::test]
async fn vertex_oauth_pipeline_three_protocols_settle_message_balance_and_usage() {
    Box::pin(async {
        for endpoint in ["chat/completions", "messages", "responses"] {
            for stream in [false, true] {
                let server = wire::Server::start(200, success_body()).await;
                let fixture = Box::pin(Fixture::new(&server)).await;
                wire::within(&server.origin, async {
                    let response = fixture.open(endpoint, body(endpoint, stream)).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert!(
                        text.contains("合成答案") && !text.contains(ACCESS),
                        "{text}"
                    );
                    if stream {
                        assert!(
                            text.contains(match endpoint {
                                "responses" => "response.completed",
                                "messages" => "message_stop",
                                _ => "[DONE]",
                            }),
                            "{text}"
                        );
                    }
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (Some(2), Some(3), Some(5))
                    );
                    assert_identity(&row);
                    assert_eq!(fixture.remaining().await, 1);
                    assert_wire(&server, 1);
                })
                .await;
                fixture.finish().await;
                server.stop().await;
            }
        }
    })
    .await;
}
