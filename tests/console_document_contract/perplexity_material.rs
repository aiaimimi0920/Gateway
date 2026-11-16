//! Synthetic secrets exercise the normal resolver; no browser, network or account reads.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::{json, Value};

fn active() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{
        "id":"renamable", "preset":"perplexity-web", "base_url":"https://www.perplexity.ai",
        "explicit_credentials_only":true, "credentials":[{"id":"stable-account",
            "api_key":"synthetic-session", "headers":{"Cookie":"__Secure-next-auth.session-token=synthetic-session; cf_clearance=synthetic-clearance", "User-Agent":"synthetic-agent"},
            "session_auth":{"transport":"cookie", "primary_cookie_name":"__Secure-next-auth.session-token"}}]
    }], "model_routes":[]})).unwrap()
}
fn keeps(document: &RouteConfigYaml) -> Vec<SecretPatch> {
    redact_route_document(document)
        .unwrap()
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect()
}

#[test]
fn perplexity_material_keep_cannot_retarget_or_erase_original_product() {
    let active = active();
    for (pointer, value) in [
        ("/providers/0/base_url", json!("https://api.perplexity.ai")),
        ("/providers/0/preset", json!("perplexity-api")),
        (
            "/providers/0/credentials/0/base_url",
            json!("https://attacker.invalid"),
        ),
        (
            "/providers/0/credentials/0/session_auth/primaryCookieName",
            json!("other"),
        ),
        (
            "/providers/0/credentials/0/headers/User-Agent",
            json!("other-agent"),
        ),
        (
            "/providers/0/credentials/0/expires_at",
            json!("2099-01-01T00:00:00Z"),
        ),
        ("/providers/0/expires_at", json!("2099-01-01T00:00:00Z")),
    ] {
        let mut draft =
            serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
        // Optional fields are inserted at their owner rather than assuming a JSON pointer exists.
        let (owner, field) = pointer.rsplit_once('/').unwrap();
        draft
            .pointer_mut(owner)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), value);
        let error = resolve_secret_patches(
            &active,
            serde_json::from_value(draft).unwrap(),
            &keeps(&active),
        )
        .expect_err(pointer);
        assert_eq!(error.code(), "secret_keep_destination_changed", "{pointer}");
    }
}

#[test]
fn perplexity_material_keep_cannot_remove_all_product_markers_together() {
    let active = active();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].preset = None;
    draft.providers[0].adapter = Some("openai_compatible".into());
    draft.providers[0].auth_mode = Some("bearer".into());
    draft.providers[0].base_url = "https://api.perplexity.ai".into();
    draft.providers[0].credentials[0].session_auth = None;
    let error = resolve_secret_patches(&active, draft, &keeps(&active)).unwrap_err();
    assert_eq!(error.code(), "secret_keep_destination_changed");
}

#[test]
fn perplexity_material_keep_preserves_ids_secrets_and_non_auth_edits() {
    let active = active();
    let redacted = redact_route_document(&active).unwrap();
    let encoded = serde_json::to_string(&redacted.document).unwrap();
    assert!(!encoded.contains("synthetic-session") && !encoded.contains("synthetic-clearance"));
    let mut draft = redacted.document;
    draft.providers[0].label = Some("operator label".into());
    draft.providers[0].credentials[0].enabled = Some(false);
    let resolved = resolve_secret_patches(&active, draft, &keeps(&active)).unwrap();
    assert_eq!(
        resolved.providers[0].credentials[0].id.as_deref(),
        Some("stable-account")
    );
    assert_eq!(
        resolved.providers[0].credentials[0].api_key.as_deref(),
        Some("synthetic-session")
    );
    assert_eq!(resolved.providers[0].credentials[0].enabled, Some(false));
}

#[test]
fn perplexity_material_explicit_replace_and_clear_can_change_destination() {
    let active = active();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].preset = Some("perplexity-api".into());
    draft.providers[0].base_url = "https://api.perplexity.ai".into();
    let patches: Vec<_> = keeps(&active)
        .into_iter()
        .map(|patch| {
            if patch.path.ends_with("/api_key") {
                SecretPatch::replace(patch.path, Value::String("synthetic-api-key".into()))
            } else {
                SecretPatch::clear(patch.path)
            }
        })
        .collect();
    let resolved = resolve_secret_patches(&active, draft, &patches).unwrap();
    assert_eq!(
        resolved.providers[0].credentials[0].api_key.as_deref(),
        Some("synthetic-api-key")
    );
    assert!(!resolved.providers[0].credentials[0]
        .headers
        .contains_key("Cookie"));
}
