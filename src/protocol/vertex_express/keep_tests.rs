//! Normal SecretPatch resolution must not retarget an existing Key without secret access.
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::json;

fn active() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"renamed","preset":"vertex-express-api","base_url":super::BASE_URL,
        "credentials":[{"id":"account","api_key":"synthetic-only","supported_models":["gemini-contract"]}]}]})).unwrap()
}
#[test]
fn vertex_express_keep_allows_metadata_but_rejects_audience_profile_and_consumer_changes() {
    let active = active();
    let redacted = redact_route_document(&active).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut metadata = redacted.document.clone();
    metadata.providers[0].label = Some("new label".into());
    metadata.providers[0].credentials[0].supported_models = vec!["gemini-another".into()];
    assert_eq!(
        resolve_secret_patches(&active, metadata, &patches)
            .unwrap()
            .providers[0]
            .credentials[0]
            .api_key
            .as_deref(),
        Some("synthetic-only")
    );
    for kind in 0..5 {
        let mut draft = redacted.document.clone();
        match kind {
            0 => {
                draft.providers[0].base_url =
                    "https://generativelanguage.googleapis.com/v1beta".into()
            }
            1 => draft.providers[0].preset = Some("google-agent-platform-official-api".into()),
            2 => draft.providers[0].protocol_profile = Some("antigravity_oauth".into()),
            3 => draft.providers[0].credentials[0].base_url = Some("https://proxy.invalid".into()),
            _ => {
                draft.providers[0].credentials[0].refresh_endpoint =
                    Some("https://oauth2.googleapis.com/token".into())
            }
        }
        assert_eq!(
            resolve_secret_patches(&active, draft, &patches)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed"
        );
    }
}
#[test]
fn vertex_express_provider_key_keep_cannot_gain_an_inherited_consumer() {
    let mut active = active();
    active.providers[0].credentials.clear();
    active.providers[0].api_key = "provider-synthetic".into();
    let redacted = redact_route_document(&active).unwrap();
    let mut draft = redacted.document;
    draft.providers[0]
        .credentials
        .push(serde_json::from_value(json!({"id":"new-consumer"})).unwrap());
    let patches = vec![SecretPatch::keep("/providers/0/api_key")];
    assert_eq!(
        resolve_secret_patches(&active, draft, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}
#[test]
fn vertex_express_explicit_replace_or_clear_uses_existing_secret_patch_owner() {
    let active = active();
    let draft = redact_route_document(&active).unwrap().document;
    let path = "/providers/0/credentials/0/api_key";
    let replaced = resolve_secret_patches(
        &active,
        draft.clone(),
        &[SecretPatch::replace(path, json!("new-synthetic"))],
    )
    .unwrap();
    assert_eq!(
        replaced.providers[0].credentials[0].api_key.as_deref(),
        Some("new-synthetic")
    );
    let cleared = resolve_secret_patches(&active, draft, &[SecretPatch::clear(path)]).unwrap();
    assert!(cleared.providers[0].credentials[0].api_key.is_none());
}
