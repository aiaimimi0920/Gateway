//! Keeping secrets must preserve the personal installation, identity and exact token generation.
use super::*;
use serde_json::json;
#[test]
fn lobsterai_secret_keep_cannot_rebind_context_but_allows_model_edits() {
    let tokens=crate::protocol::lobsterai::ReceivedTokens::from_json(
        br#"{"code":0,"data":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh","accountMode":"personal","user":{"yid":"synthetic-user"}}}"#)
        .unwrap().admit_initial(uuid::Uuid::new_v4()).unwrap();
    let active:RouteConfigYaml=serde_json::from_value(json!({"providers":[{"id":"lobster", "base_url":"https://lobsterai-server.youdao.com/api/proxy/v1",
        "credentials":[{"id":"account","api_key":tokens.access_token,"refresh_token":tokens.refresh_token,"lobsterai":tokens.context}]}]})).unwrap();
    let redacted = redact_route_document(&active).unwrap().document;
    let encoded = serde_json::to_value(&redacted).unwrap();
    assert!(!encoded.to_string().contains("synthetic-access"));
    assert!(!encoded.to_string().contains("synthetic-refresh"));
    let keeps = ["api_key", "refresh_token"]
        .map(|field| SecretPatch::keep(format!("/providers/0/credentials/0/{field}")));
    assert!(resolve_secret_patches(&active, redacted.clone(), &keeps).is_ok());
    for (field, value) in [
        ("installationId", json!(uuid::Uuid::new_v4().to_string())),
        ("identity", json!({"yid":"other-user"})),
        ("generation", json!(1)),
        ("tokenBinding", json!("a".repeat(64))),
    ] {
        let mut raw = encoded.clone();
        raw["providers"][0]["credentials"][0]["lobsterai"][field] = value;
        let error = resolve_secret_patches(&active, serde_json::from_value(raw).unwrap(), &keeps)
            .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{field}");
    }
    let mut edited = redacted;
    edited.providers[0].credentials[0].supported_models = vec!["test-model".into()];
    assert!(resolve_secret_patches(&active, edited, &keeps).is_ok());
}
