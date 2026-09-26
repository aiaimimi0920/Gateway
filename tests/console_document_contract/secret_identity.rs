//! Secret reuse follows stable provider and credential identities across edits.

use super::{document, provider_document};
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;

#[test]
fn keep_matches_provider_and_credential_ids_across_reorder() {
    let active = document(
        r#"
providers:
  - id: first
    base_url: https://first.example.com
    api_key: first-provider-key
    credentials:
      - id: first-a
        api_key: first-a-key
      - id: first-b
        api_key: first-b-key
  - id: second
    base_url: https://second.example.com
    api_key: second-provider-key
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers.swap(0, 1);
    draft.providers[1].credentials.swap(0, 1);

    let resolved = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::keep("/providers/0/api_key"),
            SecretPatch::keep("/providers/1/api_key"),
            SecretPatch::keep("/providers/1/credentials/0/api_key"),
            SecretPatch::keep("/providers/1/credentials/1/api_key"),
        ],
    )
    .expect("identity-safe keep");
    assert_eq!(resolved.providers[0].id, "second");
    assert_eq!(resolved.providers[0].api_key, "second-provider-key");
    assert_eq!(resolved.providers[1].id, "first");
    assert_eq!(resolved.providers[1].api_key, "first-provider-key");
    assert_eq!(
        resolved.providers[1].credentials[0].id.as_deref(),
        Some("first-b")
    );
    assert_eq!(
        resolved.providers[1].credentials[0].api_key.as_deref(),
        Some("first-b-key")
    );
    assert_eq!(
        resolved.providers[1].credentials[1].api_key.as_deref(),
        Some("first-a-key")
    );
}

#[test]
fn generated_active_ids_allow_existing_anonymous_credentials_to_be_kept() {
    let active = document(
        r#"
providers:
  - id: historical
    base_url: https://example.com
    credentials:
      - api_key: first-secret
      - api_key: second-secret
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials.swap(0, 1);

    let resolved = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::keep("/providers/0/credentials/0/api_key"),
            SecretPatch::keep("/providers/0/credentials/1/api_key"),
        ],
    )
    .expect("materialized IDs make reorder safe");
    assert_eq!(
        resolved.providers[0].credentials[0].id.as_deref(),
        Some("historical-cred-1")
    );
    assert_eq!(
        resolved.providers[0].credentials[0].api_key.as_deref(),
        Some("second-secret")
    );
    assert_eq!(
        resolved.providers[0].credentials[1].api_key.as_deref(),
        Some("first-secret")
    );
}

#[test]
fn raw_anonymous_keep_and_identity_changes_are_rejected() {
    let active = document(
        r#"
providers:
  - id: historical
    base_url: https://example.com
    credentials:
      - api_key: secret
model_routes: []
aliases: {}
"#,
    );
    let mut anonymous_draft = redact_route_document(&active).unwrap().document;
    anonymous_draft.providers[0].credentials[0].id = None;
    let error = resolve_secret_patches(
        &active,
        anonymous_draft,
        &[SecretPatch::keep("/providers/0/credentials/0/api_key")],
    )
    .expect_err("anonymous keep must be rejected");
    assert_eq!(error.code(), "secret_keep_anonymous_credential");

    let provider_active = provider_document("old-id", "secret");
    let mut changed_draft = redact_route_document(&provider_active).unwrap().document;
    changed_draft.providers[0].id = "new-id".to_string();
    let error = resolve_secret_patches(
        &provider_active,
        changed_draft,
        &[SecretPatch::keep("/providers/0/api_key")],
    )
    .expect_err("provider identity change must fail");
    assert_eq!(error.code(), "secret_keep_identity_changed");
}

#[test]
fn identity_change_is_allowed_when_every_secret_is_explicitly_replaced() {
    let active = provider_document("old-provider", "old-secret");
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].id = "new-provider".to_string();
    let resolved = resolve_secret_patches(
        &active,
        draft,
        &[SecretPatch::replace(
            "/providers/0/api_key",
            json!("replacement-secret"),
        )],
    )
    .expect("explicit replace creates a new identity");
    assert_eq!(resolved.providers[0].id, "new-provider");
    assert_eq!(resolved.providers[0].api_key, "replacement-secret");
}

#[test]
fn provider_identity_replacement_requires_actions_for_every_configured_secret() {
    let active = document(
        r#"
providers:
  - id: old-provider
    base_url: https://example.com
    api_key: old-api-key
    auth_token: old-auth-token
    headers:
      Authorization: old-header-secret
    extra_body:
      nested:
        session: old-body-secret
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].id = "new-provider".to_string();

    let missing = resolve_secret_patches(
        &active,
        draft.clone(),
        &[SecretPatch::replace(
            "/providers/0/api_key",
            json!("new-api-key"),
        )],
    )
    .expect_err("identity replacement cannot silently drop other secrets");
    assert_eq!(missing.code(), "secret_patch_missing");

    let cleared = resolve_secret_patches(
        &active,
        draft.clone(),
        &[
            SecretPatch::replace("/providers/0/api_key", json!("new-api-key")),
            SecretPatch::clear("/providers/0/auth_token"),
            SecretPatch::clear("/providers/0/headers/Authorization"),
            SecretPatch::clear("/providers/0/extra_body/nested/session"),
        ],
    )
    .expect_err("identity replacement requires replace rather than clear");
    assert_eq!(
        cleared.code(),
        "secret_identity_replacement_requires_replace"
    );

    let replaced = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace("/providers/0/api_key", json!("new-api-key")),
            SecretPatch::replace("/providers/0/auth_token", json!("new-auth-token")),
            SecretPatch::replace(
                "/providers/0/headers/Authorization",
                json!("new-header-secret"),
            ),
            SecretPatch::replace(
                "/providers/0/extra_body/nested/session",
                json!("new-body-secret"),
            ),
        ],
    )
    .expect("every old provider secret has an explicit replacement");
    assert_eq!(replaced.providers[0].id, "new-provider");
    assert_eq!(replaced.providers[0].api_key, "new-api-key");

    let mut deleted = redact_route_document(&active).unwrap().document;
    deleted.providers.clear();
    let deleted = resolve_secret_patches(&active, deleted, &[])
        .expect("true provider deletion does not require clearing removed secrets");
    assert!(deleted.providers.is_empty());
}

#[test]
fn credential_identity_replacement_requires_actions_for_every_configured_secret() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    credentials:
      - id: old-credential
        api_key: old-api-key
        auth_token: old-auth-token
        headers:
          Authorization: old-header-secret
        extra_body:
          nested:
            session: old-body-secret
model_routes: []
aliases: {}
"#,
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials[0].id = Some("new-credential".to_string());

    let missing = resolve_secret_patches(
        &active,
        draft.clone(),
        &[SecretPatch::replace(
            "/providers/0/credentials/0/api_key",
            json!("new-api-key"),
        )],
    )
    .expect_err("credential replacement cannot silently drop other secrets");
    assert_eq!(missing.code(), "secret_patch_missing");

    let cleared = resolve_secret_patches(
        &active,
        draft.clone(),
        &[
            SecretPatch::replace("/providers/0/credentials/0/api_key", json!("new-api-key")),
            SecretPatch::clear("/providers/0/credentials/0/auth_token"),
            SecretPatch::clear("/providers/0/credentials/0/headers/Authorization"),
            SecretPatch::clear("/providers/0/credentials/0/extra_body/nested/session"),
        ],
    )
    .expect_err("credential identity replacement requires replace rather than clear");
    assert_eq!(
        cleared.code(),
        "secret_identity_replacement_requires_replace"
    );

    let replaced = resolve_secret_patches(
        &active,
        draft,
        &[
            SecretPatch::replace("/providers/0/credentials/0/api_key", json!("new-api-key")),
            SecretPatch::replace(
                "/providers/0/credentials/0/auth_token",
                json!("new-auth-token"),
            ),
            SecretPatch::replace(
                "/providers/0/credentials/0/headers/Authorization",
                json!("new-header-secret"),
            ),
            SecretPatch::replace(
                "/providers/0/credentials/0/extra_body/nested/session",
                json!("new-body-secret"),
            ),
        ],
    )
    .expect("every old credential secret has an explicit replacement");
    assert_eq!(
        replaced.providers[0].credentials[0].id.as_deref(),
        Some("new-credential")
    );

    let mut deleted = redact_route_document(&active).unwrap().document;
    deleted.providers[0].credentials.clear();
    let deleted = resolve_secret_patches(&active, deleted, &[])
        .expect("true credential deletion does not require clearing removed secrets");
    assert!(deleted.providers[0].credentials.is_empty());
}
