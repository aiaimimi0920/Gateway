//! DIM has no private client secret; kept access/refresh tokens remain bound to complete authority.
use super::*;
use serde_json::json;
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"dim","base_url":"https://dimagent.cn/v1",
        "adapter":"openai_compatible","auth_mode":"dim_user_oauth","explicit_credentials_only":true,
        "credentials":[{"id":"account","api_key":"synthetic-access","refresh_token":"synthetic-refresh",
        "dim_oauth":crate::protocol::dim_oauth::test_support::context()}]}]})).unwrap()
}
fn keeps() -> Vec<SecretPatch> {
    ["api_key", "refresh_token"]
        .map(|name| SecretPatch::keep(format!("/providers/0/credentials/0/{name}")))
        .into()
}
#[test]
fn dim_secret_redaction_roundtrip_preserves_nonsecret_context_without_exposing_tokens() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let raw = serde_json::to_value(&redacted.document).unwrap();
    assert_eq!(
        raw["providers"][0]["credentials"][0]["dim_oauth"]["subject"],
        "synthetic-subject"
    );
    assert!(
        !raw.to_string().contains("synthetic-access")
            && !raw.to_string().contains("synthetic-refresh")
    );
    let resolved = resolve_secret_patches(&active, redacted.document, &keeps()).unwrap();
    assert_eq!(
        serde_json::to_value(resolved).unwrap(),
        serde_json::to_value(active).unwrap()
    );
}
#[test]
fn dim_secret_keep_rejects_authority_or_destination_changes_but_allows_model_edit() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap().document;
    for (pointer, value) in [
        ("/base_url", json!("https://dimagent.com/v1")),
        ("/credentials/0/dim_oauth/client_id", json!("other-client")),
        ("/credentials/0/dim_oauth/subject", json!("other-subject")),
        ("/credentials/0/dim_oauth/nonce", json!("other-nonce")),
        (
            "/credentials/0/dim_oauth/token_endpoint",
            json!("https://dimagent.cn/other"),
        ),
    ] {
        let mut value_raw = serde_json::to_value(&redacted).unwrap();
        *value_raw
            .pointer_mut(&format!("/providers/0{pointer}"))
            .unwrap() = value;
        let error = resolve_secret_patches(
            &active,
            serde_json::from_value(value_raw).unwrap(),
            &keeps(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{pointer}");
    }
    let mut draft = redacted;
    draft.providers[0].credentials[0].supported_models = vec!["other-model".into()];
    assert!(resolve_secret_patches(&active, draft, &keeps()).is_ok());
}
