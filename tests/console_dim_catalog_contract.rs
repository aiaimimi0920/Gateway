//! Directory reads require operator consent and accept no caller-selected token or destination.
#![cfg(feature = "line-dim-user-oauth")]
use axum::http::{Method, StatusCode};
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;
const PATH: &str = "/v1/internal/gateway/console/dim-model-catalog";
fn fixture() -> support::ConsoleStateFixture {
    support::ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
        "id":"dim","preset":"dim-cn-oauth","auth_mode":"dim_user_oauth",
        "base_url":"https://dimagent.cn/v1","explicit_credentials_only":true,"credentials":[]}]}))
        .unwrap(),
        true,
    )
}
#[tokio::test]
async fn dim_catalog_requires_secret_grant_without_starting_discovery() {
    let f = fixture();
    let response = support::send_console_json(&f.state, Method::POST, PATH,
        json!({"providerId":"dim","credentialId":"missing","expectedRevision":f.state.route_config.snapshot().revision().id()}), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
#[tokio::test]
async fn dim_catalog_rejects_unknown_token_and_destination_fields() {
    let f = fixture();
    let grant = support::grant_console_secret_access(&f.state).await;
    for field in ["apiKey", "baseUrl", "site", "clientId"] {
        let mut body = json!({"providerId":"dim","credentialId":"missing","expectedRevision":f.state.route_config.snapshot().revision().id()});
        body[field] = json!("caller-override");
        let response =
            support::send_console_json(&f.state, Method::POST, PATH, body, Some(&grant), None)
                .await;
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}
#[tokio::test]
async fn dim_catalog_rejects_stale_revision_and_nonexistent_explicit_account() {
    let f = fixture();
    let grant = support::grant_console_secret_access(&f.state).await;
    let response = support::send_console_json(
        &f.state,
        Method::POST,
        PATH,
        json!({"providerId":"dim","credentialId":"missing","expectedRevision":"stale"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response = support::send_console_json(&f.state, Method::POST, PATH,
        json!({"providerId":"dim","credentialId":"missing","expectedRevision":f.state.route_config.snapshot().revision().id()}), Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
