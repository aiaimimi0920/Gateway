//! Search acceptance requires real results from the exact credential, not chat text or HTTP 200.
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
async fn search(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    assert_eq!(body, json!({"query":"Rust documentation", "count":3}));
    assert!(headers.get("authorization").is_none());
    let key = headers["x-api-key"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(key.clone());
    match key.as_str() {
        "good-secret" => (StatusCode::OK, json!({"results":{"web":[{"url":"https://www.rust-lang.org/", "title":"Rust good-secret"}]}}).to_string()),
        "bad-secret" => (StatusCode::UNAUTHORIZED, "do not expose bad-secret".into()),
        "empty-secret" => (StatusCode::OK, json!({"results":{"web":[]}}).to_string()),
        "invalid-secret" => (StatusCode::OK, "not JSON invalid-secret".into()),
        "huge-secret" => (StatusCode::OK, "x".repeat(1_048_577)),
        "error-secret" => (StatusCode::OK, json!({"error":"error-secret", "results":{"web":[{"url":"https://example.com", "title":"Example"}]}}).to_string()),
        "malformed-secret" => (StatusCode::OK, json!({"results":{"web":[{"url":"javascript:alert(1)", "title":"Invalid"}]}}).to_string()),
        _ => panic!("unexpected credential"),
    }
}

async fn send(state: &Arc<neuro_gateway::state::AppState>, grant: &str, body: Value) -> Value {
    let response = build_router(state.clone())
        .oneshot(
            Request::post("/v1/internal/gateway/console/providers/you/probe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    parse_json(response).await
}

#[tokio::test]
async fn you_search_probes_use_search_wire_format_bounds_and_accurate_ungraded_audits() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/search", post(search))
        .with_state(calls.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let ids = [
        "good",
        "bad",
        "empty",
        "invalid",
        "huge",
        "error",
        "malformed",
        "disabled",
    ];
    let credentials = ids.map(|id| json!({"id":id, "api_key":format!("{id}-secret"), "enabled":id != "disabled", "supported_models":["you-search"]}));
    let mut fixture = ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
        "id":"you", "preset":"you", "base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto", "fingerprint_adapter":"auto", "special_adapter":"auto"},
        "supported_models":["you-search"], "credentials":credentials
    }], "model_routes":[], "aliases":{}}))
        .unwrap(),
        false,
    );
    // Local runners point TEMP/TMP at linshi; CI keeps its own disposable temp root.
    let directory =
        std::env::temp_dir().join(format!("you-search-contract-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, status, message) in [
        ("good", "passed", "Search returned 1 web result"),
        ("bad", "failed", "HTTP 401"),
        ("empty", "failed", "no usable web results"),
        ("invalid", "failed", "invalid JSON"),
        ("huge", "failed", "exceeded 1 MiB"),
        ("error", "failed", "no usable web results"),
        ("malformed", "failed", "no usable web results"),
        ("disabled", "unsupported", "disabled"),
    ] {
        let body = send(
            &fixture.state,
            &grant,
            json!({"prompt":"Rust documentation", "model":"you-search", "credentialIds":[id]}),
        )
        .await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["status"], status, "{body}");
        assert!(
            result["message"].as_str().unwrap().contains(message),
            "{body}"
        );
        assert!(!body.to_string().contains("-secret"));
    }
    let unsupported = send(
        &fixture.state,
        &grant,
        json!({"prompt":"Rust documentation", "model":"other", "credentialIds":["good"]}),
    )
    .await;
    assert_eq!(unsupported["result"]["unsupportedCount"], 1);
    let plan = json!({"automaticEnabled":false,"intervalMinutes":60,"modelSelection":"all","models":[],"cases":[{
        "id":"search", "name":"Search", "prompt":"Rust documentation", "expectedAnswer":null, "difficulty":1, "enabled":true
    }]});
    let body = send(
        &fixture.state,
        &grant,
        json!({"testPlan":plan, "credentialIds":["good"]}),
    )
    .await;
    let assessment = &body["result"]["results"][0]["assessment"];
    assert_eq!(assessment["models"][0]["callable"], true, "{body}");
    assert_eq!(assessment["models"][0]["score"], Value::Null);
    assert_eq!(assessment["cases"][0]["correct"], Value::Null);
    assert_eq!(calls.lock().unwrap().len(), 8);
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 8);
    assert!(audits
        .iter()
        .all(|a| a.protocol_family == "search" && a.endpoint_kind == "search"));
    for id in ids.into_iter().filter(|id| *id != "disabled") {
        assert_eq!(
            audits
                .iter()
                .filter(|a| a
                    .route_trace
                    .as_ref()
                    .is_some_and(|trace| trace["realCredentialRef"] == id))
                .count(),
            if id == "good" { 2 } else { 1 }
        );
    }
    let summary = db::request_audits::summarize_request_audit_rows(&audits);
    assert_eq!(summary.completed_count, 2);
    assert_eq!(summary.credentials.len(), 7);
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states
        .iter()
        .any(|s| s.provider_credential_ref.as_deref() == Some("good")
            && s.model == "you-search"
            && s.status == "active"));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    drop(fixture);
    local.close().await;
    drop(local);
    assert!(directory.starts_with(std::env::temp_dir()));
    // Windows can retain the final SQLite handle briefly after the pool closes.
    for attempt in 0..40 {
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => break,
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(32 | 33))
                    && attempt < 39 =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(error) => panic!("cannot clean isolated search fixture: {error}"),
        }
    }
}
