//! Secret operation shape, replacement and clear semantics follow the route schema.

use super::{document, provider_document};
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::{json, Value};

#[test]
fn provider_storage_password_uses_the_protected_secret_patch_contract() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    credential_storage_password: old-storage-password
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact storage password");
    let path = "/providers/0/credential_storage_password";
    let descriptor = redacted
        .secrets
        .iter()
        .find(|descriptor| descriptor.path == path)
        .expect("storage password descriptor");
    assert!(descriptor.configured);
    assert_eq!(
        redacted.document.providers[0].credential_storage_password,
        None
    );

    let replaced = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::replace(path, json!("new-storage-password"))],
    )
    .expect("replace storage password");
    assert_eq!(
        replaced.providers[0].credential_storage_password.as_deref(),
        Some("new-storage-password")
    );

    let cleared = resolve_secret_patches(&active, redacted.document, &[SecretPatch::clear(path)])
        .expect("clear storage password");
    assert_eq!(cleared.providers[0].credential_storage_password, None);
}

#[test]
fn provider_and_credential_clear_follow_the_real_schema() {
    let provider_active = provider_document("provider", "old-provider-key");
    let provider_draft = redact_route_document(&provider_active).unwrap().document;
    let provider_resolved = resolve_secret_patches(
        &provider_active,
        provider_draft,
        &[SecretPatch::clear("/providers/0/api_key")],
    )
    .expect("clear provider key");
    assert_eq!(provider_resolved.providers[0].api_key, "");

    let credential_active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    credentials:
      - id: credential
        api_key: old-credential-key
model_routes: []
aliases: {}
"#,
    );
    let credential_draft = redact_route_document(&credential_active).unwrap().document;
    let credential_resolved = resolve_secret_patches(
        &credential_active,
        credential_draft,
        &[SecretPatch::clear("/providers/0/credentials/0/api_key")],
    )
    .expect("clear credential key");
    assert_eq!(
        credential_resolved.providers[0].credentials[0].api_key,
        None
    );
}

#[test]
fn missing_duplicate_and_malformed_patches_are_rejected() {
    let active = provider_document("provider", "secret");
    let draft = redact_route_document(&active).unwrap().document;
    let missing = resolve_secret_patches(&active, draft.clone(), &[])
        .expect_err("configured secret requires an operation");
    assert_eq!(missing.code(), "secret_patch_missing");

    let duplicate = resolve_secret_patches(
        &active,
        draft.clone(),
        &[
            SecretPatch::keep("/providers/0/api_key"),
            SecretPatch::clear("/providers/0/api_key"),
        ],
    )
    .expect_err("duplicate patch path");
    assert_eq!(duplicate.code(), "secret_patch_duplicate");

    let malformed = resolve_secret_patches(
        &active,
        draft,
        &[SecretPatch::keep("/providers/0/headers/bad~2key")],
    )
    .expect_err("invalid RFC6901 escape");
    assert_eq!(malformed.code(), "secret_pointer_invalid");
}

#[test]
fn mask_sentinel_and_generated_preview_are_never_replacement_values() {
    let active = provider_document("provider", "a-very-long-provider-secret");
    let redacted = redact_route_document(&active).unwrap();
    let preview = redacted
        .secrets
        .iter()
        .find(|descriptor| descriptor.path == "/providers/0/api_key")
        .and_then(|descriptor| descriptor.preview.clone())
        .expect("preview");

    for value in [
        Value::String("***".to_string()),
        Value::String(preview),
        Value::String("sk-...".to_string()),
        Value::String("sk-…".to_string()),
    ] {
        let error = resolve_secret_patches(
            &active,
            redacted.document.clone(),
            &[SecretPatch::replace("/providers/0/api_key", value)],
        )
        .expect_err("masked values must fail");
        assert_eq!(error.code(), "secret_mask_sentinel_rejected");
    }

    let accepted = resolve_secret_patches(
        &active,
        redacted.document,
        &[SecretPatch::replace(
            "/providers/0/api_key",
            json!("sk-real-key"),
        )],
    )
    .expect("real sk-prefixed values are not mask placeholders");
    assert_eq!(accepted.providers[0].api_key, "sk-real-key");
}

#[test]
fn header_and_nested_extra_body_replace_and_clear_are_schema_checked() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    headers:
      Authorization: old-header
    extra_body:
      nested:
        session: old-session
model_routes: []
aliases: {}
"#,
    );
    let draft = redact_route_document(&active).unwrap().document;
    let replaced = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace("/providers/0/headers/Authorization", json!("new-header")),
            SecretPatch::clear("/providers/0/extra_body/nested/session"),
        ],
    )
    .expect("replace and clear nested secrets");
    assert_eq!(
        replaced.providers[0]
            .headers
            .get("Authorization")
            .map(String::as_str),
        Some("new-header")
    );
    assert!(replaced.providers[0].extra_body["nested"]
        .get("session")
        .is_none());

    let non_secret_draft = redact_route_document(&active).unwrap().document;
    let error = resolve_secret_patches(
        &active,
        non_secret_draft,
        &[SecretPatch::replace(
            "/providers/0/extra_body/nested/max_tokens",
            json!(2048),
        )],
    )
    .expect_err("non-secret paths cannot use secret patch API");
    assert_eq!(error.code(), "secret_path_outside_schema");
}

#[test]
fn top_level_extra_body_secrets_round_trip_for_providers_and_credentials() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      token: provider-secret
    credentials:
      - id: account
        extra_body:
          session: credential-secret
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact top-level extra-body secrets");
    let provider_path = "/providers/0/extra_body/token";
    let credential_path = "/providers/0/credentials/0/extra_body/session";
    assert!(redacted
        .secrets
        .iter()
        .any(|descriptor| descriptor.path == provider_path));
    assert!(redacted
        .secrets
        .iter()
        .any(|descriptor| descriptor.path == credential_path));

    let resolved = resolve_secret_patches(
        &active,
        redacted.document,
        &[
            SecretPatch::keep(provider_path),
            SecretPatch::keep(credential_path),
        ],
    )
    .expect("top-level extra-body secrets can be kept after redaction");

    assert_eq!(
        resolved.providers[0].extra_body["token"],
        json!("provider-secret")
    );
    assert_eq!(
        resolved.providers[0].credentials[0].extra_body["session"],
        json!("credential-secret")
    );
}

#[test]
fn replace_supports_new_entities_without_an_active_identity() {
    let active = document(
        r#"
providers: []
model_routes: []
aliases: {}
"#,
    );
    let draft = document(
        r#"
providers:
  - id: new-provider
    base_url: https://new.example.com
    api_key: ""
    headers: {}
    extra_body:
      nested: {}
    credentials:
      - id: new-credential
model_routes: []
aliases: {}
"#,
    );
    let resolved = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace("/providers/0/api_key", json!("new-provider-secret")),
            SecretPatch::replace(
                "/providers/0/credentials/0/api_key",
                json!("new-credential-secret"),
            ),
            SecretPatch::replace(
                "/providers/0/headers/Authorization",
                json!("Bearer new-header"),
            ),
            SecretPatch::replace(
                "/providers/0/extra_body/nested/session",
                json!("new-session"),
            ),
        ],
    )
    .expect("replace does not require active identity");
    assert_eq!(resolved.providers[0].api_key, "new-provider-secret");
    assert_eq!(
        resolved.providers[0].credentials[0].api_key.as_deref(),
        Some("new-credential-secret")
    );
    assert_eq!(
        resolved.providers[0]
            .headers
            .get("Authorization")
            .map(String::as_str),
        Some("Bearer new-header")
    );
    assert_eq!(
        resolved.providers[0].extra_body["nested"]["session"],
        json!("new-session")
    );
}

#[test]
fn deserialized_operation_value_shape_is_rejected_by_the_resolver() {
    let active = provider_document("provider", "secret");
    let draft = redact_route_document(&active).unwrap().document;

    let replace_without_value: SecretPatch = serde_json::from_value(json!({
        "path": "/providers/0/api_key",
        "operation": "replace"
    }))
    .expect("wire shape parses before semantic validation");
    let error = resolve_secret_patches(&active, draft.clone(), &[replace_without_value])
        .expect_err("replace without value");
    assert_eq!(error.code(), "secret_patch_value_missing");

    let keep_with_value: SecretPatch = serde_json::from_value(json!({
        "path": "/providers/0/api_key",
        "operation": "keep",
        "value": "must-not-be-accepted"
    }))
    .expect("wire shape parses before semantic validation");
    let error =
        resolve_secret_patches(&active, draft, &[keep_with_value]).expect_err("keep with value");
    assert_eq!(error.code(), "secret_patch_value_unexpected");
}
