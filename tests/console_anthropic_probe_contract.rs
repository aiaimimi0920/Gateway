//! Native Messages prompt tests bind one credential and never turn quota refusal into success.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
type Calls = Arc<Mutex<Vec<String>>>;

async fn upstream(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    assert_eq!(body["model"], "claude-sonnet-4-6");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 32);
    assert_eq!(headers["anthropic-version"], "2023-06-01");
    assert!(!headers.contains_key("authorization"));
    let key = headers["x-api-key"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(key.clone());
    if key == "limited-secret" {
        return (
            StatusCode::BAD_REQUEST,
            json!({"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. limited-secret"}}).to_string(),
        );
    }
    if key == "bad-secret" {
        return (
            StatusCode::UNAUTHORIZED,
            json!({"type":"error","error":{"type":"authentication_error","message":"bad-secret"}})
                .to_string(),
        );
    }
    let start = json!({"type":"message_start","message":{"model":"claude-sonnet-4-6","usage":{"input_tokens":8,"output_tokens":0}}});
    let text = json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":if key == "empty-secret" { " " } else { "OK good-secret" }}});
    let finish = json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}});
    let stop = if key == "partial-secret" {
        ""
    } else {
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
    };
    (StatusCode::OK, format!("event: message_start\ndata: {start}\n\nevent: content_block_delta\ndata: {text}\n\nevent: message_delta\ndata: {finish}\n\n{stop}"))
}

#[tokio::test]
async fn anthropic_prompt_probe_uses_exact_account_and_requires_completed_text() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/messages", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials = ["good", "bad", "partial", "empty", "limited", "disabled"].map(|id| json!({
        "id":id,"api_key":format!("{id}-secret"),"enabled":id!="disabled","supported_models":["claude-sonnet-4-6"]}));
    let document = serde_json::from_value(json!({"providers":[{"id":"anthropic-api","preset":"anthropic","base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "supported_models":["claude-sonnet-4-6"],"credentials":credentials}],"model_routes":[],"aliases":{}})).unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, model, expected) in [
        ("good", "claude-sonnet-4-6", "passed"),
        ("bad", "claude-sonnet-4-6", "failed"),
        ("partial", "claude-sonnet-4-6", "failed"),
        ("empty", "claude-sonnet-4-6", "failed"),
        ("limited", "claude-sonnet-4-6", "failed"),
        ("disabled", "claude-sonnet-4-6", "unsupported"),
        ("good", "unknown-model", "unsupported"),
    ] {
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post("/v1/internal/gateway/console/providers/anthropic-api/probe")
                    .header("x-management-token", MANAGEMENT_TOKEN)
                    .header("x-secret-grant", &grant)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"prompt":"Reply with only OK.","model":model,"credentialIds":[id]})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(body["result"]["results"][0]["credentialId"], id);
        assert_eq!(body["result"]["results"][0]["status"], expected, "{body}");
        assert!(!body.to_string().contains("-secret"), "{body}");
        if id == "limited" {
            assert!(body.to_string().contains("InsufficientQuota"), "{body}");
        }
    }
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "good-secret",
            "bad-secret",
            "partial-secret",
            "empty-secret",
            "limited-secret"
        ],
        "Exact credentials, no fallback or hidden retry"
    );
    server.abort();
    let _ = server.await;
}
