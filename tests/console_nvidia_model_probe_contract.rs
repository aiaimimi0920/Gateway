//! A listed model or HTTP 200 alone must never prove a NVIDIA credential works.
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
#[path = "console_nvidia_model_probe_contract/cleanup.rs"]
mod cleanup;

async fn completion(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    assert_eq!(body["model"], "nvidia/test-model");
    assert_eq!(body["stream"], false);
    assert_eq!(body["max_tokens"], 256);
    let authorization = headers["authorization"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(authorization.clone());
    match authorization.as_str() {
        "Bearer good-secret" => (StatusCode::OK, json!({"choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":8,"completion_tokens":1,"total_tokens":9}}).to_string()),
        "Bearer bad-secret" => (StatusCode::UNAUTHORIZED, "do not expose bad-secret".into()),
        "Bearer empty-secret" => (StatusCode::OK, json!({"choices":[{"message":{"content":""}}]}).to_string()),
        "Bearer invalid-secret" => (StatusCode::OK, "<html>OK</html>".into()),
        "Bearer echo-secret" => (StatusCode::OK, json!({"choices":[{"message":{"content":"OK echo-secret"}}]}).to_string()),
        _ => panic!("unexpected credential"),
    }
}

#[tokio::test]
async fn nvidia_tests_generate_with_exact_credential_and_persist_model_attribution() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(completion))
        .with_state(calls.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let credentials = ["good", "bad", "empty", "invalid", "echo"].map(|id| {
        json!({"id": id, "api_key": format!("{id}-secret"),
            "supported_models": ["retired-model", "probe-alias"]})
    });
    let document = serde_json::from_value(json!({"providers":[{
        "id":"nvidia", "preset":"nvidia-openai", "base_url":format!("http://{address}"),
        "supported_models":["probe-alias"], "model_map":{"probe-alias":"nvidia/test-model"},
        "credentials":credentials
    }],"model_routes":[],"aliases":{}}))
    .unwrap();
    let mut fixture = ConsoleStateFixture::new(document, false);
    let directory =
        std::env::temp_dir().join(format!("nvidia-model-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (credential, expected, message) in [
        ("good", "passed", "Reply: OK"),
        ("bad", "failed", "HTTP 401"),
        ("empty", "failed", "no non-empty assistant reply"),
        ("invalid", "failed", "invalid JSON"),
        ("echo", "passed", "[redacted]"),
    ] {
        let response = probe_credential(&fixture.state, &grant, credential).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(body["result"]["credentialId"], credential);
        assert_eq!(body["result"]["status"], expected);
        assert!(body["result"]["message"]
            .as_str()
            .unwrap()
            .contains(message));
        assert!(body["result"]["probePoint"]
            .as_str()
            .unwrap()
            .contains("/v1/chat/completions"));
        assert!(!body.to_string().contains("-secret"));
    }
    assert_eq!(calls.lock().unwrap().len(), 5);
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    let summary = db::request_audits::summarize_request_audit_rows(&audits);
    assert_eq!(summary.total_requests, 5);
    assert_eq!(summary.completed_count, 2);
    assert_eq!(summary.credentials.len(), 5);
    assert!(summary
        .credentials
        .iter()
        .all(|entry| entry.stats.total_requests == 1));
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    for credential in ["good", "echo"] {
        assert!(states
            .iter()
            .any(
                |entry| entry.provider_credential_ref.as_deref() == Some(credential)
                    && entry.model == "nvidia/test-model"
                    && entry.status == "active"
            ));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    drop(fixture);
    local.close().await;
    drop(local);
    cleanup::remove_probe_directory(&directory).await.unwrap();
}
