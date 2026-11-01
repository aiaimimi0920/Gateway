//! Management/session boundary only: no browser launch or OAuth token request.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/antigravity-auth-sessions";

// This integration-test binary has one test; restore the environment on every exit.
struct ClientEnvironment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl ClientEnvironment {
    fn synthetic() -> Self {
        let mut saved = Vec::new();
        for (key, value) in [
            ("GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_ID", "synthetic-client"),
            (
                "GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_SECRET",
                "synthetic-secret",
            ),
        ] {
            saved.push((key, std::env::var_os(key)));
            std::env::set_var(key, value);
        }
        Self(saved)
    }
}
impl Drop for ClientEnvironment {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

fn request(path: &str, body: Value, grant: Option<&str>) -> Request<Body> {
    let mut request = Request::post(path)
        .header("x-management-token", support::MANAGEMENT_TOKEN)
        .header("content-type", "application/json");
    if let Some(grant) = grant {
        request = request.header("x-secret-grant", grant);
    }
    request.body(Body::from(body.to_string())).unwrap()
}
#[tokio::test]
async fn antigravity_oauth_requires_secret_access_binds_owner_and_cancels_cleanly() {
    let _environment = ClientEnvironment::synthetic();
    let document=serde_json::from_value(json!({"providers":[{"id":"pool","preset":"antigravity-oauth",
        "base_url":"https://daily-cloudcode-pa.googleapis.com/v1internal","explicit_credentials_only":true,"credentials":[]}]})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    let app = build_router(fixture.state.clone());
    let response = app
        .clone()
        .oneshot(request(ROOT, json!({"providerId":"pool"}), None))
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = app
        .clone()
        .oneshot(request(ROOT, json!({"providerId":"pool"}), Some(&grant)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = support::parse_json(response).await;
    let session = &body["session"];
    assert_eq!(session["status"], "waiting_user");
    assert!(session["authorizationUrl"]
        .as_str()
        .unwrap()
        .starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(session.get("accessToken").is_none());
    assert!(session.get("refreshToken").is_none());
    assert!(!session.to_string().contains("synthetic-secret"));
    let id = session["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("{ROOT}/{id}"))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .header("host", "different.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(request(
            &format!("{ROOT}/{id}/complete"),
            json!({"callbackUrl":"https://evil.invalid/?code=private&state=private"}),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(request(&format!("{ROOT}/{id}/cancel"), json!({}), None))
        .await
        .unwrap();
    assert_eq!(
        support::parse_json(response).await["session"]["status"],
        "cancelled"
    );
    let response = app
        .clone()
        .oneshot(request(
            &format!("{ROOT}/{id}/import"),
            json!({}),
            Some(&grant),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    std::env::set_var("GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_SECRET", "");
    let response = app
        .oneshot(request(ROOT, json!({"providerId":"pool"}), Some(&grant)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = support::parse_json(response).await;
    assert!(body
        .to_string()
        .contains("antigravity_oauth_client_invalid"));
    assert!(!body.to_string().contains("synthetic-secret"));
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
}
