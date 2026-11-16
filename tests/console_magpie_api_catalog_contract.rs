//! New API lines use existing management grants, account attribution and local persistence.
#![cfg(all(
    feature = "line-river-official-api",
    feature = "line-cloudflare-workers-ai"
))]
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{db, local_runtime::LocalRuntime, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use support::*;

#[tokio::test]
async fn river_and_cloudflare_catalogs_require_grants_and_reject_mixed_or_foreign_targets_before_network(
) {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"river","preset":"river-api","base_url":"https://api.river.ai/v1","credentials":[{"id":"river-one","api_key":"rv_synthetic"}]},
        {"id":"cf","preset":"cloudflare-workers-ai","base_url":format!("https://api.cloudflare.com/client/v4/accounts/{}/ai/v1", "a".repeat(32)),"credentials":[{"id":"cf-one","api_key":"cf-synthetic"}]},
        {"id":"other","base_url":"https://example.com","credentials":[{"id":"other-one","api_key":"synthetic"}]}]})).unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let grant = grant_console_secret_access(&fixture.state).await;
    for (kind, id, credential) in [
        ("river", "river", "river-one"),
        ("cloudflare-workers-ai", "cf", "cf-one"),
    ] {
        let endpoint = format!("/v1/internal/gateway/console/{kind}-model-catalog");
        let denied = send_console_json(
            &fixture.state,
            Method::POST,
            &endpoint,
            json!({"credentialId":credential}),
            None,
            None,
        )
        .await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        for body in [
            json!({}),
            json!({"credentialId":"other-one"}),
            json!({"credentialId":credential,"apiKey":"override"}),
            json!({"providerId":"other","apiKey":"synthetic"}),
            json!({"providerId":id,"apiKey":"bad\r\ntoken"}),
        ] {
            let denied = send_console_json(
                &fixture.state,
                Method::POST,
                &endpoint,
                body,
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
        }
        let denied = send_console_json(
            &fixture.state,
            Method::POST,
            &endpoint,
            json!({"credentialId":credential,"baseUrl":"https://evil.invalid"}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}

async fn completion(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    assert_eq!(body["model"], "org/river-contract");
    assert_eq!(body["stream"], false);
    let authorization = headers["authorization"].to_str().unwrap().to_owned();
    calls.lock().unwrap().push(authorization.clone());
    if authorization == "Bearer good-secret" {
        (StatusCode::OK, json!({"model":"org/river-contract","choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}).to_string())
    } else {
        (
            StatusCode::UNAUTHORIZED,
            "bad-secret must not reach audit".into(),
        )
    }
}

#[tokio::test]
async fn river_explicit_model_probe_uses_exact_saved_key_and_persists_attributed_results() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(completion))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let document = serde_json::from_value(json!({"providers":[{"id":"river","preset":"river-api","base_url":format!("http://{address}/v1"),
        "supported_models":["org/river-contract"],
        "test_policy":{"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"selected","models":["org/river-contract"],
            "cases":[{"id":"ok","name":"OK","prompt":"hello","expectedAnswer":"OK","difficulty":2,"enabled":true}]},
        "credentials":[{"id":"good","api_key":"good-secret"},{"id":"bad","api_key":"bad-secret"}]}]})).unwrap();
    let mut fixture = ConsoleStateFixture::new(document, false);
    // The validation runner points TEMP/TMP at linshi; never use a live .ng profile.
    let directory = std::env::temp_dir().join(format!("river-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, expected) in [("good", "passed"), ("bad", "failed")] {
        let response = probe_credential(&fixture.state, &grant, id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(body["result"]["credentialId"], id);
        assert_eq!(body["result"]["status"], expected, "{body}");
        assert!(!body.to_string().contains("-secret"));
    }
    assert_eq!(
        *calls.lock().unwrap(),
        ["Bearer good-secret", "Bearer bad-secret"]
    );
    let rows = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    let summary = db::request_audits::summarize_request_audit_rows(&rows);
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.completed_count, 1);
    assert_eq!(summary.credentials.len(), 2);
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states.iter().any(
        |entry| entry.provider_credential_ref.as_deref() == Some("good")
            && entry.model == "org/river-contract"
            && entry.status == "active"
    ));
    assert!(!states.iter().any(
        |entry| entry.provider_credential_ref.as_deref() == Some("bad") && entry.status == "active"
    ));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    drop(fixture);
    local.close().await;
    drop(local);
    tokio::fs::remove_dir_all(directory).await.unwrap();
}
