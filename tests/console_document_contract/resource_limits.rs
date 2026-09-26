//! Document, patch and traversal admission limits reject hostile resource sizes.

use super::{diagnostic_codes, document, provider_document};
use neuro_gateway::console::document::{
    canonicalize_route_document, inspect_route_document, validate_route_document,
};
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::RouteConfigStore;
use serde_json::json;
use std::fs;
use std::panic::AssertUnwindSafe;

#[test]
fn alias_cycle_inspection_handles_large_chains_without_rewalking_each_suffix() {
    let mut candidate = provider_document("provider", "");
    for index in 0..4096 {
        candidate
            .aliases
            .insert(format!("alias-{index}"), format!("alias-{}", index + 1));
    }
    candidate
        .aliases
        .insert("alias-4096".to_string(), "terminal-model".to_string());
    assert!(!inspect_route_document(&candidate).requires_repair);

    candidate
        .aliases
        .insert("alias-4096".to_string(), "alias-2048".to_string());
    let inspection = inspect_route_document(&candidate);
    assert!(inspection
        .diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "alias_cycle"));
}

#[test]
fn extreme_token_expiry_is_diagnostic_and_never_panics() {
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    credentials:
      - id: credential
        api_key: access-token
        refresh_token: refresh-token
        refresh_endpoint: https://oauth.example.com/token
        token_expires_in_secs: 18446744073709551615
model_routes: []
aliases: {}
"#,
    );
    let inspection =
        std::panic::catch_unwind(AssertUnwindSafe(|| inspect_route_document(&candidate)))
            .expect("inspection must not panic on hostile duration values");
    assert!(inspection.requires_repair);
    assert!(inspection
        .diagnostics
        .diagnostics
        .iter()
        .any(
            |diagnostic| diagnostic.code == "credential_token_expiry_invalid"
                && diagnostic.path == "/providers/0/credentials/0/token_expires_in_secs"
        ));
    let error = validate_route_document(candidate).expect_err("strict validation rejects expiry");
    assert!(diagnostic_codes(&error).contains(&"credential_token_expiry_invalid"));
}

#[test]
fn legacy_compiler_rejects_extreme_token_expiry_without_panicking() {
    let yaml = r#"
providers:
  - id: provider
    base_url: https://example.com
    credentials:
      - id: credential
        api_key: access-token
        refresh_token: refresh-token
        refresh_endpoint: https://oauth.example.com/token
        token_expires_in_secs: 18446744073709551615
model_routes: []
aliases: {}
"#;
    let path = std::env::temp_dir().join(format!(
        "gateway-console-token-expiry-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, yaml).expect("write token expiry fixture");
    let result =
        std::panic::catch_unwind(AssertUnwindSafe(|| RouteConfigStore::load_from_yaml(&path)))
            .expect("legacy compiler must return an error rather than panic");
    fs::remove_file(&path).ok();
    let error = match result {
        Ok(_) => panic!("legacy compiler must reject an unrepresentable expiry"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("token_expires_in_secs"),
        "unexpected error: {error}"
    );
}

#[test]
fn very_large_secret_preview_is_bounded_and_does_not_echo_the_value() {
    let huge = "密".repeat(200_000);
    let active = provider_document("provider", &huge);
    let redacted = redact_route_document(&active).expect("large secret redaction");
    let preview = redacted
        .secrets
        .iter()
        .find(|descriptor| descriptor.path == "/providers/0/api_key")
        .and_then(|descriptor| descriptor.preview.as_deref())
        .expect("bounded preview");
    assert!(preview.chars().count() <= 9);
    assert!(!preview.contains(&huge));
}

#[test]
fn secret_patch_count_has_a_stable_upper_bound() {
    let active = provider_document("provider", "active-secret");
    let draft = redact_route_document(&active).unwrap().document;
    let patches = (0..4097)
        .map(|_| SecretPatch::keep("/providers/0/api_key"))
        .collect::<Vec<_>>();
    let error = resolve_secret_patches(&active, draft, &patches)
        .expect_err("oversized patch lists must be rejected before per-patch work");
    assert_eq!(error.code(), "secret_patch_count_exceeded");
}

#[test]
fn single_secret_replacement_value_has_a_stable_size_bound() {
    let active = provider_document("provider", "active-secret");
    let draft = redact_route_document(&active).unwrap().document;
    let oversized = "x".repeat(1024 * 1024 + 1);
    let error = resolve_secret_patches(
        &active,
        draft,
        &[SecretPatch::replace(
            "/providers/0/api_key",
            json!(oversized),
        )],
    )
    .expect_err("oversized replacement values must be rejected");
    assert_eq!(error.code(), "secret_patch_value_too_large");
}

#[test]
fn canonical_document_serialization_has_a_stable_size_bound() {
    let mut candidate = provider_document("provider", "secret");
    candidate.providers[0].label = Some("x".repeat(16 * 1024 * 1024 + 1));

    let error = canonicalize_route_document(&candidate)
        .expect_err("oversized canonical documents must be rejected");
    assert!(
        error.to_string().contains("canonical document size limit"),
        "unexpected error: {error}"
    );

    let diagnostics = validate_route_document(candidate)
        .expect_err("strict validation must expose a stable document-size diagnostic");
    assert!(diagnostic_codes(&diagnostics).contains(&"route_config_document_too_large"));
}

#[test]
fn resolver_rejects_oversized_active_and_draft_documents_before_redaction() {
    let mut oversized_active = provider_document("provider", "active-secret");
    oversized_active.providers[0].label = Some("x".repeat(16 * 1024 * 1024 + 1));
    let small_draft = provider_document("provider", "");
    let error = resolve_secret_patches(&oversized_active, small_draft, &[])
        .expect_err("oversized active document must be rejected before redaction");
    assert_eq!(error.code(), "secret_document_too_large");

    let small_active = provider_document("provider", "active-secret");
    let mut oversized_draft = provider_document("provider", "");
    oversized_draft.providers[0].label = Some("x".repeat(16 * 1024 * 1024 + 1));
    let error = resolve_secret_patches(&small_active, oversized_draft, &[])
        .expect_err("oversized draft document must be rejected before redaction");
    assert_eq!(error.code(), "secret_document_too_large");
}

#[test]
fn resolver_rejects_a_single_oversized_patch_path() {
    let active = provider_document("provider", "active-secret");
    let draft = provider_document("provider", "");
    let path = format!("/{}", "x".repeat(16 * 1024));
    let error = resolve_secret_patches(&active, draft, &[SecretPatch::keep(path)])
        .expect_err("single oversized patch path must be rejected early");
    assert_eq!(error.code(), "secret_patch_path_too_large");
}

#[test]
fn resolver_rejects_excessive_total_patch_path_bytes() {
    let active = provider_document("provider", "active-secret");
    let draft = provider_document("provider", "");
    let path = format!("/{}", "x".repeat(16 * 1024 - 1));
    let patches = (0..65)
        .map(|_| SecretPatch::keep(path.clone()))
        .collect::<Vec<_>>();
    let error = resolve_secret_patches(&active, draft, &patches)
        .expect_err("total patch path bytes must be rejected early");
    assert_eq!(error.code(), "secret_patch_paths_too_large");
}

#[test]
fn resolver_rejects_excessive_total_replacement_bytes() {
    let active = provider_document("provider", "active-secret");
    let draft = provider_document("provider", "");
    let value = "x".repeat(1024 * 1024 - 2);
    let patches = (0..17)
        .map(|index| {
            SecretPatch::replace(
                format!("/providers/0/headers/replacement-{index}"),
                json!(value.clone()),
            )
        })
        .collect::<Vec<_>>();
    let error = resolve_secret_patches(&active, draft, &patches)
        .expect_err("total replacement bytes must be rejected early");
    assert_eq!(error.code(), "secret_patch_value_total_too_large");
}
