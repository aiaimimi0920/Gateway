//! Web catalog authorization/origin guards run without contacting the real service.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

#[tokio::test]
async fn web_catalog_requires_grant_and_rejects_non_web_origins_and_secret_overrides() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"web", "preset":"chatgpt-web-reverse", "base_url":"https://example.com",
        "credentials":[{"id":"web-account","api_key":"fixture-private-token"}]}], "model_routes":[]})).unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let endpoint = "/v1/internal/gateway/console/chatgpt-model-catalog";
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"web-account"}),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    for body in [
        json!({"credentialId":"web-account"}),
        json!({"credentialId":"web-account","accessToken":"override"}),
        json!({"credentialId":"web-account","accountId":"override"}),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            endpoint,
            body,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!parse_json(response)
            .await
            .to_string()
            .contains("fixture-private-token"));
    }
}
