//! Gemini API prompt tests bind one credential and never turn quota refusal into success.
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
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 256);
    assert!(!headers.contains_key("authorization"));
    let key = headers["x-goog-api-key"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(key.clone());
    if key == "bad-secret" {
        return (
            StatusCode::UNAUTHORIZED,
            json!({"error":{"code":401,"message":"bad-secret"}}).to_string(),
        );
    }
    if key == "limited-secret" {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            json!({"error":{"code":429,"message":"quota exhausted limited-secret"}}).to_string(),
        );
    }
    let chunk = json!({"candidates":[{"content":{"role":"model","parts":[{"text":if key == "empty-secret" { " " } else { "OK good-secret" }}]},
        "finishReason":if key == "partial-secret" { "MAX_TOKENS" } else { "STOP" }}],"usageMetadata":{"promptTokenCount":8,"candidatesTokenCount":2,"totalTokenCount":10}});
    (StatusCode::OK, format!("data: {chunk}\n\n"))
}

#[tokio::test]
async fn gemini_prompt_probe_uses_exact_account_and_requires_completed_text() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route(
            "/v1beta/models/gemini-2.5-flash:streamGenerateContent",
            post(upstream),
        )
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials = ["good", "bad", "partial", "empty", "limited", "disabled"].map(|id| json!({
        "id":id,"api_key":format!("{id}-secret"),"enabled":id!="disabled","supported_models":["gemini-2.5-flash"]}));
    let document = serde_json::from_value(json!({"providers":[{"id":"gemini-api","preset":"gemini-api","base_url":format!("http://{address}/v1beta"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "supported_models":["gemini-2.5-flash"],"credentials":credentials}],"model_routes":[],"aliases":{}})).unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, model, expected) in [
        ("good", "gemini-2.5-flash", "passed"),
        ("bad", "gemini-2.5-flash", "failed"),
        ("partial", "gemini-2.5-flash", "failed"),
        ("empty", "gemini-2.5-flash", "failed"),
        ("limited", "gemini-2.5-flash", "failed"),
        ("disabled", "gemini-2.5-flash", "unsupported"),
        ("good", "unknown-model", "unsupported"),
    ] {
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post("/v1/internal/gateway/console/providers/gemini-api/probe")
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
