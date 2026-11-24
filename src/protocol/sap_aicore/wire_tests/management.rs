//! Actual management router protects the complete service key and its resource-group audience.
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
const CONFIG: &str = "/v1/internal/gateway/console/route-config";
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(
        serde_json::from_value(super::super::tests::document()).unwrap(),
        true,
    )
}
fn draft(f: &ConsoleStateFixture) -> Value {
    let snapshot = f.state.route_config.snapshot();
    let redacted = crate::console::secrets::redact_route_document(snapshot.document()).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| json!({"path":s.path,"operation":"keep"}))
        .collect();
    json!({"expectedRevision":snapshot.revision().id(),"document":redacted.document,"secretPatches":patches})
}
fn run<F: std::future::Future<Output = ()> + 'static>(f: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(f())
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn sap_aicore_management_grant_replace_keep_and_cas() {
    run(save);
}
async fn save() {
    let fixture = fixture();
    let grant = grant_console_secret_access(&fixture.state).await;
    let replacement = KEY.replace("synthetic-sap-secret", "replacement-sap-secret");
    let mut body = draft(&fixture);
    body["secretPatches"] = json!([{"path":"/providers/0/credentials/0/api_key","operation":"replace","value":replacement}]);
    assert_eq!(
        send_console_json(
            &fixture.state,
            Method::PUT,
            CONFIG,
            body.clone(),
            None,
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        CONFIG,
        body.clone(),
        Some(&grant),
        None,
    )
    .await;
    let status = response.status();
    let result = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(!result.to_string().contains("replacement-sap-secret"));
    assert_eq!(
        send_console_json(
            &fixture.state,
            Method::PUT,
            CONFIG,
            body,
            Some(&grant),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let mut body = draft(&fixture);
    body["document"]["providers"][0]["credentials"][0]["account_name"] = json!("Renamed");
    let response = send_console_json(&fixture.state, Method::PUT, CONFIG, body, None, None).await;
    let status = response.status();
    let result = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let snapshot = fixture.state.route_config.snapshot();
    let account = &snapshot.document().providers[0].credentials[0];
    assert_eq!(account.id.as_deref(), Some("stable"));
    assert_eq!(account.api_key.as_deref(), Some(replacement.as_str()));
    assert!(account.refresh_token.is_none());
}
#[test]
fn sap_aicore_keep_rejects_resource_group_deployment_and_product_rebinding() {
    run(binding);
}
async fn binding() {
    let fixture = fixture();
    let original = draft(&fixture);
    for (path, value) in [
        (
            "/document/providers/0/headers/AI-Resource-Group",
            json!("group-b"),
        ),
        (
            "/document/providers/0/base_url",
            json!(super::super::test_support::BASE.replace("deployment1", "deployment2")),
        ),
        (
            "/document/providers/0/base_url",
            json!(super::super::test_support::azure_base()),
        ),
        ("/document/providers/0/preset", json!("openai")),
    ] {
        let mut body = original.clone();
        *body.pointer_mut(path).unwrap() = value;
        let response =
            send_console_json(&fixture.state, Method::PUT, CONFIG, body, None, None).await;
        let status = response.status();
        let result = parse_json(response).await;
        assert!(!status.is_success(), "{result}");
        assert!(
            result
                .to_string()
                .contains("secret_keep_destination_changed"),
            "{result}"
        );
        assert_eq!(draft(&fixture), original);
    }
}

#[test]
fn sap_aicore_management_creates_service_key_without_echo() {
    run(create);
}
async fn create() {
    let mut doc = super::super::tests::document();
    doc["providers"][0]["credentials"] = json!([]);
    let fixture = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), true);
    let grant = grant_console_secret_access(&fixture.state).await;
    let mut body = draft(&fixture);
    body["document"]["providers"][0]["credentials"] =
        json!([{"id":"new-stable","supported_models":["deployment1"]}]);
    body["secretPatches"] =
        json!([{"path":"/providers/0/credentials/0/api_key","operation":"replace","value":KEY}]);
    let response = send_console_json(
        &fixture.state,
        Method::PUT,
        CONFIG,
        body,
        Some(&grant),
        None,
    )
    .await;
    let status = response.status();
    let result = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(!result.to_string().contains("synthetic-sap-secret"));
    let snapshot = fixture.state.route_config.snapshot();
    let account = &snapshot.document().providers[0].credentials[0];
    assert_eq!(account.id.as_deref(), Some("new-stable"));
    assert_eq!(account.api_key.as_deref(), Some(KEY));
}

#[test]
fn sap_aicore_stored_account_discovery_rejects_before_secret_use() {
    run(discovery);
}
async fn discovery() {
    let mut doc = super::super::tests::document();
    // A pretty JSON service key is valid SAP material but invalid generic-discovery key input.
    // Even a regression in the product guard cannot initiate external I/O in this test.
    doc["providers"][0]["credentials"][0]["api_key"] =
        json!(serde_json::to_string_pretty(&serde_json::from_str::<Value>(KEY).unwrap()).unwrap());
    let fixture = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), true);
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        "/v1/internal/gateway/console/account-discovery",
        json!({"credentialId":"stable"}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let result = parse_json(response).await;
    assert!(
        result.to_string().contains("invalid_sap_aicore"),
        "{result}"
    );
    assert!(!result.to_string().contains("synthetic-sap-secret"));
}
