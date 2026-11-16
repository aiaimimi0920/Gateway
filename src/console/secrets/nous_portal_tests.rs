//! Kept opaque tokens cannot be retargeted by changing a non-secret authorization snapshot.
use super::*;
use serde_json::json;

#[test]
fn nous_secret_keep_binds_client_target_access_digest_subject_hint_and_expiry() {
    let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"nous",
        "base_url":"https://welcome-api.nousresearch.com/v1","credentials":[{"id":"account",
        "api_key":"synthetic-access","refresh_token":"synthetic-refresh","nous_portal":{
            "client_id":"operator-client","inference":"welcome","subject_hint":"synthetic-subject",
            "access_digest":"a".repeat(64),"scope":"inference:invoke","expires_at_unix":4102444800u64}}]}]})).unwrap();
    let redacted = redact_route_document(&active).unwrap().document;
    let encoded = serde_json::to_value(&redacted).unwrap();
    assert!(!encoded.to_string().contains("synthetic-access"));
    assert!(!encoded.to_string().contains("synthetic-refresh"));
    let keeps = ["api_key", "refresh_token"]
        .map(|field| SecretPatch::keep(format!("/providers/0/credentials/0/{field}")));
    assert!(resolve_secret_patches(&active, redacted.clone(), &keeps).is_ok());
    for (field, value) in [
        ("client_id", json!("other-client")),
        ("inference", json!("subscription")),
        ("subject_hint", json!("other-subject")),
        ("access_digest", json!("b".repeat(64))),
        ("expires_at_unix", json!(4102444801u64)),
    ] {
        let mut raw = encoded.clone();
        raw["providers"][0]["credentials"][0]["nous_portal"][field] = value;
        let error = resolve_secret_patches(&active, serde_json::from_value(raw).unwrap(), &keeps)
            .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{field}");
    }
    let mut edited = redacted;
    edited.providers[0].credentials[0].supported_models = vec!["test-model".into()];
    assert!(resolve_secret_patches(&active, edited, &keeps).is_ok());
}
