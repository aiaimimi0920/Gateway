//! Synthetic authorization snapshots must survive storage and bind every retained secret.
use super::*;
use crate::console::{
    document::canonicalize_route_document,
    secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
};
use serde_json::{json, Value};

pub(super) fn context() -> Value {
    json!({"issuer":"https://auth.x.ai", "client_id":"synthetic-gateway-client",
        "requested_scopes":["openid","profile"], "granted_scopes":["openid","profile"],
        "principal":{"kind":"Team","id":"synthetic-team"}})
}
pub(super) fn document() -> Value {
    let mut value = json!({"providers":[{"id":"build-pool","preset":"grok-oauth",
        "base_url":"https://cli-chat-proxy.grok.com/v1","explicit_credentials_only":true,
        "supported_models":["grok-contract"],
        "credentials":[{"id":"exact-build","api_key":"synthetic-build-token"}]}]});
    let credential = &mut value["providers"][0]["credentials"][0];
    credential["grok_oauth"] = context();
    credential["refresh_token"] = json!("synthetic-refresh");
    credential["expires_at"] = json!("2020-01-01T00:00:00Z");
    value
}
fn typed(value: Value) -> RouteConfigYaml {
    serde_json::from_value(value).unwrap()
}

#[test]
fn grok_context_json_yaml_roundtrip_retains_whole_authorization() {
    let original = typed(document());
    let value = serde_json::to_value(&original).unwrap();
    assert_eq!(
        value["providers"][0]["credentials"][0]["grok_oauth"],
        context()
    );
    let restored: RouteConfigYaml =
        serde_yaml::from_str(&serde_yaml::to_string(&original).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), value);
}

#[test]
fn grok_context_creates_dedicated_refresh_without_legacy_endpoint() {
    let store = crate::routing::config::RouteConfigStore::from_document(typed(document())).unwrap();
    let snapshot = store.snapshot();
    let refresh = snapshot
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap();
    assert_eq!(refresh.refresh_endpoint(), "https://auth.x.ai/oauth2/token");
    assert_eq!(refresh.client_id(), "synthetic-gateway-client");
}

#[test]
fn grok_context_keep_binds_client_scopes_principal_issuer_and_presence() {
    for change in [
        json!({"client_id":"other-client"}),
        json!({"requested_scopes":["other"]}),
        json!({"granted_scopes":["profile"]}),
        json!({"principal":{"kind":"Team","id":"other-team"}}),
        json!({"issuer":"https://untrusted.invalid"}),
        Value::Null,
    ] {
        let active = typed(document());
        let redacted = redact_route_document(&active).unwrap();
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        let mut value = serde_json::to_value(redacted.document).unwrap();
        let target = &mut value["providers"][0]["credentials"][0]["grok_oauth"];
        if change.is_null() {
            *target = Value::Null;
        } else {
            for (key, value) in change.as_object().unwrap() {
                target[key] = value.clone();
            }
        }
        assert!(
            resolve_secret_patches(&active, typed(value), &keeps).is_err(),
            "context mutation was allowed: {change}"
        );
    }
}

#[test]
fn grok_context_rejects_foreign_product_and_mixed_legacy_refresh() {
    for (pointer, value) in [
        ("/providers/0/preset", json!("xai-api")),
        (
            "/providers/0/credentials/0/refresh_endpoint",
            json!("https://untrusted.invalid/token"),
        ),
        (
            "/providers/0/credentials/0/refresh_client_id",
            json!("wrong-client"),
        ),
    ] {
        let mut original = document();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        original
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
        assert!(crate::routing::config::RouteConfigStore::from_document(typed(original)).is_err());
    }
}

#[test]
fn grok_context_alone_cannot_be_attached_to_a_generic_or_xai_api_credential() {
    for marker in [false, true] {
        let mut value = document();
        let provider = &mut value["providers"][0];
        provider["base_url"] = json!("https://api.x.ai/v1");
        provider["preset"] = json!("xai-api");
        provider["credentials"][0]
            .as_object_mut()
            .unwrap()
            .remove("refresh_token");
        if marker {
            provider.as_object_mut().unwrap().remove("preset");
        }
        assert!(crate::routing::config::RouteConfigStore::from_document(typed(value)).is_err());
    }
}

#[test]
fn legacy_document_without_grok_context_retains_canonical_digest() {
    let value = json!({"providers":[{"id":"legacy", "base_url":"https://example.invalid/v1", "credentials":[{"id":"legacy-account", "api_key":"synthetic-legacy"}]}]});
    let document = typed(value);
    let canonical = canonicalize_route_document(&document).unwrap();
    // Both values were captured against the old production schema in red-context-v1.log.
    assert_eq!(
        canonical.document_digest(),
        "2914cac308fd32a7092c2cdfd2c5affd42bbe603f245ba687602153dc7928c26"
    );
    assert_eq!(
        canonical.yaml_digest(),
        "58b95f1c1ab41a0efff9dfee07a5b516fe29789d771bec79fc9daa220ebb7194"
    );
    assert!(
        serde_json::to_value(document).unwrap()["providers"][0]["credentials"][0]
            .get("grok_oauth")
            .is_none()
    );
}

#[test]
fn grok_context_rejects_invalid_snapshot_even_without_refresh_or_enabled_account() {
    for change in [
        json!({"issuer":"https://auth.x.ai.evil.invalid"}),
        json!({"client_id":""}),
        json!({"requested_scopes":[]}),
        json!({"requested_scopes":["openid","openid"]}),
        json!({"granted_scopes":["not-requested"]}),
        json!({"principal":{}}),
        json!({"principal":{"kind":"Team","id":"bad\nid"}}),
        json!({"access_token":"must-not-be-hidden-in-context"}),
    ] {
        let mut value = document();
        let credential = &mut value["providers"][0]["credentials"][0];
        credential["enabled"] = json!(false);
        credential.as_object_mut().unwrap().remove("refresh_token");
        for (key, value) in change.as_object().unwrap() {
            credential["grok_oauth"][key] = value.clone();
        }
        let result = serde_json::from_value(value)
            .ok()
            .and_then(|doc| crate::routing::config::RouteConfigStore::from_document(doc).ok());
        assert!(result.is_none(), "invalid snapshot accepted");
    }
}

#[test]
fn grok_context_keep_is_field_independent_but_explicit_rotation_can_replace_all_secrets() {
    let active = typed(document());
    let redacted = redact_route_document(&active).unwrap();
    let mut value = serde_json::to_value(&redacted.document).unwrap();
    value["providers"][0]["credentials"][0]["grok_oauth"]["client_id"] = json!("new-client");
    for replace_all in [false, true] {
        let patches: Vec<_> = redacted
            .secrets
            .iter()
            .map(|secret| {
                if replace_all || secret.path.ends_with("/api_key") {
                    SecretPatch::replace(&secret.path, json!("synthetic-replacement"))
                } else {
                    SecretPatch::keep(&secret.path)
                }
            })
            .collect();
        assert_eq!(
            resolve_secret_patches(&active, typed(value.clone()), &patches).is_ok(),
            replace_all
        );
    }
    let keeps: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut metadata = redacted.document;
    metadata.providers[0].label = Some("Only a label".into());
    assert!(resolve_secret_patches(&active, metadata, &keeps).is_ok());
}
