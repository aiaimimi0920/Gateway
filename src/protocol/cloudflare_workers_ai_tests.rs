//! Account/preset/Keep policy is tested through the real route and secret owners.
use super::*;
use crate::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"cf","preset":PRESET,
        "base_url":format!("https://api.cloudflare.com/client/v4/accounts/{}/ai/v1", "a".repeat(32)),
        "explicit_credentials_only":true,"call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "credentials":[{"id":"one","api_key":"synthetic-cf-token","supported_models":["@cf/meta/llama-contract"]}]}]})).unwrap()
}
#[test]
fn cloudflare_account_grammar_does_not_normalize_foreign_origins_or_paths() {
    let base = document().providers[0].base_url.clone();
    assert!(Endpoint::parse(&base).is_some());
    for bad in [
        base.replace("https:", "http:"),
        format!("{base}?secret=x"),
        base.replace("api.cloudflare.com", "api.cloudflare.com.evil.invalid"),
        base.replace(&"a".repeat(32), "${CLOUDFLARE_ACCOUNT_ID}"),
    ] {
        assert!(Endpoint::parse(&bad).is_none());
    }
}
#[cfg(feature = "line-cloudflare-workers-ai")]
#[test]
fn cloudflare_configuration_uses_token_pool_and_rejects_cross_account_or_product_overrides() {
    let store = RouteConfigStore::from_document(document()).unwrap();
    let snapshot = store.snapshot();
    let provider = &snapshot.get_providers()[0];
    assert_eq!(provider.protocol_profile, PROFILE);
    assert!(provider.credential_pool[0].refresh_config.is_none());
    let payload = &provider.credential_pool[0].payload;
    assert_eq!(payload.auth_mode.as_deref(), Some(AUTH_MODE));
    assert_eq!(
        crate::implementation_lines::line_for_payload(payload),
        Some(crate::implementation_lines::RefactoredImplementationLine::CloudflareWorkersAi)
    );
    for case in 0..5 {
        let mut bad = document();
        match case {
            0 => {
                bad.providers[0].credentials[0].base_url = Some(
                    bad.providers[0]
                        .base_url
                        .replace(&"a".repeat(32), &"b".repeat(32)),
                )
            }
            1 => bad.providers[0].credentials[0].auth_mode = Some("api-key".into()),
            2 => bad.providers[0].base_url = "https://evil.invalid/v1".into(),
            3 => bad.providers[0].credentials[0].api_key = Some("bad\r\ntoken".into()),
            _ => {
                bad.providers[0].execution_mode =
                    Some(crate::routing::candidate::ProviderExecutionMode::BrowserBacked)
            }
        }
        assert!(
            RouteConfigStore::from_document(bad).is_err(),
            "accepted case {case}"
        );
    }
    let mut empty = document();
    empty.providers[0].credentials.clear();
    RouteConfigStore::from_document(empty).unwrap();
}
#[test]
fn cloudflare_keep_allows_metadata_but_rejects_account_id_or_consumer_changes() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut renamed = redacted.document.clone();
    renamed.providers[0].label = Some("metadata".into());
    assert_eq!(
        resolve_secret_patches(&active, renamed, &patches)
            .unwrap()
            .providers[0]
            .credentials[0]
            .api_key
            .as_deref(),
        Some("synthetic-cf-token")
    );
    for case in 0..3 {
        let mut changed = redacted.document.clone();
        match case {
            0 => {
                changed.providers[0].base_url = changed.providers[0]
                    .base_url
                    .replace(&"a".repeat(32), &"b".repeat(32))
            }
            1 => {
                changed.providers[0].credentials[0].base_url =
                    Some("https://foreign.invalid".into())
            }
            _ => changed.providers[0].preset = Some("openai".into()),
        }
        assert_eq!(
            resolve_secret_patches(&active, changed, &patches)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed"
        );
    }
    let path = "/providers/0/credentials/0/api_key";
    let replaced = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::replace(path, json!("new-synthetic"))],
    )
    .unwrap();
    assert_eq!(
        replaced.providers[0].credentials[0].api_key.as_deref(),
        Some("new-synthetic")
    );
    assert!(
        resolve_secret_patches(&active, redacted.document, &[SecretPatch::clear(path)])
            .unwrap()
            .providers[0]
            .credentials[0]
            .api_key
            .is_none()
    );
}
