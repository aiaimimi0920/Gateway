//! Synthetic real SecretPatch resolver: Keep must not rebind a Web session identity.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::json;
fn active() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"web","preset":"qwen-web-chat","base_url":"https://chat.qwen.ai/api/v1",
        "explicit_credentials_only":true,"call_flow":{"special_adapter":"qwen_web","fingerprint_adapter":"session"},
        "credentials":[{"id":"a","api_key":"synthetic-private-token","headers":{"Cookie":"token=synthetic-private-token","User-Agent":"original-UA"},
            "expires_at":"2099-01-01T00:00:00Z","session_auth":{"transport":"bearer","header_name":"authorization"}}]}]})).unwrap()
}
#[test]
fn qwen_web_keep_preserves_secrets_for_metadata_and_model_edits() {
    let active = active();
    let redacted = redact_route_document(&active).unwrap();
    let mut draft = redacted.document;
    draft.providers[0].supported_models = vec!["synthetic-model".into()];
    let keeps: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let resolved = resolve_secret_patches(&active, draft, &keeps).unwrap();
    assert_eq!(
        resolved.providers[0].credentials[0].api_key,
        active.providers[0].credentials[0].api_key
    );
    assert_eq!(
        resolved.providers[0].credentials[0].headers,
        active.providers[0].credentials[0].headers
    );
}
#[test]
fn qwen_web_keep_rejects_audience_auth_ua_and_absolute_expiry_rebinding() {
    let active = active();
    for field in [
        "base",
        "preset",
        "ua",
        "ua-case",
        "provider-ua",
        "expiry",
        "flow",
    ] {
        let redacted = redact_route_document(&active).unwrap();
        let mut draft = redacted.document;
        let provider = &mut draft.providers[0];
        match field {
            "base" => provider.base_url = "https://api.example.invalid".into(),
            "preset" => provider.preset = Some("openai".into()),
            "ua" => {
                provider.credentials[0]
                    .headers
                    .insert("User-Agent".into(), "foreign-UA".into());
            }
            "ua-case" => {
                provider.credentials[0]
                    .headers
                    .insert("user-agent".into(), "foreign-UA".into());
            }
            "provider-ua" => {
                provider
                    .headers
                    .insert("user-agent".into(), "foreign-UA".into());
            }
            "expiry" => provider.credentials[0].expires_at = Some("2099-02-01T00:00:00Z".into()),
            _ => provider.call_flow = None,
        }
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        let error = resolve_secret_patches(&active, draft, &keeps).expect_err(field);
        assert_eq!(error.code(), "secret_keep_destination_changed", "{field}");
        assert!(!error.to_string().contains("synthetic-private"));
    }
}
