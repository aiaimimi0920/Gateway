//! Catalog reads require management + secret grant and never accept arbitrary destinations.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use support::{grant_console_secret_access, send_console_json, ConsoleStateFixture};

#[tokio::test]
async fn nvidia_catalog_rejects_missing_grant_and_invalid_targets_before_network() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"nvidia","preset":"nvidia-openai","base_url":"https://integrate.api.nvidia.com",
         "credentials":[{"id":"account","api_key":"fixture-secret"}]},
        {"id":"other","base_url":"https://example.com","credentials":[{"id":"other-account","api_key":"fixture"}]}],
        "model_routes":[]})).unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let endpoint = "/v1/internal/gateway/console/nvidia-model-catalog";
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account"}),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    for body in [
        json!({"credentialId":"other-account"}),
        json!({"credentialId":"account","apiKey":"override"}),
        json!({"providerId":"other","apiKey":"fixture"}),
        json!({"providerId":"nvidia","apiKey":"bad\r\nkey"}),
        json!({}),
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
    }
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account","baseUrl":"https://example.com"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
