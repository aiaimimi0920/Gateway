//! Gemini Web prompt proofs use the exact session and production form dispatcher.
use axum::{
    body::Body,
    extract::{Form, State},
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
    Router,
};
use neuro_gateway::http::router::build_router;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;
type Calls = Arc<Mutex<Vec<(String, String)>>>;

async fn bootstrap() -> &'static str {
    r#"<html>{"SNlM0e":"access-secret","cfb2h":"build-test","FdrFJe":"session-secret"}</html>"#
}

async fn upstream(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, String) {
    let cookie = headers["cookie"].to_str().unwrap().to_string();
    assert!(!headers.contains_key("authorization"));
    assert_eq!(form.get("at").map(String::as_str), Some("access-secret"));
    assert!(form["f.req"].contains("Reply with only OK."));
    let id = cookie
        .split("__Secure-1PSID=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    calls.lock().unwrap().push((id.into(), cookie.clone()));
    if id == "bad-secret" {
        return (
            StatusCode::UNAUTHORIZED,
            "<html>Sign in bad-secret</html>".into(),
        );
    }
    if id == "malformed-secret" {
        return (StatusCode::OK, "{\"no_candidate\":true}".into());
    }
    let text = if id == "empty-secret" {
        " "
    } else {
        "OK good-secret access-secret session-secret"
    };
    let body = json!({"candidates":[{"content":{"parts":[{"text":text}]},
        "finishReason":if id == "partial-secret" { "MAX_TOKENS" } else { "STOP" }}]});
    (StatusCode::OK, body.to_string())
}

#[tokio::test]
async fn gemini_web_probe_keeps_session_identity_and_rejects_non_answers() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/app", get(bootstrap))
        .route(
            "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
            post(upstream),
        )
        .with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let credentials = ["good", "bad", "partial", "empty", "malformed", "disabled"].map(|id| json!({
        "id":id,"api_key":format!("{id}-secret"),"auth_token":"secondary-secret", "enabled":id!="disabled",
        "headers":{"Cookie":"__Secure-1PSID=stale; SID=supplemental-secret"},
        "extra_body":{"accessToken":"access-secret","sessionId":"session-secret"},
        "supported_models":["gemini-web-chat"]}));
    let document = serde_json::from_value(json!({"providers":[{"id":"gemini-web","preset":"gemini-web-chat", "base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"gemini_web"},
        "supported_models":["gemini-web-chat"],"credentials":credentials}],"model_routes":[],"aliases":{}})).unwrap();
    let fixture = ConsoleStateFixture::new(document, false);
    let grant = grant_console_secret_access(&fixture.state).await;
    for (id, model, expected, explicit) in [
        ("good", "gemini-web-chat", "passed", true),
        ("bad", "gemini-web-chat", "failed", true),
        ("partial", "gemini-web-chat", "failed", true),
        ("empty", "gemini-web-chat", "failed", true),
        ("malformed", "gemini-web-chat", "failed", true),
        ("disabled", "gemini-web-chat", "unsupported", true),
        ("good", "unknown-model", "unsupported", true),
        ("good", "gemini-web-chat", "passed", false),
    ] {
        let mut body = json!({"credentialIds":[id]});
        if explicit {
            body["prompt"] = json!("Reply with only OK.");
            body["model"] = json!(model);
        }
        let path = if explicit {
            "/v1/internal/gateway/console/providers/gemini-web/probe".to_string()
        } else {
            format!("/v1/internal/gateway/console/credentials/{id}/probe")
        };
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post(path)
                    .header("x-management-token", MANAGEMENT_TOKEN)
                    .header("x-secret-grant", &grant)
                    .header("content-type", "application/json")
                    .body(if explicit {
                        Body::from(body.to_string())
                    } else {
                        Body::empty()
                    })
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{id}: {body}");
        let result = if explicit {
            &body["result"]["results"][0]
        } else {
            &body["result"]
        };
        assert_eq!(result["credentialId"], id);
        assert_eq!(result["status"], expected, "{body}");
        assert!(!body.to_string().contains("-secret"), "{body}");
    }
    let calls = calls.lock().unwrap().clone();
    assert_eq!(
        calls.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        [
            "good-secret",
            "bad-secret",
            "partial-secret",
            "empty-secret",
            "malformed-secret",
            "good-secret"
        ],
        "no fallback or retry"
    );
    assert!(calls
        .iter()
        .all(|(_, cookie)| cookie.contains("__Secure-1PSIDTS=secondary-secret")));
    assert!(
        calls
            .iter()
            .all(|(_, cookie)| cookie.contains("SID=supplemental-secret")
                && !cookie.contains("stale"))
    );
    server.abort();
    let _ = server.await;
}
