//! A reveal is secret-grant protected, scoped to one account and revision fenced.
use axum::http::{Method, StatusCode};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[tokio::test]
async fn reveal_requires_grant_and_exact_account_revision() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"pool","adapter":"openai_compatible","base_url":"https://fixture.test",
        "api_key":"inherited-secret", "credentials":[{"id":"account","api_key":"fixture-secret"},
        {"id":"inherited"},{"id":"other","api_key":"other-secret"}]}],"model_routes":[]}))
    .unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let endpoint = "/v1/internal/gateway/console/credential-api-key";
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let body = json!({"providerId":"pool","credentialId":"account","expectedRevision":revision});
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        body.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        parse_json(response).await,
        json!({"apiKey":"fixture-secret"})
    );
    for (field, value, expected) in [
        ("providerId", "missing", StatusCode::NOT_FOUND),
        ("credentialId", "missing", StatusCode::NOT_FOUND),
        ("expectedRevision", "stale", StatusCode::CONFLICT),
    ] {
        let mut invalid = body.clone();
        invalid[field] = json!(value);
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            endpoint,
            invalid,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), expected);
        assert!(!parse_json(response).await.to_string().contains("secret"));
    }
    let mut inherited = body;
    inherited["credentialId"] = json!("inherited");
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        inherited,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(
        parse_json(response).await,
        json!({"apiKey":"inherited-secret"})
    );
}
