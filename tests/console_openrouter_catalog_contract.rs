//! Official metadata endpoint rejects unauthorized or cross-origin targets before networking.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use support::{grant_console_secret_access, send_console_json, ConsoleStateFixture};

#[tokio::test]
async fn openrouter_catalog_requires_grant_and_rejects_mismatched_targets() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[
        {"id":"or","preset":"openrouter-openai","base_url":"https://openrouter.ai/api/v1",
         "credentials":[{"id":"account","api_key":"fixture-secret"},
            {"id":"override","api_key":"fixture-secret","base_url":"https://evil.invalid"}]},
        {"id":"nv","preset":"nvidia-openai","base_url":"https://integrate.api.nvidia.com",
         "credentials":[{"id":"nv-account","api_key":"fixture-secret"}]},
        {"id":"evil","preset":"openrouter-openai","base_url":"https://evil.invalid","credentials":[]}],
        "model_routes":[]})).unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let endpoint = "/v1/internal/gateway/console/openrouter-model-catalog";
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
        json!({"credentialId":"nv-account"}),
        json!({"credentialId":"override"}),
        json!({"credentialId":"account","apiKey":"override"}),
        json!({"providerId":"evil","apiKey":"fixture"}),
        json!({"providerId":"or","apiKey":"bad\r\nkey"}),
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
    let invalid = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"providerId":"or","apiKey":"fixture","baseUrl":"https://evil.invalid"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
