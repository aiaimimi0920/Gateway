//! Credential snapshots are additive, credential-only and do not borrow another product's grant.
use neuro_gateway::db::provider_credentials::merge_provider_account_and_credential_payloads;
use neuro_gateway::routing::config::ProviderCredentialYaml;
use serde_json::json;

#[test]
fn nous_context_survives_credential_roundtrip_without_changing_legacy_absence() {
    let context = json!({"client_id":"operator-client","inference":"welcome",
        "access_digest":"a".repeat(64),"scope":"inference:invoke","expires_at_unix":4102444800u64});
    let credential: ProviderCredentialYaml = serde_json::from_value(json!({
        "id":"account","api_key":"synthetic-access","nous_portal":context
    }))
    .unwrap();
    let encoded = serde_json::to_value(credential).unwrap();
    assert_eq!(encoded.get("nous_portal"), Some(&context));
    let legacy: ProviderCredentialYaml = serde_json::from_value(json!({"id":"legacy"})).unwrap();
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("nous_portal")
        .is_none());
}

#[test]
fn nous_context_is_never_inherited_or_recursively_assembled_from_account_payload() {
    let account = json!({"nous_portal":{"client_id":"account-client","inference":"welcome"},
        "base_url":"https://example.com/v1"});
    for credential in [
        json!({}),
        json!({"nous_portal":null}),
        json!({"nous_portal":{"client_id":"credential-client"}}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account, &credential);
        assert_eq!(merged.get("nous_portal"), credential.get("nous_portal"));
        assert_eq!(merged["base_url"], account["base_url"]);
    }
}
