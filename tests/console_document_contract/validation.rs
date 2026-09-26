//! Strict validation, legacy loading and document identity diagnostics.

use super::{diagnostic_codes, document};
use neuro_gateway::console::document::{inspect_route_document, validate_route_document};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use std::fs;
use std::panic::AssertUnwindSafe;

#[test]
fn legacy_load_remains_tolerant_while_strict_validation_requires_repair() {
    let yaml = r#"
providers:
  - id: existing
    base_url: https://example.com
    api_key: test
model_routes:
  - pattern: coder-model
    provider_ids: [missing]
aliases: {}
"#;
    let path = std::env::temp_dir().join(format!(
        "gateway-console-dangling-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, yaml).expect("write fixture");

    let legacy = RouteConfigStore::load_from_yaml(&path);
    fs::remove_file(&path).ok();
    assert!(
        legacy.is_ok(),
        "legacy startup compiler must remain tolerant"
    );

    let candidate = document(yaml);
    let inspection = inspect_route_document(&candidate);
    assert!(inspection.requires_repair);
    assert!(inspection
        .diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "route_provider_unknown"
            && diagnostic.path == "/model_routes/0/provider_ids/0"));

    let error = validate_route_document(candidate).expect_err("strict validation must reject");
    assert!(diagnostic_codes(&error).contains(&"route_provider_unknown"));
}

#[test]
fn strict_validation_rejects_duplicate_provider_and_effective_credential_ids() {
    let duplicate_provider = document(
        r#"
providers:
  - id: same
    base_url: https://one.example.com
  - id: same
    base_url: https://two.example.com
model_routes: []
aliases: {}
"#,
    );
    let error = validate_route_document(duplicate_provider).expect_err("duplicate provider IDs");
    assert!(diagnostic_codes(&error).contains(&"provider_id_duplicate"));

    let duplicate_credential = document(
        r#"
providers:
  - id: pool
    base_url: https://example.com
    credentials:
      - api_key: first
      - id: pool-cred-0
        api_key: second
model_routes: []
aliases: {}
"#,
    );
    let error =
        validate_route_document(duplicate_credential).expect_err("effective IDs must be unique");
    assert!(diagnostic_codes(&error).contains(&"credential_id_duplicate"));
}

#[test]
fn strict_validation_rejects_credential_ids_colliding_with_provider_default_accounts() {
    let candidate = document(
        r#"
providers:
  - id: fallback-provider
    base_url: https://fallback.example.com
    api_key: fallback-key
  - id: managed-provider
    base_url: https://managed.example.com
    credentials:
      - id: fallback-provider::default
        api_key: managed-key
model_routes: []
aliases: {}
"#,
    );

    let error = validate_route_document(candidate)
        .expect_err("credential IDs must not collide with provider default accounts");
    assert!(
        diagnostic_codes(&error).contains(&"account_identity_duplicate")
            || diagnostic_codes(&error).contains(&"credential_id_duplicate"),
        "expected a clear account identity collision diagnostic, got {error:?}"
    );
    assert!(error.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "/providers/1/credentials/0/id"
            && diagnostic.message.contains("fallback-provider::default")
    }));
}

#[test]
fn strict_validation_rejects_credential_ids_with_leading_or_trailing_whitespace() {
    let candidate = document(
        r#"
providers:
  - id: managed-provider
    base_url: https://managed.example.com
    credentials:
      - id: " managed-account "
        api_key: managed-key
model_routes: []
aliases: {}
"#,
    );

    let error = validate_route_document(candidate)
        .expect_err("credential IDs with surrounding whitespace must be rejected");
    assert!(
        diagnostic_codes(&error).contains(&"credential_id_whitespace"),
        "expected credential_id_whitespace, got {error:?}"
    );
    assert!(error.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "credential_id_whitespace"
            && diagnostic.path == "/providers/0/credentials/0/id"
    }));
}

#[test]
fn strict_validation_rejects_provider_ids_with_leading_or_trailing_whitespace() {
    let candidate = document(
        r#"
providers:
  - id: " managed-provider "
    base_url: https://managed.example.com
    api_key: managed-key
model_routes: []
aliases: {}
"#,
    );

    let error = validate_route_document(candidate)
        .expect_err("provider IDs with surrounding whitespace must be rejected");
    assert!(diagnostic_codes(&error).contains(&"provider_id_whitespace"));
    assert!(error.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "provider_id_whitespace" && diagnostic.path == "/providers/0/id"
    }));
}

#[test]
fn bundled_route_documents_have_no_dangling_provider_references() {
    for file_name in ["routes.example.yaml", "routes.yaml"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file_name);
        let yaml = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("read {file_name}: {error}");
        });
        let candidate: RouteConfigYaml = serde_yaml::from_str(&yaml).unwrap_or_else(|error| {
            panic!("parse {file_name}: {error}");
        });
        let diagnostics = inspect_route_document(&candidate);
        let dangling = diagnostics
            .diagnostics
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "route_provider_unknown")
            .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
            .collect::<Vec<_>>();

        assert!(
            dangling.is_empty(),
            "{file_name} contains dangling provider references: {dangling:?}"
        );
    }
}

#[test]
fn strict_validation_rejects_empty_ids_invalid_urls_routes_and_alias_cycles() {
    let candidate = document(
        r#"
providers:
  - id: " "
    base_url: file:///tmp/provider
    credentials:
      - id: ""
        base_url: ftp://example.com
model_routes:
  - pattern: " "
    provider_ids: []
aliases:
  first: second
  second: first
"#,
    );
    let error = validate_route_document(candidate).expect_err("invalid document must fail");
    let codes = diagnostic_codes(&error);
    for expected in [
        "provider_id_empty",
        "credential_id_empty",
        "provider_base_url_invalid",
        "credential_base_url_invalid",
        "route_pattern_empty",
        "route_provider_ids_empty",
        "alias_cycle",
    ] {
        assert!(codes.contains(&expected), "missing diagnostic {expected:?}");
    }
}

#[test]
fn validated_document_materializes_historical_anonymous_credential_ids() {
    let candidate = document(
        r#"
providers:
  - id: pool
    base_url: https://example.com
    credentials:
      - api_key: first
      - api_key: second
model_routes: []
aliases: {}
"#,
    );

    let validated = validate_route_document(candidate).expect("valid document");
    assert_eq!(
        validated.document().providers[0].credentials[0]
            .id
            .as_deref(),
        Some("pool-cred-0")
    );
    assert_eq!(
        validated.document().providers[0].credentials[1]
            .id
            .as_deref(),
        Some("pool-cred-1")
    );
}

#[test]
fn normalized_alias_collisions_with_different_targets_are_rejected() {
    let conflicting = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
model_routes: []
aliases:
  foo-bar: target-a
  foo_bar: target-b
"#,
    );
    let error = validate_route_document(conflicting).expect_err("normalized collision");
    assert!(diagnostic_codes(&error).contains(&"alias_normalized_collision"));

    let same_target = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
model_routes: []
aliases:
  foo-bar: target-a
  foo_bar: target-a
"#,
    );
    validate_route_document(same_target).expect("same normalized alias target is deterministic");
}

#[test]
fn legacy_compiler_rejects_normalized_alias_collisions_deterministically() {
    let yaml = r#"
providers:
  - id: provider
    base_url: https://example.com
model_routes: []
aliases:
  foo_bar: target-b
  foo-bar: target-a
"#;
    let path = std::env::temp_dir().join(format!(
        "gateway-console-alias-collision-{}.yaml",
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, yaml).expect("write alias collision fixture");
    let result =
        std::panic::catch_unwind(AssertUnwindSafe(|| RouteConfigStore::load_from_yaml(&path)))
            .expect("legacy compiler must return an error rather than panic");
    fs::remove_file(&path).ok();
    let error = match result {
        Ok(_) => panic!("legacy compiler must reject ambiguous normalized aliases"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(message.contains("foo-bar"), "unexpected error: {message}");
    assert!(message.contains("foo_bar"), "unexpected error: {message}");
}
