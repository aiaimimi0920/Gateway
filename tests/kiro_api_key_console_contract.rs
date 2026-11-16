//! Real management router, synthetic storage backend and ordinary SecretPatch/revision admission.
#![cfg(feature = "line-kiro-official-vendor-api")]
#[path = "console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use neuro_gateway::{console::secrets::redact_route_document, routing::config::RouteConfigYaml};
use serde_json::{json, Value};
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const ENDPOINT: &str = "/v1/internal/gateway/console/route-config";
fn empty_pool() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"renamed", "preset":"kiro-api-key",
        "base_url":"https://runtime.us-east-1.kiro.dev", "explicit_credentials_only":true, "credentials":[]}]})).unwrap()
}
fn candidate(fixture: &ConsoleStateFixture, document: Value, patches: Value) -> Value {
    json!({"document":document,"secretPatches":patches,
        "expectedRevision":fixture.state.route_config.snapshot().revision().id()})
}

#[tokio::test]
async fn kiro_key_normal_management_add_keep_clear_obeys_grant_origin_revision_and_exact_models() {
    let active = empty_pool();
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let mut document = serde_json::to_value(active).unwrap();
    document["providers"][0]["credentials"] = json!([{"id":"stable-a", "api_key":null,
        "account_name":"Preserved", "supported_models":["Exact.Model/Version"]}]);
    let path = "/providers/0/credentials/0/api_key";
    let body = candidate(
        &fixture,
        document,
        json!([{"path":path,"operation":"replace","value":"synthetic-managed-key"}]),
    );
    let missing = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    let wrong = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body.clone(),
        Some(&grant),
        Some("https://wrong.invalid"),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
    let success = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(success.status(), StatusCode::OK);
    let saved = parse_json(success).await;
    assert!(!saved.to_string().contains("synthetic-"));
    let providers = fixture.state.route_config.get_providers();
    let account = &providers[0].credential_pool[0];
    assert_eq!(account.payload.api_key, "synthetic-managed-key");
    assert_eq!(account.payload.auth_mode.as_deref(), Some("kiro_api_key"));
    assert_eq!(account.payload.adapter, "kiro_compatible");
    let stale = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        body,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let mut document = saved["routeConfig"]["document"].clone();
    document["providers"][0]["credentials"][0]["enabled"] = json!(false);
    document["providers"][0]["credentials"][0]["account_name"] = json!("Metadata only");
    document["providers"][0]["credentials"][0]["supported_models"] = json!(["Another.Exact/Model"]);
    let metadata = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        candidate(
            &fixture,
            document,
            json!([{"path":path,"operation":"keep"}]),
        ),
        None,
        None,
    )
    .await;
    assert_eq!(metadata.status(), StatusCode::OK);
    let metadata = parse_json(metadata).await;
    let document = &metadata["routeConfig"]["document"];
    assert_eq!(document["providers"][0]["credentials"][0]["id"], "stable-a");
    assert_eq!(
        document["providers"][0]["credentials"][0]["supported_models"],
        json!(["Another.Exact/Model"])
    );
    assert_eq!(
        fixture.state.route_config.get_providers()[0].credential_pool[0]
            .payload
            .api_key,
        "synthetic-managed-key"
    );
    let clear = candidate(
        &fixture,
        document.clone(),
        json!([{"path":path,"operation":"clear"}]),
    );
    let missing = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        clear.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let cleared = send_console_json(
        &fixture.state,
        Method::PUT,
        ENDPOINT,
        clear,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert!(
        fixture.state.route_config.get_providers()[0].credential_pool[0]
            .payload
            .api_key
            .is_empty()
    );
    assert!(!parse_json(cleared).await.to_string().contains("synthetic-"));
}

#[tokio::test]
async fn kiro_key_normal_management_rejects_erased_product_keep_before_compile_and_activation() {
    let mut active = empty_pool();
    active.providers[0].credentials =
        serde_json::from_value(json!([{"id":"stable-a","api_key":"synthetic-original"}])).unwrap();
    let fixture = ConsoleStateFixture::new(active.clone(), true);
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let mut document =
        serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
    document["providers"][0]["preset"] = json!("openai");
    document["providers"][0]["base_url"] = json!("https://untrusted.invalid/v1");
    let body = candidate(
        &fixture,
        document,
        json!([{"path":"/providers/0/credentials/0/api_key","operation":"keep"}]),
    );
    for (method, endpoint) in [
        (Method::POST, format!("{ENDPOINT}/validate")),
        (Method::PUT, ENDPOINT.into()),
    ] {
        let response =
            send_console_json(&fixture.state, method, &endpoint, body.clone(), None, None).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error = parse_json(response).await;
        assert_eq!(error["error"]["code"], "secret_keep_destination_changed");
        assert!(!error.to_string().contains("synthetic-"));
        assert_eq!(
            fixture.state.route_config.snapshot().revision().id(),
            original
        );
    }
}
