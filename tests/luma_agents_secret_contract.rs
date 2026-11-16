//! Keep binds a saved Agents Key to its original product and complete consumer set.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::json;
fn active(shared: bool) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"renamed","preset":"lumalabs-api",
        "base_url":"https://agents.lumalabs.ai/v1","api_key":if shared {"luma-api-provider"} else {""},
        "credentials":[{"id":"stable-a","api_key":if shared {serde_json::Value::Null} else {json!("luma-api-account")},"supported_models":["uni-1"]}]}]})).unwrap()
}
fn draft(active: &RouteConfigYaml) -> (RouteConfigYaml, Vec<SecretPatch>) {
    let redacted = redact_route_document(active).unwrap();
    let keeps = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    (redacted.document, keeps)
}
#[test]
fn luma_agents_keep_metadata_preserves_original_key_and_stable_account() {
    let active = active(false);
    let (mut document, patches) = draft(&active);
    document.providers[0].label = Some("Metadata".into());
    document.providers[0].credentials[0].enabled = Some(false);
    document.providers[0].credentials[0].supported_models = vec!["ray-3.2".into()];
    let saved = resolve_secret_patches(&active, document, &patches).unwrap();
    assert_eq!(
        saved.providers[0].credentials[0].api_key.as_deref(),
        Some("luma-api-account")
    );
    assert_eq!(
        saved.providers[0].credentials[0].id.as_deref(),
        Some("stable-a")
    );
}
#[test]
fn luma_agents_keep_rejects_erased_preset_target_and_generic_material_entering_product() {
    let active = active(false);
    let (document, patches) = draft(&active);
    for kind in 0..5 {
        let mut changed = document.clone();
        match kind {
            0 => {
                changed.providers[0].preset = Some("openai".into());
                changed.providers[0].base_url = "https://other.example/v1".into();
            }
            1 => changed.providers[0].protocol_profile = Some("lumalabs".into()),
            2 => {
                changed.providers[0].credentials[0].base_url =
                    Some("https://other.example/v1".into())
            }
            3 => {
                changed.providers[0].credentials[0].refresh_endpoint =
                    Some("https://other.example/token".into())
            }
            _ => {
                changed.providers[0].execution_mode =
                    Some(neuro_gateway::routing::candidate::ProviderExecutionMode::BrowserBacked)
            }
        }
        assert_eq!(
            resolve_secret_patches(&active, changed, &patches)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed"
        );
    }
    let mut generic = active.clone();
    generic.providers[0].preset = Some("openai".into());
    generic.providers[0].base_url = "https://other.example/v1".into();
    generic.providers[0].credentials[0].api_key = Some("generic-key".into());
    let (mut incoming, patches) = draft(&generic);
    incoming.providers[0].preset = Some("lumalabs-api".into());
    incoming.providers[0].base_url = "https://agents.lumalabs.ai/v1".into();
    assert_eq!(
        resolve_secret_patches(&generic, incoming, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}
#[test]
fn luma_agents_shared_key_cannot_gain_consumers_even_when_override_is_cleared() {
    let active = active(true);
    let (mut document, patches) = draft(&active);
    document.providers[0]
        .credentials
        .push(serde_json::from_value(json!({"id":"new","enabled":false})).unwrap());
    assert_eq!(
        resolve_secret_patches(&active, document, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
    let mut active = active.clone();
    active.providers[0]
        .credentials
        .push(serde_json::from_value(json!({"id":"own","api_key":"own-key"})).unwrap());
    let (document, mut patches) = draft(&active);
    patches.retain(|p| p.path != "/providers/0/credentials/1/api_key");
    patches.push(SecretPatch::clear("/providers/0/credentials/1/api_key"));
    assert_eq!(
        resolve_secret_patches(&active, document, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}
#[test]
fn luma_agents_explicit_replace_clear_and_reordered_provider_use_normal_secret_owner() {
    let active = active(false);
    let (document, _) = draft(&active);
    let path = "/providers/0/credentials/0/api_key";
    let replaced = resolve_secret_patches(
        &active,
        document.clone(),
        &[SecretPatch::replace(path, json!("luma-api-new"))],
    )
    .unwrap();
    assert_eq!(
        replaced.providers[0].credentials[0].api_key.as_deref(),
        Some("luma-api-new")
    );
    assert!(
        resolve_secret_patches(&active, document, &[SecretPatch::clear(path)])
            .unwrap()
            .providers[0]
            .credentials[0]
            .api_key
            .is_none()
    );
    let (mut document, _) = draft(&active);
    document.providers.insert(
        0,
        serde_json::from_value(json!({"id":"unrelated","base_url":"https://generic.invalid/v1"}))
            .unwrap(),
    );
    let reordered = resolve_secret_patches(
        &active,
        document,
        &[SecretPatch::keep("/providers/1/credentials/0/api_key")],
    )
    .unwrap();
    assert_eq!(
        reordered.providers[1].credentials[0].api_key.as_deref(),
        Some("luma-api-account")
    );
}

#[test]
fn luma_agents_each_public_identity_or_saved_key_owns_keep_without_a_preset() {
    for signal in ["adapter", "profile", "credential_base", "key"] {
        let mut active = active(false);
        let p = &mut active.providers[0];
        p.preset = None;
        p.base_url = "https://generic.invalid/v1".into();
        p.auth_mode = Some("bearer".into());
        p.credentials[0].api_key = Some("synthetic-generic".into());
        match signal {
            "adapter" => p.adapter = Some("lumalabs_agents_compatible".into()),
            "profile" => p.protocol_profile = Some("lumalabs-api".into()),
            "credential_base" => {
                p.credentials[0].base_url = Some("https://agents.lumalabs.ai/v1".into())
            }
            _ => p.credentials[0].api_key = Some("luma-api-synthetic".into()),
        }
        let (mut document, patches) = draft(&active);
        let p = &mut document.providers[0];
        p.adapter = None;
        p.protocol_profile = None;
        p.base_url = "https://other.invalid/v1".into();
        p.credentials[0].base_url = None;
        let error = resolve_secret_patches(&active, document, &patches).unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{signal}");
        assert!(!error.to_string().contains("synthetic"));
    }
}

#[test]
fn luma_agents_shared_keep_allows_new_explicit_key_in_either_patch_order() {
    for reverse in [false, true] {
        let active = active(true);
        let (mut document, mut patches) = draft(&active);
        document.providers[0]
            .credentials
            .push(serde_json::from_value(json!({"id":"new-private"})).unwrap());
        patches.push(SecretPatch::replace(
            "/providers/0/credentials/1/api_key",
            json!("luma-api-new-private"),
        ));
        if reverse {
            patches.reverse();
        }
        let saved = resolve_secret_patches(&active, document, &patches).unwrap();
        assert_eq!(saved.providers[0].api_key, "luma-api-provider");
        assert_eq!(
            saved.providers[0].credentials[1].api_key.as_deref(),
            Some("luma-api-new-private")
        );
    }
}

#[test]
fn luma_agents_ownership_does_not_turn_all_generic_bearer_keys_into_agents() {
    let mut active = active(false);
    active.providers[0].preset = None;
    active.providers[0].auth_mode = Some("bearer".into());
    active.providers[0].base_url = "https://generic.invalid/v1".into();
    active.providers[0].credentials[0].api_key = Some("synthetic-generic".into());
    let (mut document, patches) = draft(&active);
    document.providers[0].base_url = "https://another.invalid/v1".into();
    assert!(resolve_secret_patches(&active, document, &patches).is_ok());
}
