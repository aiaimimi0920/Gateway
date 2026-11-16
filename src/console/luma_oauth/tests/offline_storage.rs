//! Offline context participates in normal validation and secret destination binding.
use super::account;
use crate::console::{
    document::validate_route_document,
    secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
};
use serde_json::json;

#[test]
fn luma_offline_storage_requires_context_and_preserves_disabled_refresh_gates() {
    let mut p = account::provider();
    p["credentials"][0]["refresh_token"] = json!("synthetic-refresh");
    assert!(validate_route_document(account::document(p.clone())).is_err());
    p["credentials"][0]["luma_oauth"]["offline_access"] = json!(true);
    assert!(validate_route_document(account::document(p.clone())).is_ok());
    for (key, value) in [
        ("enabled", json!(true)),
        (
            "refresh_endpoint",
            json!("https://auth.lumalabs.ai/oauth2/token"),
        ),
        ("refresh_client_id", json!("registered-client")),
        ("token_expires_in_secs", json!(3600)),
        ("refresh_token", json!("")),
        ("refresh_token", json!("bad\nvalue")),
    ] {
        let mut changed = p.clone();
        changed["credentials"][0][key] = value;
        assert!(
            validate_route_document(account::document(changed)).is_err(),
            "{key}"
        );
    }
    let active = account::document(p);
    let redacted = redact_route_document(&active).unwrap();
    assert!(!serde_json::to_string(&redacted.document)
        .unwrap()
        .contains("synthetic-"));
    let access = "/providers/0/credentials/0/api_key";
    let refresh = "/providers/0/credentials/0/refresh_token";
    let saved = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::keep(access), SecretPatch::keep(refresh)],
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(&active).unwrap()
    );
    let mut raw = serde_json::to_value(&redacted.document).unwrap();
    raw["providers"][0]["credentials"][0]["luma_oauth"]["offline_access"] = json!(false);
    for keep in [access, refresh] {
        let clear = if keep == access { refresh } else { access };
        let error = resolve_secret_patches(
            &active,
            serde_json::from_value(raw.clone()).unwrap(),
            &[SecretPatch::keep(keep), SecretPatch::clear(clear)],
        )
        .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed");
    }
    // Explicit secret clearing must remain possible while non-secret context stays readable.
    let cleared = resolve_secret_patches(
        &active,
        redacted.document,
        &[SecretPatch::clear(access), SecretPatch::clear(refresh)],
    )
    .unwrap();
    assert!(validate_route_document(cleared).is_ok());
}
