//! Credential-only Kiro registration identity and secret contracts; synthetic material only.
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::console::{refresh_grants::GrantRequest, RouteConfigRedisRevision};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

#[path = "kiro_oauth_context_contract/config.rs"]
mod config;
#[path = "kiro_oauth_context_contract/grants.rs"]
mod grants;
#[path = "kiro_oauth_context_contract/secrets.rs"]
mod secrets;

fn context() -> Value {
    json!({"auth_method":"builder_id","directory_region":"us-east-1",
        "start_url":"https://view.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-registered-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64)})
}
fn document(context: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"pool","preset":"kiro",
        "base_url":"https://q.us-east-1.amazonaws.com","explicit_credentials_only":true,
        "credentials":[{"id":"account","api_key":"synthetic-access","refresh_token":"synthetic-refresh",
        "refresh_endpoint":"https://oidc.us-east-1.amazonaws.com/token","refresh_client_id":"synthetic-client",
        "expires_at":"2020-01-01T00:00:00Z","kiro_oauth":context}]}]})).unwrap()
}
fn request(document: RouteConfigYaml) -> GrantRequest {
    let store = RouteConfigStore::from_document(document.clone()).unwrap();
    let revision =
        RouteConfigRedisRevision::new(store.snapshot().revision().clone(), document).unwrap();
    GrantRequest::from_revision(&revision, "pool", "account").unwrap()
}

#[test]
fn credential_roundtrip_preserves_complete_registered_client_context() {
    let expected = context();
    let value = serde_json::to_value(document(expected.clone())).unwrap();
    assert_eq!(
        value["providers"][0]["credentials"][0]["kiro_oauth"],
        expected
    );
}

#[test]
fn console_redacts_registered_secret_but_preserves_directory_and_scope_metadata() {
    let redacted = redact_route_document(&document(context())).unwrap();
    let path = "/providers/0/credentials/0/kiro_oauth/client_secret";
    assert!(redacted
        .secrets
        .iter()
        .any(|entry| entry.path == path && entry.configured));
    let value = serde_json::to_value(redacted.document).unwrap();
    assert_eq!(
        value["providers"][0]["credentials"][0]["kiro_oauth"]["directory_region"],
        "us-east-1"
    );
    assert!(!value.to_string().contains("synthetic-registered-secret"));
}

#[test]
fn registered_secret_changes_material_identity_without_escaping_consumable_token_family() {
    let original = request(document(context()));
    let mut changed = context();
    changed["client_secret"] = json!("different-registered-secret");
    let changed = request(document(changed));
    assert_eq!(original.family_id(), changed.family_id());
    assert_ne!(original.material_id(), changed.material_id());
}
