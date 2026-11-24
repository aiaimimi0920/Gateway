//! Static CLI Key Keep binds the complete consumer; all material here is synthetic.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::RouteConfigYaml,
};
use serde_json::{json, Value};

const BASE: &str = "https://runtime.us-east-1.kiro.dev";
fn document(value: Value) -> RouteConfigYaml {
    serde_json::from_value(value).unwrap()
}
fn fixture(shared: bool) -> RouteConfigYaml {
    document(
        json!({"providers":[{"id":"renamed-pool", "preset":"kiro-api-key",
        "auth_mode":"kiro_api_key", "base_url":BASE, "explicit_credentials_only":true,
        "api_key":if shared {"synthetic-shared"} else {""},
        "credentials":[{"id":"stable-a", "api_key":if shared {Value::Null} else {json!("synthetic-private")}}]}]}),
    )
}
fn redacted(active: &RouteConfigYaml) -> (Value, Vec<SecretPatch>) {
    let redacted = redact_route_document(active).unwrap();
    let patches = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    (serde_json::to_value(redacted.document).unwrap(), patches)
}
fn reject(active: &RouteConfigYaml, draft: Value, patches: &[SecretPatch]) {
    let original = serde_json::to_value(active).unwrap();
    let error = resolve_secret_patches(active, document(draft), patches)
        .expect_err("static Key cannot acquire a different audience through Keep");
    assert_eq!(error.code(), "secret_keep_destination_changed");
    assert!(!error.to_string().contains("synthetic-"));
    assert!(!error.to_string().contains("kiro.dev"));
    assert_eq!(serde_json::to_value(active).unwrap(), original);
}

#[test]
fn kiro_key_keep_preserves_stable_identity_and_allows_metadata_model_edits() {
    let active = fixture(false);
    let (mut draft, keeps) = redacted(&active);
    let account = &mut draft["providers"][0]["credentials"][0];
    account["account_name"] = json!("Renamed account");
    account["enabled"] = json!(false);
    account["supported_models"] = json!(["Exact.Model/Version"]);
    let resolved = resolve_secret_patches(&active, document(draft), &keeps).unwrap();
    let account = &resolved.providers[0].credentials[0];
    assert_eq!(account.id.as_deref(), Some("stable-a"));
    assert_eq!(account.api_key.as_deref(), Some("synthetic-private"));
    assert_eq!(account.supported_models, ["Exact.Model/Version"]);
    assert_eq!(account.enabled, Some(false));
}

#[test]
fn kiro_key_keep_cannot_erase_static_identity_into_generic_or_oauth() {
    for preset in [Value::Null, json!("kiro"), json!("openai")] {
        let active = fixture(false);
        let (mut draft, keeps) = redacted(&active);
        let provider = &mut draft["providers"][0];
        provider["preset"] = preset;
        provider["auth_mode"] = Value::Null;
        provider["base_url"] = json!("https://untrusted.invalid/v1");
        provider["adapter"] = json!("openai_compatible");
        reject(&active, draft, &keeps);
    }
}

#[test]
fn kiro_key_keep_cannot_enter_static_product_with_a_generic_kept_key() {
    let mut active = fixture(false);
    active.providers[0].preset = None;
    active.providers[0].auth_mode = None;
    active.providers[0].base_url = "https://generic.invalid/v1".into();
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["preset"] = json!("kiro-api-key");
    draft["providers"][0]["auth_mode"] = json!("kiro_api_key");
    draft["providers"][0]["base_url"] = json!(BASE);
    reject(&active, draft, &keeps);
}

#[test]
fn kiro_key_credential_mode_or_runtime_host_owns_keep_without_parent_preset() {
    for by_mode in [true, false] {
        let mut active = fixture(false);
        active.providers[0].preset = None;
        active.providers[0].auth_mode = None;
        active.providers[0].base_url = "https://generic.invalid/v1".into();
        if by_mode {
            active.providers[0].credentials[0].auth_mode = Some("kiro_api_key".into());
        } else {
            active.providers[0].credentials[0].base_url = Some(BASE.into());
        }
        let (mut draft, keeps) = redacted(&active);
        draft["providers"][0]["credentials"][0]["base_url"] = json!("https://other.invalid/v1");
        draft["providers"][0]["credentials"][0]["auth_mode"] = Value::Null;
        reject(&active, draft, &keeps);
    }
}

#[test]
fn kiro_key_shared_keep_cannot_gain_new_or_override_cleared_consumers() {
    for clear_override in [false, true] {
        let mut active = fixture(true);
        if clear_override {
            active.providers[0].credentials[0].api_key = Some("synthetic-own".into());
        }
        let (mut draft, mut keeps) = redacted(&active);
        if clear_override {
            keeps.retain(|p| !p.path.contains("credentials"));
            keeps.push(SecretPatch::clear("/providers/0/credentials/0/api_key"));
        } else {
            draft["providers"][0]["credentials"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"new-consumer", "enabled":false}));
        }
        reject(&active, draft, &keeps);
    }
}

#[test]
fn kiro_key_shared_keep_allows_new_explicit_material_in_either_patch_order() {
    for reverse in [false, true] {
        let active = fixture(true);
        let (mut draft, mut patches) = redacted(&active);
        draft["providers"][0]["credentials"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"new-private"}));
        patches.push(SecretPatch::replace(
            "/providers/0/credentials/1/api_key",
            json!("synthetic-new"),
        ));
        if reverse {
            patches.reverse();
        }
        let resolved = resolve_secret_patches(&active, document(draft), &patches).unwrap();
        assert_eq!(
            resolved.providers[0].credentials[1].api_key.as_deref(),
            Some("synthetic-new")
        );
    }
}

#[test]
fn kiro_key_replace_clear_and_reordered_keep_use_existing_secret_patch_owner() {
    let active = fixture(false);
    for clear in [false, true] {
        let (draft, _) = redacted(&active);
        let path = "/providers/0/credentials/0/api_key";
        let patch = if clear {
            SecretPatch::clear(path)
        } else {
            SecretPatch::replace(path, json!("synthetic-replaced"))
        };
        let resolved = resolve_secret_patches(&active, document(draft), &[patch]).unwrap();
        assert_eq!(
            resolved.providers[0].credentials[0].api_key.as_deref(),
            if clear {
                None
            } else {
                Some("synthetic-replaced")
            }
        );
    }
    let (mut draft, _) = redacted(&active);
    draft["providers"].as_array_mut().unwrap().insert(
        0,
        json!({"id":"unrelated", "base_url":"https://generic.invalid"}),
    );
    let resolved = resolve_secret_patches(
        &active,
        document(draft),
        &[SecretPatch::keep("/providers/1/credentials/0/api_key")],
    )
    .unwrap();
    assert_eq!(
        resolved.providers[1].credentials[0].api_key.as_deref(),
        Some("synthetic-private")
    );
}
