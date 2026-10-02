//! Storage auth is redacted, grant-protected and bound to its selected destination.
use super::provider_document;
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;

fn active() -> neuro_gateway::routing::config::RouteConfigYaml {
    let mut document = provider_document("provider", "");
    document.providers[0].credential_storage_connection = Some(serde_json::from_value(json!({
        "type":"webdav", "endpoint":"https://dav.invalid", "directory":"vault", "username":"fixture", "password":"fixture-password"
    })).unwrap());
    document
}

#[test]
fn secrets_are_redacted_and_keep_is_bound_to_destination() {
    let active = active();
    let redacted = redact_route_document(&active).unwrap();
    let path = "/providers/0/credential_storage_connection/password";
    assert!(redacted
        .secrets
        .iter()
        .any(|secret| secret.path == path && secret.configured));
    assert!(!serde_json::to_string(&redacted.document)
        .unwrap()
        .contains("fixture-password"));
    let kept = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::keep(path)],
    )
    .unwrap();
    assert!(serde_json::to_string(&kept)
        .unwrap()
        .contains("fixture-password"));
    for (field, value) in [
        ("endpoint", "https://other.invalid"),
        ("username", "other-user"),
    ] {
        let mut draft = serde_json::to_value(&redacted.document).unwrap();
        draft["providers"][0]["credential_storage_connection"][field] = json!(value);
        let draft = serde_json::from_value(draft).unwrap();
        let error = resolve_secret_patches(&active, draft, &[SecretPatch::keep(path)]).unwrap_err();
        assert_eq!(error.code(), "secret_keep_storage_identity_changed");
    }
}

#[test]
fn create_connection_then_patch_and_protocol_switch_are_supported() {
    let active = active();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credential_storage_connection = Some(serde_json::from_value(json!({
        "type":"s3", "endpoint":"https://s3.invalid", "bucket":"fixture", "region":"auto", "prefix":"vault"
    })).unwrap());
    let resolved = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace(
                "/providers/0/credential_storage_connection/access_key_id",
                json!("fixture-access"),
            ),
            SecretPatch::replace(
                "/providers/0/credential_storage_connection/secret_access_key",
                json!("fixture-secret"),
            ),
        ],
    )
    .unwrap();
    let wire = serde_json::to_string(&resolved).unwrap();
    assert!(wire.contains("fixture-secret"));
    assert!(!wire.contains("fixture-password"));
    let mut draft = redact_route_document(&resolved).unwrap().document;
    draft.providers[0].credential_storage_connection = None;
    assert!(resolve_secret_patches(&resolved, draft, &[]).is_ok());
}

#[test]
fn embedded_auth_requires_secret_patch_and_new_archive_works() {
    let active = provider_document("provider", "");
    let mut draft = active.clone();
    draft.providers[0].credential_archive_connection = Some(serde_json::from_value(json!({
        "type":"webdav", "endpoint":"https://dav.invalid", "directory":"archive", "username":"fixture", "password":"must-not-embed"
    })).unwrap());
    let error = resolve_secret_patches(&active, draft, &[]).unwrap_err();
    assert_eq!(error.code(), "secret_value_must_use_patch");
    assert!(!error.to_string().contains("must-not-embed"));
    let mut draft = active.clone();
    draft.providers[0].credential_archive_connection = Some(serde_json::from_value(json!({
        "type":"webdav", "endpoint":"https://dav.invalid", "directory":"archive", "username":"fixture"
    })).unwrap());
    assert!(resolve_secret_patches(
        &active,
        draft,
        &[SecretPatch::replace(
            "/providers/0/credential_archive_connection/password",
            json!("fixture-password")
        )]
    )
    .is_ok());
}

#[test]
fn s3_keep_rejects_region_bucket_and_endpoint_changes() {
    let mut active = provider_document("provider", "");
    active.providers[0].credential_archive_connection = Some(serde_json::from_value(json!({
        "type":"s3", "endpoint":"https://s3.invalid", "bucket":"fixture", "region":"auto", "access_key_id":"fixture-access", "secret_access_key":"fixture-secret", "session_token":"fixture-session"
    })).unwrap());
    let redacted = redact_route_document(&active).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|secret| secret.configured)
        .map(|secret| SecretPatch::keep(&secret.path))
        .collect();
    for field in ["endpoint", "bucket", "region"] {
        let mut draft = serde_json::to_value(&redacted.document).unwrap();
        draft["providers"][0]["credential_archive_connection"][field] = json!("changed");
        assert_eq!(
            resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &patches)
                .unwrap_err()
                .code(),
            "secret_keep_storage_identity_changed"
        );
    }
}
