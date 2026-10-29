//! Loopback management authorization and secret-safe local key reuse contracts.
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::{local_runtime::LocalRuntime, routing::config::RouteConfigYaml};
use serde_json::json;
use std::sync::Arc;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

#[tokio::test]
async fn key_reuse_requires_grant_reseals_and_preserves_group_restrictions() {
    let empty: RouteConfigYaml =
        serde_json::from_value(json!({"providers":[],"model_routes":[]})).unwrap();
    let mut fixture = ConsoleStateFixture::new(empty, true);
    let root = std::env::temp_dir().join(format!("gateway-key-import-{}", uuid::Uuid::new_v4()));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    let endpoint = "/v1/internal/gateway/access/keys/import-unlimited";
    let body = json!({"token":"sk-gw-fixture-existing-secret", "key":{
        "ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"codex", "metadata":{"accountGroupIds":["default"]}}});
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
    assert!(fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .access_catalog()
        .await
        .unwrap()
        .access_keys
        .is_empty());
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
    let key = parse_json(response).await;
    assert!(!key.to_string().contains("sk-gw-fixture-existing-secret"));
    assert_eq!(key["metadata"], body["key"]["metadata"]);
    let local = fixture.state.local_runtime.as_ref().unwrap();
    assert_eq!(
        local
            .access_key_secret(key["id"].as_str().unwrap())
            .await
            .unwrap(),
        body["token"].as_str().unwrap()
    );
    let duplicate = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let mut invalid = body;
    invalid["key"]["metadata"] = json!({"accountGroupIds":["missing"]});
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        endpoint,
        invalid,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn catalog_requires_grant_and_rejects_overrides_wrong_accounts_and_bad_material_before_network(
) {
    let active: RouteConfigYaml = serde_json::from_value(
        json!({"providers":[{"id":"plain", "base_url":"https://example.com",
        "credentials":[{"id":"account","api_key":"fixture-secret"}]}],"model_routes":[]}),
    )
    .unwrap();
    let fixture = ConsoleStateFixture::new(active, true);
    let endpoint = "/v1/internal/gateway/console/chatgpt-model-catalog";
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
        json!({"credentialId":"account"}),
        json!({"credentialId":"account","accessToken":"override"}),
        json!({"accessToken":"fixture\r\ninjection","accountId":"account"}),
        json!({"accessToken":"fixture"}),
        json!({"credentialId":"account","accountId":"override"}),
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
}
