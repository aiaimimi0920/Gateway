//! Cohere prompt tests must use native Chat and exactly the selected credential.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{db, http::router::build_router, local_runtime::LocalRuntime};
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
    assert_eq!(body["model"], "command-r7b-12-2024");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 32);
    assert_eq!(body["messages"][0]["content"], "Reply with only OK.");
    let key = headers["authorization"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(key.clone());
    match key.as_str() {
        "Bearer bad-secret" => (StatusCode::UNAUTHORIZED, "bad-secret".into()),
        "Bearer limited-secret" => (StatusCode::TOO_MANY_REQUESTS, "limited-secret".into()),
        "Bearer partial-secret" => (StatusCode::OK,
            "event: content-delta\ndata: {\"type\":\"content-delta\",\"delta\":{\"message\":{\"content\":{\"text\":\"partial\"}}}}\n\n".into()),
        _ => {
            let text = if key == "Bearer empty-secret" { " " } else { "OK good-secret" };
            let content = json!({"type":"content-delta","delta":{"message":{"content":{"text":text}}}});
            let end = json!({"type":"message-end","delta":{"finish_reason":"COMPLETE",
                "usage":{"tokens":{"input_tokens":8.0,"output_tokens":2.0},
                "billed_units":{"input_tokens":3,"output_tokens":1}}}});
            (StatusCode::OK, format!("event: content-delta\ndata: {content}\n\nevent: message-end\ndata: {end}\n\n"))
        }
    }
}

async fn send(
    state: &Arc<neuro_gateway::state::AppState>,
    grant: &str,
    id: &str,
    model: &str,
) -> Value {
    let response = build_router(state.clone())
        .oneshot(
            Request::post("/v1/internal/gateway/console/providers/cohere-custom-id/probe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
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
    parse_json(response).await
}

#[tokio::test]
async fn cohere_prompt_probe_requires_completed_native_output_and_exact_account() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v2/chat", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials = ["good", "bad", "partial", "empty", "limited", "disabled"].map(|id| {
        json!({
            "id":id,"api_key":format!("{id}-secret"),"enabled":id!="disabled",
            "supported_models":["command-r7b-12-2024"]
        })
    });
    let document = serde_json::from_value(json!({"providers":[{
        "id":"cohere-custom-id","preset":"cohere-chat","base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "supported_models":["command-r7b-12-2024"],"credentials":credentials
    }],"model_routes":[],"aliases":{}}))
    .unwrap();
    let mut fixture = ConsoleStateFixture::new(document, false);
    let directory = std::env::temp_dir().join(format!("cohere-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, expected) in [
        ("good", "passed"),
        ("bad", "failed"),
        ("partial", "failed"),
        ("empty", "failed"),
        ("limited", "failed"),
        ("disabled", "unsupported"),
    ] {
        let body = send(&fixture.state, &grant, id, "command-r7b-12-2024").await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["credentialId"], id);
        assert_eq!(result["status"], expected, "{body}");
        assert!(!body.to_string().contains("-secret"), "{body}");
    }
    let unsupported = send(&fixture.state, &grant, "good", "unconfigured-model").await;
    assert_eq!(unsupported["result"]["unsupportedCount"], 1);
    assert_eq!(
        calls.lock().unwrap().len(),
        5,
        "No fallback or hidden retry is allowed"
    );
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 5);
    let good = audits
        .iter()
        .find(|audit| audit.status == "completed")
        .unwrap();
    assert_eq!(good.prompt_tokens, Some(8));
    assert_eq!(good.completion_tokens, Some(2));
    assert_eq!(good.total_tokens, Some(10));
    assert_eq!(good.endpoint_kind, "chat_completions");
    server.abort();
    let _ = server.await;
    local.close().await;
    drop(fixture);
    drop(local);
}
