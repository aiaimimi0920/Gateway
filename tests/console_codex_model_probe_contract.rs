//! Codex probes must generate with the exact selected OAuth account.
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{db, local_runtime::LocalRuntime};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

async fn completion(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    assert_eq!(body["model"], "gpt-5.6-luna");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert!(body["instructions"].is_string());
    assert_eq!(headers["accept"], "text/event-stream");
    let auth = headers["authorization"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(auth.clone());
    let account = headers["chatgpt-account-id"].to_str().unwrap();
    assert_eq!(auth, format!("Bearer {account}-secret"));
    if account == "bad" {
        return (StatusCode::UNAUTHORIZED, "bad-secret".into());
    }
    if account == "partial" {
        return (
            StatusCode::OK,
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"OK\"}\n\n".into(),
        );
    }
    let event = json!({"type":"response.completed", "response":{"status":"completed", "output":[
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK good-secret"}]}],
        "usage":{"input_tokens":8,"output_tokens":1,"total_tokens":9}}});
    (
        StatusCode::OK,
        format!("data: {event}\r\n\r\ndata: [DONE]\r\n\r\n"),
    )
}

#[tokio::test]
async fn codex_probe_requires_completed_reply_and_preserves_account_identity() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/responses", post(completion))
        .with_state(calls.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let credentials = ["good", "bad", "partial"].map(|id| {
        json!({"id":id, "api_key":format!("{id}-secret"),
        "headers":{"Chatgpt-Account-Id":id}})
    });
    let document = serde_json::from_value(json!({"providers":[{"id":"chatgpt",
        "preset":"chatgpt-codex-oauth-official-api", "base_url":format!("http://{address}"),
        "credentials":credentials}],"model_routes":[],"aliases":{}}))
    .unwrap();
    let mut fixture = ConsoleStateFixture::new(document, false);
    let directory = std::env::temp_dir().join(format!("codex-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, expected) in [("good", "passed"), ("bad", "failed"), ("partial", "failed")] {
        let response = probe_credential(&fixture.state, &grant, id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(body["result"]["credentialId"], id);
        assert_eq!(body["result"]["status"], expected);
        assert!(!body.to_string().contains("-secret"));
    }
    assert_eq!(calls.lock().unwrap().len(), 3);
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    let summary = db::request_audits::summarize_request_audit_rows(&audits);
    assert_eq!(summary.total_requests, 3);
    assert_eq!(summary.completed_count, 1);
    assert_eq!(summary.credentials.len(), 3);
    let good = audits
        .iter()
        .find(|audit| audit.status == "completed")
        .unwrap();
    assert_eq!(good.prompt_tokens, Some(8));
    assert_eq!(good.completion_tokens, Some(1));
    assert_eq!(good.endpoint_kind, "responses");
    assert!(good.stream);
    server.abort();
    local.close().await;
    drop(fixture);
    drop(local);
    // Windows SQLite worker handles can outlive pool.close() briefly.
    for attempt in 0..20 {
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => break,
            Err(error) if attempt == 19 => panic!("Cannot clean probe fixture: {error}"),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
}
