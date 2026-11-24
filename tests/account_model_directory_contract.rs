//! Custom directory reads preserve grants/credential origin and never run generation.
use axum::{
    extract::Request,
    http::{Method, StatusCode},
    routing::any,
    Json, Router,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

#[tokio::test]
async fn directory_only_fetch_uses_saved_key_without_generation_or_configuration_writes() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let captured = calls.clone();
    let upstream = Router::new().fallback(any(move |request: Request| {
        let captured = captured.clone();
        async move {
            captured
                .lock()
                .unwrap()
                .push((request.method().clone(), request.uri().path().to_owned()));
            if request.uri().path() == "/v1/models" {
                assert_eq!(request.headers()["authorization"], "Bearer selected-key");
                (
                    StatusCode::OK,
                    Json(json!({"data":[{"id":"catalog-text"},{"id":"catalog-image"}]})),
                )
            } else {
                // A denied optional alias must not discard the standard directory.
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({"cloudflare_error":true,"error_code":1010})),
                )
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let fixture = ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{
            "id":"custom","adapter":"openai_compatible","base_url":origin,
            "credentials":[{"id":"account","api_key":"selected-key","supported_models":["manual"]}]
        }]}))
        .unwrap(),
        true,
    );
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let endpoint = "/v1/internal/gateway/console/account-discovery";
    let body = json!({"credentialId":"account","catalogOnly":true});
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
    assert!(calls.lock().unwrap().is_empty());
    let grant = grant_console_secret_access(&fixture.state).await;
    let override_reply = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        json!({"credentialId":"account","catalogOnly":true,"baseUrl":"http://127.0.0.1:1"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(override_reply.status(), StatusCode::BAD_REQUEST);
    assert!(calls.lock().unwrap().is_empty());
    let reply = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(reply.status(), StatusCode::OK);
    assert_eq!(reply.headers()["cache-control"], "no-store");
    let value = parse_json(reply).await;
    assert_eq!(
        value["catalog"]["models"],
        json!(["catalog-text", "catalog-image"])
    );
    assert!(value.get("discovery").is_none());
    assert!(!value.to_string().contains("selected-key"));
    assert_eq!(value["revision"], revision);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .all(|(method, _)| method == Method::GET));
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn client_restriction_is_explicit_without_echoing_private_upstream_body() {
    let upstream = Router::new().fallback(any(|| async { (StatusCode::FORBIDDEN,
        "{\"error\":{\"message\":\"This account only allows Codex official clients\"}}event: response.failed\nprivate-key") }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap() });
    let error = neuro_gateway::provider_discovery::discover_with_models(
        &origin,
        "private-key",
        &["known".into()],
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("本次探测的客户端身份"));
    assert!(error.to_string().contains("client_restricted"));
    assert!(!error.to_string().contains("private-key"));
    let error = neuro_gateway::provider_discovery::read_account_models(&origin, "private-key")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("本次探测的客户端身份"));
    assert!(!error.to_string().contains("private-key"));
    server.abort();
    let _ = server.await;
}
