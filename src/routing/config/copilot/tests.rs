use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};

fn document() -> Value {
    json!({"providers":[{"id":"copilot", "base_url":"", "auth_mode":"copilot_sdk",
        "explicit_credentials_only":true, "credentials":[{"id":"account", "enabled":true,
        "copilot_github_token":"synthetic-copilot-token"}]}]})
}

#[test]
fn copilot_identity_is_never_projected_and_routing_requires_feature() {
    let document: RouteConfigYaml = serde_json::from_value(document()).unwrap();
    let store = RouteConfigStore::from_document(document.clone()).unwrap();
    let snapshot = store.snapshot();
    let providers = snapshot.get_providers();
    let provider = &providers[0];
    assert!(provider.explicit_credentials_only);
    assert!(provider.payload.base_url.is_empty());
    let credential = &provider.credential_pool[0];
    assert_eq!(credential.enabled, cfg!(feature = "copilot-sdk-runtime"));
    assert!(credential.payload.api_key.is_empty());
    assert!(credential.payload.auth_token.is_none());
    assert!(!format!("{:?}", credential.payload).contains("synthetic-copilot-token"));
    let serialized = serde_yaml::to_string(&document).unwrap();
    let restored: RouteConfigYaml = serde_yaml::from_str(&serialized).unwrap();
    assert_eq!(
        restored.providers[0].credentials[0]
            .copilot_github_token
            .as_deref(),
        Some("synthetic-copilot-token")
    );
}

#[test]
fn copilot_routing_requires_explicit_enable_and_saved_token() {
    for enabled in [None, Some(false), Some(true)] {
        for token in [false, true] {
            let mut value = document();
            value["providers"][0]["credentials"][0]["enabled"] = json!(enabled);
            if !token {
                value["providers"][0]["credentials"][0]["copilot_github_token"] = Value::Null;
            }
            let store =
                RouteConfigStore::from_document(serde_json::from_value(value).unwrap()).unwrap();
            assert_eq!(
                store.snapshot().get_providers()[0].credential_pool[0].enabled,
                cfg!(feature = "copilot-sdk-runtime") && enabled == Some(true) && token
            );
        }
    }
}

#[test]
fn copilot_identity_rejects_transport_mixing_and_legacy_stays_unchanged() {
    for (pointer, value) in [
        ("/providers/0/auth_mode", json!("bearer")),
        ("/providers/0/base_url", json!("https://example.invalid")),
        ("/providers/0/explicit_credentials_only", json!(false)),
        (
            "/providers/0/credentials/0/copilot_github_token",
            json!("bad token"),
        ),
    ] {
        let mut value_document = document();
        *value_document.pointer_mut(pointer).unwrap() = value;
        assert!(
            RouteConfigStore::from_document(serde_json::from_value(value_document).unwrap())
                .is_err()
        );
    }
    for field in ["api_key", "auth_token", "refresh_token", "base_url"] {
        let mut value = document();
        value["providers"][0]["credentials"][0][field] = json!("synthetic-other");
        assert!(RouteConfigStore::from_document(serde_json::from_value(value).unwrap()).is_err());
    }
    let legacy: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"old",
        "base_url":"https://example.invalid", "credentials":[{"id":"old-account","api_key":"old-secret"}]}]})).unwrap();
    assert!(legacy.providers[0].credentials[0]
        .copilot_github_token
        .is_none());
    assert!(!serde_json::to_string(&legacy)
        .unwrap()
        .contains("copilot_github_token"));
    assert!(RouteConfigStore::from_document(legacy).is_ok());
}

#[test]
fn copilot_secret_patch_preserves_reorders_replaces_and_clears() {
    let mut active: RouteConfigYaml = serde_json::from_value(document()).unwrap();
    let mut second = active.providers[0].credentials[0].clone();
    second.id = Some("second".into());
    second.copilot_github_token = Some("second-secret".into());
    active.providers[0].credentials.push(second);
    let redacted = redact_route_document(&active).unwrap();
    assert!(!serde_json::to_string(&redacted.document)
        .unwrap()
        .contains("synthetic-copilot-token"));
    let mut draft = redacted.document;
    draft.providers[0].credentials.reverse();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let kept = resolve_secret_patches(&active, draft, &patches).unwrap();
    assert_eq!(
        kept.providers[0].credentials[0]
            .copilot_github_token
            .as_deref(),
        Some("second-secret")
    );
    let redacted = redact_route_document(&kept).unwrap();
    let mut patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| !s.path.ends_with("copilot_github_token"))
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    patches.push(SecretPatch::replace(
        "/providers/0/credentials/0/copilot_github_token",
        json!("rotated-secret"),
    ));
    patches.push(SecretPatch::clear(
        "/providers/0/credentials/1/copilot_github_token",
    ));
    let saved = resolve_secret_patches(&kept, redacted.document, &patches).unwrap();
    assert_eq!(
        saved.providers[0].credentials[0]
            .copilot_github_token
            .as_deref(),
        Some("rotated-secret")
    );
    assert!(saved.providers[0].credentials[1]
        .copilot_github_token
        .is_none());
    assert!(resolve_secret_patches(&active, active.clone(), &[]).is_err());
}
