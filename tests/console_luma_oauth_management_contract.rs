//! Authorization management must reject missing consent before opening any callback listener.
use axum::http::{Method, StatusCode};
use serde_json::json;
#[path = "fixtures/luma_mcp.rs"]
mod account;
#[path = "console_contract_support/mod.rs"]
mod support;
const PATH: &str = "/v1/internal/gateway/console/luma-auth-sessions";

#[tokio::test]
async fn luma_login_requires_secret_consent_and_no_store() {
    let f = support::ConsoleStateFixture::new(account::document(account::provider()), true);
    let body = json!({"providerId":"luma-mcp","expectedRevision":f.state.route_config.snapshot().revision().id(),
        "clientId":"synthetic-client","redirectUri":"http://127.0.0.1:54421/callback"});
    let response = support::send_console_json(&f.state, Method::POST, PATH, body, None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
}

#[tokio::test]
async fn luma_login_rejects_stale_revision_before_listener_creation() {
    let f = support::ConsoleStateFixture::new(account::document(account::provider()), true);
    let grant = support::grant_console_secret_access(&f.state).await;
    let body = json!({"providerId":"luma-mcp","expectedRevision":"stale",
        "clientId":"synthetic-client","redirectUri":"http://127.0.0.1:54421/callback"});
    let response =
        support::send_console_json(&f.state, Method::POST, PATH, body, Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
