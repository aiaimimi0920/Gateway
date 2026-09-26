//! RFC 6901 escaping and array tokens are validated before secret-schema access.

use super::document;
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;

#[test]
fn rfc6901_paths_round_trip_tilde_slash_and_unicode_map_keys() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    headers:
      "Authorization/代理~值": secret-header
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact escaped key");
    let path = "/providers/0/headers/Authorization~1代理~0值";
    assert!(redacted
        .secrets
        .iter()
        .any(|descriptor| descriptor.path == path));

    let kept = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::keep(path)],
    )
    .expect("escaped keep");
    assert_eq!(
        kept.providers[0]
            .headers
            .get("Authorization/代理~值")
            .map(String::as_str),
        Some("secret-header")
    );

    let cleared = resolve_secret_patches(&active, redacted.document, &[SecretPatch::clear(path)])
        .expect("escaped clear");
    assert!(!cleared.providers[0]
        .headers
        .contains_key("Authorization/代理~值"));
}

#[test]
fn nested_array_indexes_follow_rfc6901_array_rules() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      accounts:
        - session: secret
model_routes: []
aliases: {}
"#,
    );
    let draft = redact_route_document(&active).unwrap().document;
    for path in [
        "/providers/0/extra_body/accounts/01/session",
        "/providers/0/extra_body/accounts/-/session",
    ] {
        let error = resolve_secret_patches(&active, draft.clone(), &[SecretPatch::keep(path)])
            .expect_err("invalid array index must be rejected as a pointer error");
        assert_eq!(error.code(), "secret_pointer_invalid", "path={path}");
    }
}

#[test]
fn empty_rfc6901_reference_tokens_are_valid_before_schema_validation() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      nested:
        "":
          session: empty-key-secret
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).unwrap();
    let path = "/providers/0/extra_body/nested//session";
    assert!(redacted
        .secrets
        .iter()
        .any(|descriptor| descriptor.path == path));
    let kept = resolve_secret_patches(
        &active,
        redacted.document.clone(),
        &[SecretPatch::keep(path)],
    )
    .expect("empty reference token keep");
    assert_eq!(
        kept.providers[0].extra_body["nested"][""]["session"],
        json!("empty-key-secret")
    );
    let cleared = resolve_secret_patches(&active, redacted.document, &[SecretPatch::clear(path)])
        .expect("empty reference token clear");
    assert!(cleared.providers[0].extra_body["nested"][""]
        .get("session")
        .is_none());

    let root = resolve_secret_patches(
        &active,
        document(
            r#"
providers:
  - id: provider
    base_url: https://example.com
model_routes: []
aliases: {}
"#,
        ),
        &[SecretPatch::keep("")],
    )
    .expect_err("empty root pointer is valid RFC6901 but outside secret schema");
    assert_eq!(root.code(), "secret_path_outside_schema");
}
