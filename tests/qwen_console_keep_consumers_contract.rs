//! Final resolved inheritance, including disabled and newly introduced consumers.
#[path = "qwen_console_secret_contract/support.rs"]
mod support;
use neuro_gateway::console::secrets::{resolve_secret_patches, SecretPatch};
use serde_json::json;
use support::*;

#[test]
fn qwen_shared_key_cannot_gain_new_inherited_consumers_even_when_disabled() {
    for enabled in [true, false] {
        for endpoint in [BASE, OTHER, "https://example.test/v1"] {
            let active = fixture(true);
            let (mut draft, keeps) = redacted(&active);
            draft["providers"][0]["credentials"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"new", "base_url":endpoint, "enabled":enabled}));
            reject(&active, draft, &keeps);
        }
    }
}

#[test]
fn qwen_only_credential_ownership_still_protects_a_generic_provider_key() {
    for marker in [true, false] {
        let mut active = fixture(true);
        active.providers[0].preset = None;
        active.providers[0].auth_mode = None;
        active.providers[0].base_url = "https://example.test/v1".into();
        let (mut draft, keeps) = redacted(&active);
        let account = &mut draft["providers"][0]["credentials"][0];
        if marker {
            account["auth_mode"] = json!("qwen_workspace_api");
        } else {
            account["base_url"] = json!(BASE);
        }
        reject(&active, draft, &keeps);
    }
}

#[test]
fn qwen_private_key_ownership_uses_effective_credential_base_not_just_parent() {
    let mut active = fixture(false);
    active.providers[0].preset = None;
    active.providers[0].auth_mode = None;
    active.providers[0].base_url = "https://example.test/v1".into();
    active.providers[0].credentials[0].base_url = Some(BASE.into());
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["credentials"][0]["base_url"] = json!(OTHER);
    reject(&active, draft, &keeps);
}

#[test]
fn qwen_new_account_with_explicit_key_is_not_a_shared_key_consumer_in_either_patch_order() {
    for reversed in [false, true] {
        let active = fixture(true);
        let (mut draft, mut patches) = redacted(&active);
        draft["providers"][0]["credentials"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"new", "base_url":OTHER}));
        patches.push(SecretPatch::replace(
            "/providers/0/credentials/1/api_key",
            json!("synthetic-own-key"),
        ));
        if reversed {
            patches.reverse();
        }
        let resolved = resolve_secret_patches(&active, document(draft), &patches).unwrap();
        assert_eq!(
            resolved.providers[0].credentials[1].api_key.as_deref(),
            Some("synthetic-own-key")
        );
    }
}

#[test]
fn qwen_dropping_override_cannot_borrow_a_kept_provider_key() {
    let mut active = fixture(true);
    active.providers[0].credentials[0].api_key = Some("synthetic-private-key".into());
    let (draft, mut patches) = redacted(&active);
    patches.retain(|p| !p.path.contains("credentials"));
    patches.push(SecretPatch::clear("/providers/0/credentials/0/api_key"));
    reject(&active, draft, &patches);
}

#[test]
fn qwen_adding_override_cannot_hide_a_kept_private_keys_new_audience() {
    let active = fixture(false);
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["credentials"][0]["base_url"] = json!(OTHER);
    reject(&active, draft, &keeps);
}

#[test]
fn qwen_unchanged_private_override_and_new_unrelated_provider_do_not_broaden_the_key() {
    let mut active = fixture(false);
    active.providers[0].credentials[0].base_url = Some(BASE.into());
    let (mut draft, patches) = redacted(&active);
    draft["providers"][0]["base_url"] = json!(OTHER);
    draft["providers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"unrelated", "base_url":"https://example.test"}));
    let resolved = resolve_secret_patches(&active, document(draft), &patches).unwrap();
    assert_eq!(
        resolved.providers[0].credentials[0].base_url.as_deref(),
        Some(BASE)
    );
}

#[test]
fn qwen_non_key_secrets_cannot_be_kept_into_a_new_consumer_or_refresh_target() {
    for field in [
        "auth_token",
        "refresh_token",
        "headers",
        "extra_body",
        "keepalive",
    ] {
        let mut source = serde_json::to_value(fixture(false)).unwrap();
        source["providers"][0]["credentials"][0]["api_key"] = json!(null);
        let secret = match field {
            "headers" => json!({"Authorization":"synthetic-secondary"}),
            "extra_body" => json!({"access_token":"synthetic-secondary"}),
            "keepalive" => {
                json!({"serviceUrl":"https://example.test", "authToken":"synthetic-secondary"})
            }
            _ => json!("synthetic-secondary"),
        };
        source["providers"][0]["credentials"][0][field] = secret;
        let active = document(source);
        let (mut draft, patches) = redacted(&active);
        assert!(
            !patches.is_empty(),
            "secret fixture must be classified: {field}"
        );
        draft["providers"][0]["credentials"][0]["refresh_endpoint"] =
            json!("https://untrusted.invalid/token");
        reject(&active, draft, &patches);
    }
}

#[test]
fn protected_new_consumer_cannot_clear_map_override_to_acquire_a_kept_generic_secret() {
    for mode in ["qwen_workspace_api", "aws_sigv4"] {
        for (field, key) in [("headers", "Authorization"), ("extra_body", "access_token")] {
            let active = document(
                json!({"providers":[{"id":"generic", "base_url":"https://example.test",
                field:{key:"synthetic-shared"}, "credentials":[{"id":"a", field:{key:"synthetic-private"}}]}]}),
            );
            let (mut draft, _) = redacted(&active);
            draft["providers"][0]["credentials"][0]["auth_mode"] = json!(mode);
            let patches = [
                SecretPatch::keep(format!("/providers/0/{field}/{key}")),
                SecretPatch::clear(format!("/providers/0/credentials/0/{field}/{key}")),
            ];
            reject(&active, draft, &patches);
        }
    }
}

#[test]
fn new_private_qwen_consumer_does_not_protect_unrelated_generic_material() {
    let active = document(
        json!({"providers":[{"id":"generic", "base_url":"https://example.test",
        "api_key":"synthetic-generic-key", "credentials":[{"id":"a"}]}]}),
    );
    for reversed in [false, true] {
        let (mut draft, mut patches) = redacted(&active);
        draft["providers"][0]["base_url"] = json!("https://another.example.test");
        draft["providers"][0]["credentials"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"new-qwen", "base_url":BASE}));
        patches.push(SecretPatch::replace(
            "/providers/0/credentials/1/api_key",
            json!("synthetic-new-key"),
        ));
        if reversed {
            patches.reverse();
        }
        assert!(resolve_secret_patches(&active, document(draft), &patches).is_ok());
    }
}

#[test]
fn qwen_effective_credential_departure_is_protected_without_parent_identity() {
    let mut active = fixture(false);
    active.providers[0].preset = None;
    active.providers[0].auth_mode = None;
    active.providers[0].base_url = "https://example.test/v1".into();
    active.providers[0].credentials[0].base_url = Some(BASE.into());
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["credentials"][0]["base_url"] = json!(null);
    reject(&active, draft, &keeps);
}

#[test]
fn checked_owner_pair_does_not_skip_later_field_new_inheritance() {
    for (field, key) in [("headers", "Authorization"), ("extra_body", "access_token")] {
        for reversed in [false, true] {
            let mut source = serde_json::to_value(fixture(true)).unwrap();
            source["providers"][0][field] = json!({key:"synthetic-shared"});
            source["providers"][0]["credentials"][0][field] = json!({key:"synthetic-private"});
            let active = document(source);
            let (draft, _) = redacted(&active);
            let mut patches = vec![
                SecretPatch::keep("/providers/0/api_key"),
                SecretPatch::keep(format!("/providers/0/{field}/{key}")),
                SecretPatch::clear(format!("/providers/0/credentials/0/{field}/{key}")),
            ];
            if reversed {
                patches.reverse();
            }
            reject(&active, draft, &patches);
        }
    }
}

#[test]
fn private_key_replacement_does_not_authorize_inherited_provider_header_retargeting() {
    let active = document(
        json!({"providers":[{"id":"generic", "base_url":"https://example.test",
        "headers":{"Authorization":"synthetic-shared"},
        "credentials":[{"id":"a", "api_key":"synthetic-private"}]}]}),
    );
    let (mut draft, _) = redacted(&active);
    draft["providers"][0]["credentials"][0]["base_url"] = json!(BASE);
    let patches = [
        SecretPatch::keep("/providers/0/headers/Authorization"),
        SecretPatch::replace(
            "/providers/0/credentials/0/api_key",
            json!("synthetic-new-key"),
        ),
    ];
    reject(&active, draft, &patches);
}
