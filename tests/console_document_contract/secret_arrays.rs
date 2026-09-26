//! Array secret reuse requires a unique structural identity, including reorders.

use super::document;
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;

#[test]
fn extra_body_array_secret_paths_use_rfc6901_indexes() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      accounts:
        - label: first
          session: array-secret
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact array secret");
    let path = "/providers/0/extra_body/accounts/0/session";
    assert!(redacted
        .secrets
        .iter()
        .any(|descriptor| descriptor.path == path));
    let resolved = resolve_secret_patches(
        &active,
        redacted.document,
        &[SecretPatch::replace(path, json!("replaced-array-secret"))],
    )
    .expect("replace nested array secret");
    assert_eq!(
        resolved.providers[0].extra_body["accounts"][0]["session"],
        json!("replaced-array-secret")
    );
}

#[test]
fn provider_extra_body_array_reorder_keeps_secrets_by_unique_structural_identity() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      accounts:
        - label: first
          session: first-secret
        - label: second
          session: second-secret
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0]
        .extra_body
        .get_mut("accounts")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    let kept = resolve_secret_patches(
        &active,
        draft.clone(),
        &[
            SecretPatch::keep("/providers/0/extra_body/accounts/0/session"),
            SecretPatch::keep("/providers/0/extra_body/accounts/1/session"),
        ],
    )
    .expect("unique redacted array elements can safely follow a reorder");
    assert_eq!(
        kept.providers[0].extra_body["accounts"][0]["session"],
        json!("second-secret")
    );
    assert_eq!(
        kept.providers[0].extra_body["accounts"][1]["session"],
        json!("first-secret")
    );

    let mut changed = draft.clone();
    changed.providers[0]
        .extra_body
        .get_mut("accounts")
        .unwrap()
        .as_array_mut()
        .unwrap()[0]["label"] = json!("changed");
    let changed = resolve_secret_patches(
        &active,
        changed,
        &[
            SecretPatch::keep("/providers/0/extra_body/accounts/0/session"),
            SecretPatch::keep("/providers/0/extra_body/accounts/1/session"),
        ],
    )
    .expect_err("structural identity changes require explicit replacement");
    assert_eq!(changed.code(), "secret_keep_array_identity_changed");

    let replaced = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace(
                "/providers/0/extra_body/accounts/0/session",
                json!("second-replacement"),
            ),
            SecretPatch::replace(
                "/providers/0/extra_body/accounts/1/session",
                json!("first-replacement"),
            ),
        ],
    )
    .expect("explicit replacement is safe after reorder");
    assert_eq!(
        replaced.providers[0].extra_body["accounts"][0]["session"],
        json!("second-replacement")
    );
}

#[test]
fn credential_extra_body_array_reorder_uses_the_same_keep_guard() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    credentials:
      - id: credential
        extra_body:
          accounts:
            - label: first
              session: first-secret
            - label: second
              session: second-secret
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials[0]
        .extra_body
        .get_mut("accounts")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    let kept = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::keep("/providers/0/credentials/0/extra_body/accounts/0/session"),
            SecretPatch::keep("/providers/0/credentials/0/extra_body/accounts/1/session"),
        ],
    )
    .expect("credential extra_body uses the same unique structural identity mapping");
    assert_eq!(
        kept.providers[0].credentials[0].extra_body["accounts"][0]["session"],
        json!("second-secret")
    );
    assert_eq!(
        kept.providers[0].credentials[0].extra_body["accounts"][1]["session"],
        json!("first-secret")
    );
}

#[test]
fn duplicate_redacted_array_elements_reject_keep_as_ambiguous() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    extra_body:
      accounts:
        - label: same
          session: first-secret
        - label: same
          session: second-secret
model_routes: []
aliases: {}
"#,
    );
    let draft = redact_route_document(&active).unwrap().document;
    let error = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::keep("/providers/0/extra_body/accounts/0/session"),
            SecretPatch::keep("/providers/0/extra_body/accounts/1/session"),
        ],
    )
    .expect_err("duplicate redacted elements cannot identify a secret safely");
    assert_eq!(error.code(), "secret_keep_array_identity_changed");
}
