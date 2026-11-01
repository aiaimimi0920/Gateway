//! Explicit Web tests retain the exact account, HTTP stages and secret boundary.
use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{db, http::router::build_router, local_runtime::LocalRuntime};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
type Calls = Arc<Mutex<Vec<(String, String)>>>;

async fn homepage(State(calls): State<Calls>, headers: HeaderMap) -> (StatusCode, String) {
    let key = headers["authorization"].to_str().unwrap().to_string();
    calls.lock().unwrap().push(("/".into(), key.clone()));
    if key == "Bearer challenged-secret" {
        return (
            StatusCode::FORBIDDEN,
            "<html>Just a moment... verify you are human challenged-secret</html>".into(),
        );
    }
    (StatusCode::OK, "<html></html>".into())
}

async fn upstream(
    State(calls): State<Calls>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, [(String, String); 1], String) {
    let key = headers["authorization"].to_str().unwrap().to_string();
    calls.lock().unwrap().push((uri.path().into(), key.clone()));
    let reply = |status, media: &str, text| (status, [("content-type".into(), media.into())], text);
    if uri.path().ends_with("/prepare") {
        if key == "Bearer expired-secret" {
            return reply(
                StatusCode::UNAUTHORIZED,
                "application/json",
                "{\"detail\":\"expired-secret\"}".into(),
            );
        }
        return reply(
            StatusCode::OK,
            "application/json",
            json!({"prepare_token":"prepared-secret"}).to_string(),
        );
    }
    if uri.path().ends_with("/finalize") {
        assert_eq!(body["prepare_token"], "prepared-secret");
        return reply(
            StatusCode::OK,
            "application/json",
            json!({"token":"requirements-secret"}).to_string(),
        );
    }
    assert_eq!(
        body["model"], "gpt-5.6-sol",
        "do not replace the requested model"
    );
    assert!(body.to_string().contains("Reply with only OK."));
    assert_eq!(
        headers["openai-sentinel-chat-requirements-token"],
        "requirements-secret"
    );
    let text = if key == "Bearer empty-secret" {
        " "
    } else {
        "OK good-secret secondary-secret cookie-secret"
    };
    let event = json!({"message":{"author":{"role":"assistant"},"content":{"parts":[text]}}});
    let end = match key.as_str() {
        "Bearer truncated-secret" => "",
        "Bearer failed-secret" => "data: failed\n\ndata: [DONE]\n\n",
        "Bearer partial-secret" => "data: {\"p\":\"/message/status\",\"o\":\"replace\",\"v\":\"finished_partial_completion\"}\n\ndata: [DONE]\n\n",
        "Bearer blocked-secret" => "data: {\"type\":\"moderation\",\"moderation_response\":{\"blocked\":true}}\n\ndata: [DONE]\n\n",
        _ => "data: [DONE]\n\n",
    };
    reply(
        StatusCode::OK,
        "text/event-stream",
        format!("data: {event}\n\n{end}"),
    )
}

#[tokio::test]
async fn chatgpt_web_prompt_probe_uses_production_http_without_account_fallback() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/", get(homepage))
        .route(
            "/backend-api/sentinel/chat-requirements/prepare",
            post(upstream),
        )
        .route(
            "/backend-api/sentinel/chat-requirements/finalize",
            post(upstream),
        )
        .route("/backend-api/conversation", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials = ["good", "challenged", "expired", "truncated", "empty", "failed", "partial", "blocked", "disabled"].map(|id| json!({
        "id":id,"api_key":format!("{id}-secret"),"auth_token":"secondary-secret","enabled":id!="disabled",
        "headers":{"Cookie":"SID=cookie-secret"},"supported_models":["gpt-5.6-sol"],
        "session_auth":{"transport":"bearer","expiresAt":"2099-01-01T00:00:00Z"}
    }));
    let document = serde_json::from_value(json!({"providers":[{
        "id":"web-custom-id","preset":"chatgpt-web-reverse","base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"chatgpt_web"},
        "supported_models":["gpt-5.6-sol"],"credentials":credentials
    }],"model_routes":[],"aliases":{}})).unwrap();
    let mut fixture = ConsoleStateFixture::new(document, false);
    let directory =
        std::env::temp_dir().join(format!("chatgpt-web-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, model, expected) in [
        ("good", "gpt-5.6-sol", "passed"),
        ("challenged", "gpt-5.6-sol", "failed"),
        ("expired", "gpt-5.6-sol", "failed"),
        ("truncated", "gpt-5.6-sol", "failed"),
        ("empty", "gpt-5.6-sol", "failed"),
        ("failed", "gpt-5.6-sol", "failed"),
        ("partial", "gpt-5.6-sol", "failed"),
        ("blocked", "gpt-5.6-sol", "failed"),
        ("disabled", "gpt-5.6-sol", "unsupported"),
        ("good", "unknown-model", "unsupported"),
    ] {
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post("/v1/internal/gateway/console/providers/web-custom-id/probe")
                    .header("x-management-token", MANAGEMENT_TOKEN)
                    .header("x-secret-grant", &grant)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"credentialIds":[id],"model":model,"prompt":"Reply with only OK."})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["credentialId"], id);
        assert_eq!(result["status"], expected, "{body}");
        assert!(!body.to_string().contains("-secret"), "{body}");
        if id == "challenged" {
            assert!(result["message"]
                .as_str()
                .unwrap()
                .contains("stage=bootstrap"));
            assert!(result["message"].as_str().unwrap().contains("HTTP 403"));
        }
        if id == "expired" {
            assert!(result["message"]
                .as_str()
                .unwrap()
                .contains("stage=requirements_prepare"));
        }
    }
    for (grant_header, expected) in [
        (Some(grant.as_str()), StatusCode::OK),
        (None, StatusCode::FORBIDDEN),
    ] {
        let mut request = Request::post("/v1/internal/gateway/console/credentials/good/probe")
            .header("x-management-token", MANAGEMENT_TOKEN);
        if let Some(grant) = grant_header {
            request = request.header("x-secret-grant", grant);
        }
        let response = build_router(fixture.state.clone())
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == StatusCode::OK {
            assert_eq!(
                parse_json(response).await["result"]["status"],
                "unsupported",
                "an ordinary connectivity check must not start generation"
            );
        }
    }
    let seen = calls.lock().unwrap().clone();
    for (id, expected) in [
        ("good", 4),
        ("challenged", 1),
        ("expired", 2),
        ("truncated", 4),
        ("empty", 4),
        ("failed", 4),
        ("partial", 4),
        ("blocked", 4),
    ] {
        assert_eq!(
            seen.iter()
                .filter(|(_, key)| key == &format!("Bearer {id}-secret"))
                .count(),
            expected,
            "no hidden retry, browser or alternate account"
        );
    }
    assert_eq!(seen.len(), 27);
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 8);
    assert_eq!(
        audits
            .iter()
            .filter(|audit| audit.status == "completed")
            .count(),
        1
    );
    assert!(!serde_json::to_string(&audits).unwrap().contains("-secret"));
    server.abort();
    let _ = server.await;
    local.close().await;
}
