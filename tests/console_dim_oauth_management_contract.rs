//! Management authorization and extractor failures are independent of the OAuth callback.
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;

const ROOT: &str = "/v1/internal/gateway/console/dim-auth-sessions";
fn document() -> RouteConfigYaml {
    serde_json::from_value(
        json!({"providers":[{"id":"dim","auth_mode":"dim_user_oauth",
        "base_url":"https://dimagent.cn/v1","explicit_credentials_only":true,"credentials":[]}]}),
    )
    .unwrap()
}
#[tokio::test]
async fn dim_oauth_management_requires_secret_grant_and_never_caches_rejection() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let response = support::send_console_json(&fixture.state, Method::POST, ROOT,
        json!({"providerId":"dim","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
            "site":"china","clientId":"permitted-gateway","scopes":["openid"],
            "redirectUri":"http://127.0.0.1:54322/oauth/callback"}), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(response.headers()["cache-control"], "no-store");
}
#[tokio::test]
async fn dim_oauth_management_rejects_unregistered_redirect_before_discovery() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let response = support::send_console_json(&fixture.state, Method::POST, ROOT,
        json!({"providerId":"dim","expectedRevision":fixture.state.route_config.snapshot().revision().id(),
            "site":"china","clientId":"permitted-gateway","scopes":["openid"],
            "redirectUri":"http://localhost:54322/oauth/callback"}), Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["cache-control"], "no-store");
}

#[test]
fn dim_oauth_management_cannot_hide_a_global_account_in_a_china_product_pool() {
    let mut raw = serde_json::to_value(document()).unwrap();
    raw["providers"][0]["credentials"] = json!([{"id":"global-account","base_url":"https://dimagent.com/v1",
        "api_key":"synthetic-access","refresh_token":"synthetic-refresh","dim_oauth":{
            "site":"global","client_id":"permitted-gateway","requested_scopes":["openid"],
            "token_endpoint":"https://dimagent.com/oauth/token","userinfo_endpoint":"https://dimagent.com/oauth/userinfo",
            "jwks_uri":"https://dimagent.com/oauth/jwks.json","subject":"synthetic-subject","user_id":"synthetic-user",
            "assurance":"user_info_endpoint","nonce":"synthetic-nonce"}}]);
    assert!(
        neuro_gateway::routing::config::RouteConfigStore::from_document(
            serde_json::from_value(raw).unwrap()
        )
        .is_err()
    );
}
