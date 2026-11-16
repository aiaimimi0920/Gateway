//! Known model candidates bypass unavailable catalogues without bypassing secret access.
use axum::{
    http::{Method, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[tokio::test]
async fn known_models_work_through_management_without_catalogue_or_key_disclosure() {
    let upstream = Router::new().route(
        "/v1/chat/completions",
        post(|| async { Json(json!({"choices":[{"message":{"content":"OK"}}]})) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"pool", "adapter":"openai_compatible", "base_url":root,
        "supported_models":["provider-model"],
        "credentials":[{"id":"account", "api_key":"synthetic-private-key",
            "supported_models":["account-model"]}]}],"model_routes":[]}))
    .unwrap();
    let fixture = ConsoleStateFixture::new(document, true);
    let endpoint = "/v1/internal/gateway/console/account-discovery";
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account","models":["draft-model"]}),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_string();
    for (request, expected) in [
        (json!({"credentialId":"account"}), "account-model"),
        (
            json!({"credentialId":"account","models":["draft-model"]}),
            "draft-model",
        ),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            endpoint,
            request,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = parse_json(response).await;
        assert_eq!(body["discovery"]["verified_models"], json!([expected]));
        assert!(!body.to_string().contains("synthetic-private-key"));
    }
    for request in [
        json!({"credentialId":"account","models":[""]}),
        json!({"credentialId":"account","models":["draft-model"],"baseUrl":root}),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            endpoint,
            request,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    server.abort();
    let _ = server.await;
}
