use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, OnceLock};

use neuro_gateway::console::document::{
    canonicalize_route_document, inspect_route_document, validate_route_document,
};
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};
use time::OffsetDateTime;

fn document(yaml: &str) -> RouteConfigYaml {
    serde_yaml::from_str(yaml).expect("fixture YAML must parse")
}

fn environment_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ScopedEnvVar {
    name: &'static str,
    previous: Option<OsString>,
}

impl ScopedEnvVar {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

fn provider_document(provider_id: &str, api_key: &str) -> RouteConfigYaml {
    document(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com
    api_key: {api_key:?}
model_routes: []
aliases: {{}}
"#
    ))
}

fn diagnostic_codes(error: &neuro_gateway::console::document::RouteConfigDiagnostics) -> Vec<&str> {
    error
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

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
fn bundled_routes_example_is_readable_by_console_redaction_pipeline() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("routes.example.yaml");
    let yaml = fs::read_to_string(&path).expect("read routes.example.yaml");
    let candidate: RouteConfigYaml =
        serde_yaml::from_str(&yaml).expect("routes.example.yaml must parse");

    redact_route_document(&candidate)
        .expect("routes.example.yaml must be readable by the console redaction pipeline");
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
fn websocket_provider_urls_follow_the_real_compiler_transport_rules() {
    let candidate = document(
        r#"
providers:
  - id: xfyun-native
    preset: xfyun-websocket
    base_url: wss://spark-api.xf-yun.com/v1.1/chat
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    validate_route_document(candidate)
        .expect("xfyun websocket providers legitimately use wss URLs");
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
fn canonical_serialization_recursively_sorts_maps_without_sorting_arrays() {
    let left = document(
        r#"
providers:
  - id: stable
    base_url: https://example.com
    api_key: secret
    headers: { Z-Key: z, A-Key: a }
    extra_body:
      outer: { z: 1, a: 2 }
      list:
        - { z: 3, a: 4 }
    model_map: { z-model: z, a-model: a }
model_routes: []
aliases: { zed: z-model, alpha: a-model }
"#,
    );
    let right = document(
        r#"
providers:
  - model_map: { a-model: a, z-model: z }
    extra_body:
      list:
        - { a: 4, z: 3 }
      outer: { a: 2, z: 1 }
    headers: { A-Key: a, Z-Key: z }
    api_key: secret
    base_url: https://example.com
    id: stable
model_routes: []
aliases: { alpha: a-model, zed: z-model }
"#,
    );

    let left = canonicalize_route_document(&left).expect("canonical left");
    let right = canonicalize_route_document(&right).expect("canonical right");
    assert_eq!(left.canonical_json(), right.canonical_json());
    assert_eq!(left.canonical_yaml(), right.canonical_yaml());
    assert_eq!(left.document_digest(), right.document_digest());

    let mut reordered = provider_document("second", "secret");
    reordered
        .providers
        .insert(0, provider_document("first", "secret").providers.remove(0));
    let first_order = canonicalize_route_document(&reordered).expect("first order");
    reordered.providers.swap(0, 1);
    let second_order = canonicalize_route_document(&reordered).expect("second order");
    assert_ne!(
        first_order.document_digest(),
        second_order.document_digest()
    );
}

#[test]
fn canonical_yaml_is_lf_only_utf8_without_bom_and_round_trips() {
    let candidate = provider_document("unicode-provider", "secret-value");
    let canonical = canonicalize_route_document(&candidate).expect("canonical document");
    assert!(!canonical.canonical_yaml().starts_with(&[0xEF, 0xBB, 0xBF]));
    assert!(!canonical
        .canonical_yaml()
        .windows(2)
        .any(|pair| pair == b"\r\n"));
    assert_eq!(canonical.canonical_yaml().last(), Some(&b'\n'));
    let reparsed: RouteConfigYaml =
        serde_yaml::from_slice(canonical.canonical_yaml()).expect("canonical YAML must parse");
    assert_eq!(reparsed.providers[0].id, "unicode-provider");
}

#[test]
fn revision_uses_server_side_secret_digest_and_twelve_hex_prefix() {
    let first = validate_route_document(provider_document("provider", "first-secret"))
        .expect("first validated");
    let second = validate_route_document(provider_document("provider", "second-secret"))
        .expect("second validated");
    assert_ne!(first.document_digest(), second.document_digest());

    let timestamp = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let revision = RevisionMetadata::from_validated(
        7,
        Some("r6-0123456789ab".to_string()),
        RevisionActor::ManagementToken,
        timestamp,
        Some("rotate provider".to_string()),
        &first,
    );
    assert_eq!(
        revision.id(),
        format!("r7-{}", &first.document_digest()[..12])
    );
    assert_eq!(revision.sequence(), 7);
    assert_eq!(revision.parent(), Some("r6-0123456789ab"));
    assert_eq!(revision.document_digest(), first.document_digest());
    assert_eq!(revision.yaml_digest(), first.yaml_digest());
    assert_eq!(revision.message(), Some("rotate provider"));
}

#[test]
fn revision_metadata_wire_deserialization_validates_identity_and_digest_invariants() {
    let validated = validate_route_document(provider_document("provider", "revision-secret"))
        .expect("validated document");
    let revision = RevisionMetadata::from_validated(
        7,
        Some("r6-0123456789ab".to_string()),
        RevisionActor::ManagementToken,
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        Some("roundtrip".to_string()),
        &validated,
    );
    revision.validate().expect("constructed revision is valid");
    let wire = serde_json::to_value(&revision).expect("serialize revision wire");
    let roundtrip: RevisionMetadata =
        serde_json::from_value(wire.clone()).expect("valid revision wire roundtrips");
    assert_eq!(roundtrip.id(), revision.id());
    assert_eq!(roundtrip.document_digest(), revision.document_digest());

    let mut cases = Vec::new();
    let mut forged_id = wire.clone();
    forged_id["id"] = json!("r7-000000000000");
    cases.push(("forged id", forged_id));
    let mut uppercase_digest = wire.clone();
    uppercase_digest["documentDigest"] = json!("A".repeat(64));
    cases.push(("uppercase digest", uppercase_digest));
    let mut short_digest = wire.clone();
    short_digest["yamlDigest"] = json!("abcd");
    cases.push(("short digest", short_digest));
    let mut path_parent = wire.clone();
    path_parent["parent"] = json!("C:\\tmp\\r6-0123456789ab");
    cases.push(("path-like parent", path_parent));
    let mut future_parent = wire;
    future_parent["parent"] = json!("r7-0123456789ab");
    cases.push(("parent sequence not earlier", future_parent));

    for (label, value) in cases {
        assert!(
            serde_json::from_value::<RevisionMetadata>(value).is_err(),
            "malformed revision wire unexpectedly accepted: {label}"
        );
    }
}

#[test]
fn tolerant_startup_can_build_revision_metadata_from_canonical_document() {
    let dangling = document(
        r#"
providers:
  - id: existing
    base_url: https://example.com
    api_key: startup-secret
model_routes:
  - pattern: coder-model
    provider_ids: [missing]
aliases: {}
"#,
    );
    assert!(inspect_route_document(&dangling).requires_repair);
    let canonical = canonicalize_route_document(&dangling).expect("canonical legacy document");
    let metadata = RevisionMetadata::from_canonical(
        1,
        None,
        RevisionActor::Bootstrap,
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        Some("legacy startup".to_string()),
        &canonical,
    );
    assert_eq!(
        metadata.id(),
        format!("r1-{}", &canonical.document_digest()[..12])
    );
    assert_eq!(metadata.document_digest(), canonical.document_digest());
}

#[test]
fn validated_document_debug_output_never_contains_secret_material() {
    let validated = validate_route_document(provider_document(
        "provider",
        "debug-output-must-not-contain-this-secret",
    ))
    .expect("validated document");
    let debug = format!("{validated:?}");
    assert!(!debug.contains("debug-output-must-not-contain-this-secret"));
    assert!(debug.contains(validated.document_digest()));
}

#[test]
fn redaction_covers_typed_nested_and_keepalive_secrets_without_false_positives() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    api_key: provider-key
    auth_token: provider-auth
    keepalive:
      serviceUrl: https://keeper.example.com
      authToken: keeper-auth
    headers:
      Authorization: Bearer provider
      max_tokens: visible-header
    extra_body:
      token: body-token
      nested:
        client_secret: nested-secret
        session_id: nested-session-id
        client_secret_value: nested-secret-value
        authorization_header_value: nested-authorization-value
        aws_secret_access_key: nested-aws-secret-access-key
        max_tokens: 128
        token_expires: visible-token-expires
        token_count: 2
        session_timeout: 30
        oauth_token_endpoint: https://oauth.example.com/token
        token_expires_in_secs: 300
    credentials:
      - api_key: credential-key
        auth_token: credential-auth
        refresh_token: refresh-secret
        keepalive:
          serviceUrl: https://keeper.example.com
          authToken: credential-keeper-auth
        headers:
          Cookie: session=cookie
        extra_body:
          session: body-session
          max_tokens: 256
          token_expires_in_secs: 600
model_routes: []
aliases: {}
"#,
    );

    let redacted = redact_route_document(&active).expect("redact document");
    let provider = &redacted.document.providers[0];
    assert_eq!(provider.api_key, "");
    assert_eq!(provider.auth_token, None);
    assert!(!provider.headers.contains_key("Authorization"));
    assert_eq!(
        provider.headers.get("max_tokens").map(String::as_str),
        Some("visible-header")
    );
    assert!(!provider.extra_body.contains_key("token"));
    assert!(provider.extra_body["nested"].get("session_id").is_none());
    assert!(provider.extra_body["nested"]
        .get("client_secret_value")
        .is_none());
    assert!(provider.extra_body["nested"]
        .get("authorization_header_value")
        .is_none());
    assert!(provider.extra_body["nested"]
        .get("aws_secret_access_key")
        .is_none());
    assert_eq!(provider.extra_body["nested"]["max_tokens"], json!(128));
    assert_eq!(
        provider.extra_body["nested"]["token_expires"],
        json!("visible-token-expires")
    );
    assert_eq!(provider.extra_body["nested"]["token_count"], json!(2));
    assert_eq!(provider.extra_body["nested"]["session_timeout"], json!(30));
    assert_eq!(
        provider.extra_body["nested"]["oauth_token_endpoint"],
        json!("https://oauth.example.com/token")
    );
    assert_eq!(
        provider.extra_body["nested"]["token_expires_in_secs"],
        json!(300)
    );

    let credential = &provider.credentials[0];
    assert_eq!(credential.id.as_deref(), Some("provider-cred-0"));
    assert_eq!(credential.api_key, None);
    assert_eq!(credential.auth_token, None);
    assert_eq!(credential.refresh_token, None);
    assert!(!credential.headers.contains_key("Cookie"));
    assert!(!credential.extra_body.contains_key("session"));
    assert_eq!(credential.extra_body["max_tokens"], json!(256));
    assert_eq!(credential.extra_body["token_expires_in_secs"], json!(600));

    let paths: Vec<&str> = redacted
        .secrets
        .iter()
        .filter(|descriptor| descriptor.configured)
        .map(|descriptor| descriptor.path.as_str())
        .collect();
    for expected in [
        "/providers/0/api_key",
        "/providers/0/auth_token",
        "/providers/0/keepalive/authToken",
        "/providers/0/headers/Authorization",
        "/providers/0/extra_body/token",
        "/providers/0/extra_body/nested/client_secret",
        "/providers/0/extra_body/nested/session_id",
        "/providers/0/extra_body/nested/client_secret_value",
        "/providers/0/extra_body/nested/authorization_header_value",
        "/providers/0/extra_body/nested/aws_secret_access_key",
        "/providers/0/credentials/0/api_key",
        "/providers/0/credentials/0/auth_token",
        "/providers/0/credentials/0/refresh_token",
        "/providers/0/credentials/0/keepalive/authToken",
        "/providers/0/credentials/0/headers/Cookie",
        "/providers/0/credentials/0/extra_body/session",
    ] {
        assert!(paths.contains(&expected), "missing descriptor {expected}");
    }
    assert!(redacted
        .secrets
        .windows(2)
        .all(|pair| pair[0].path <= pair[1].path));
}

#[test]
fn secret_descriptor_preview_is_unicode_safe_and_never_enters_the_document() {
    let active = provider_document("unicode", "密钥秘密值很长而且不能泄露");
    let redacted = redact_route_document(&active).expect("unicode redaction");
    let descriptor = redacted
        .secrets
        .iter()
        .find(|descriptor| descriptor.path == "/providers/0/api_key")
        .expect("api key descriptor");
    assert!(descriptor.configured);
    assert!(descriptor.fingerprint.is_some());
    let preview = descriptor.preview.as_deref().expect("preview");
    assert_ne!(preview, "密钥秘密值很长而且不能泄露");
    assert_eq!(redacted.document.providers[0].api_key, "");
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    assert!(!serialized.contains(preview));
}

#[test]
fn configured_state_distinguishes_provider_empty_string_from_credential_some_empty() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    api_key: ""
    credentials:
      - api_key: ""
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact empty values");
    let descriptors: HashMap<&str, bool> = redacted
        .secrets
        .iter()
        .map(|descriptor| (descriptor.path.as_str(), descriptor.configured))
        .collect();
    assert_eq!(descriptors["/providers/0/api_key"], false);
    assert_eq!(descriptors["/providers/0/credentials/0/api_key"], true);
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

#[test]
fn secret_patch_debug_output_redacts_replace_values() {
    let patch = SecretPatch::replace(
        "/providers/0/api_key",
        json!("debug-must-not-print-this-replacement-secret"),
    );
    let debug = format!("{patch:?}");
    assert!(!debug.contains("debug-must-not-print-this-replacement-secret"));
    assert!(debug.contains("<redacted>"));
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

#[test]
fn strict_url_validation_uses_environment_substitution_but_canonical_stays_raw() {
    let _guard = environment_test_lock();
    let _provider = ScopedEnvVar::set("GW_CONSOLE_PROVIDER_BASE", "https://provider.example.com");
    let _credential = ScopedEnvVar::set(
        "GW_CONSOLE_CREDENTIAL_BASE",
        "https://credential.example.com",
    );
    let _keepalive =
        ScopedEnvVar::set("GW_CONSOLE_KEEPALIVE_BASE", "https://keepalive.example.com");
    let _refresh = ScopedEnvVar::set("GW_CONSOLE_REFRESH_BASE", "https://oauth.example.com/token");
    let _unknown = ScopedEnvVar::remove("GW_CONSOLE_UNKNOWN_BASE");
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: ${GW_CONSOLE_PROVIDER_BASE}
    keepalive:
      serviceUrl: ${GW_CONSOLE_KEEPALIVE_BASE}
    credentials:
      - id: credential
        base_url: ${GW_CONSOLE_CREDENTIAL_BASE}
model_routes: []
aliases: {}
"#,
    );
    let validated = validate_route_document(candidate).expect("resolved URLs are valid");
    assert!(
        String::from_utf8_lossy(validated.canonical_yaml()).contains("${GW_CONSOLE_PROVIDER_BASE}")
    );
    assert!(!String::from_utf8_lossy(validated.canonical_yaml()).contains("provider.example.com"));
    let unknown = document(
        r#"
providers:
  - id: unknown
    base_url: ${GW_CONSOLE_UNKNOWN_BASE}
model_routes: []
aliases: {}
"#,
    );
    let error = validate_route_document(unknown).expect_err("unknown env URL must be diagnosed");
    assert!(diagnostic_codes(&error).contains(&"provider_base_url_invalid"));

    let literal_refresh = document(
        r#"
providers:
  - id: literal-refresh
    base_url: https://example.com
    credentials:
      - id: credential
        refresh_endpoint: ${GW_CONSOLE_REFRESH_BASE}
model_routes: []
aliases: {}
"#,
    );
    let error = validate_route_document(literal_refresh)
        .expect_err("refresh endpoints are literal because the compiler does not substitute them");
    assert!(diagnostic_codes(&error).contains(&"credential_refresh_endpoint_invalid"));
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
fn url_userinfo_is_rejected_and_never_returned_by_redaction() {
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: https://provider-user:provider-url-secret@example.com?api_key=provider-query-secret&region=us
    keepalive:
      serviceUrl: https://keepalive-user:keepalive-url-secret@keeper.example.com
    credentials:
      - id: credential
        base_url: https://credential-user:credential-url-secret@credential.example.com
        refresh_endpoint: https://oauth-user:oauth-url-secret@oauth.example.com/token?client_secret=oauth-query-secret
  - id: websocket
    preset: xfyun-websocket
    base_url: wss://websocket-user:websocket-url-secret@spark-api.xf-yun.com/v1.1/chat
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    let inspection = inspect_route_document(&candidate);
    let codes: Vec<&str> = inspection
        .diagnostics
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    for expected in [
        "provider_base_url_userinfo_forbidden",
        "provider_keepalive_url_userinfo_forbidden",
        "credential_base_url_userinfo_forbidden",
        "credential_refresh_endpoint_userinfo_forbidden",
        "provider_base_url_query_secret_forbidden",
        "credential_refresh_endpoint_query_secret_forbidden",
    ] {
        assert!(codes.contains(&expected), "missing {expected}");
    }
    for misleading in [
        "provider_base_url_invalid",
        "provider_keepalive_url_invalid",
        "credential_base_url_invalid",
        "credential_refresh_endpoint_invalid",
    ] {
        assert!(
            !codes.contains(&misleading),
            "structurally valid unsafe URL should not also report {misleading}"
        );
    }

    let redacted = redact_route_document(&candidate).expect("redaction remains safe for repair UI");
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "provider-url-secret",
        "keepalive-url-secret",
        "credential-url-secret",
        "oauth-url-secret",
        "websocket-url-secret",
        "provider-query-secret",
        "oauth-query-secret",
    ] {
        assert!(!serialized.contains(secret), "URL secret leaked: {secret}");
    }
    assert!(!serialized.contains("provider-user"));
    assert!(!serialized.contains("websocket-user"));
    assert!(serialized.contains("region=us"));
}

#[test]
fn url_fragments_are_rejected_and_removed_from_the_management_document() {
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: https://provider.example.com/v1?region=us#provider-fragment-secret
    keepalive:
      serviceUrl: https://keeper.example.com/health#keepalive-fragment-secret
    credentials:
      - id: credential
        base_url: https://credential.example.com/v1#credential-fragment-secret
        refresh_endpoint: https://oauth.example.com/token#refresh-fragment-secret
  - id: websocket
    preset: xfyun-websocket
    base_url: wss://spark-api.xf-yun.com/v1.1/chat#websocket-fragment-secret
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    let inspection = inspect_route_document(&candidate);
    for (code, path) in [
        (
            "provider_base_url_fragment_forbidden",
            "/providers/0/base_url",
        ),
        (
            "provider_keepalive_url_fragment_forbidden",
            "/providers/0/keepalive/serviceUrl",
        ),
        (
            "credential_base_url_fragment_forbidden",
            "/providers/0/credentials/0/base_url",
        ),
        (
            "credential_refresh_endpoint_fragment_forbidden",
            "/providers/0/credentials/0/refresh_endpoint",
        ),
        (
            "provider_base_url_fragment_forbidden",
            "/providers/1/base_url",
        ),
    ] {
        assert!(
            inspection
                .diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.path == path),
            "missing {code} at {path}"
        );
    }

    let redacted = redact_route_document(&candidate).expect("fragment redaction");
    let provider = &redacted.document.providers[0];
    for value in [
        provider.base_url.as_str(),
        provider.keepalive.as_ref().unwrap().service_url.as_str(),
        provider.credentials[0].base_url.as_deref().unwrap(),
        provider.credentials[0].refresh_endpoint.as_deref().unwrap(),
        redacted.document.providers[1].base_url.as_str(),
    ] {
        let url = url::Url::parse(value).expect("redacted endpoint remains a valid URL");
        assert_eq!(url.fragment(), None, "fragment remained in {value}");
    }
    assert!(provider.base_url.contains("region=us"));
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "provider-fragment-secret",
        "keepalive-fragment-secret",
        "credential-fragment-secret",
        "refresh-fragment-secret",
        "websocket-fragment-secret",
    ] {
        assert!(
            !serialized.contains(secret),
            "fragment secret leaked: {secret}"
        );
    }
}

#[test]
fn composite_cookie_value_keys_are_secret_but_cookie_name_metadata_is_not() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com/v1?cookie_value=query-cookie-secret&cookie_name=session
    headers:
      cookie_value: header-cookie-value-secret
      cookieValue: header-cookie-camel-secret
      set_cookie_value: header-set-cookie-secret
      cookie_header_value: header-cookie-header-secret
      cookie_name: visible-cookie-name
      cookie_names: visible-cookie-names
      cookie_header_name: visible-cookie-header-name
    extra_body:
      nested:
        cookie_value: body-cookie-value-secret
        cookieValue: body-cookie-camel-secret
        set_cookie_value: body-set-cookie-secret
        cookie_header_value: body-cookie-header-secret
        cookie_name: visible-body-cookie-name
        cookie_names: visible-body-cookie-names
model_routes: []
aliases: {}
"#,
    );

    let inspection = inspect_route_document(&active);
    assert!(inspection
        .diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "provider_base_url_query_secret_forbidden"));

    let redacted = redact_route_document(&active).expect("composite cookie redaction");
    let provider = &redacted.document.providers[0];
    for key in [
        "cookie_value",
        "cookieValue",
        "set_cookie_value",
        "cookie_header_value",
    ] {
        assert!(!provider.headers.contains_key(key), "header retained {key}");
        assert!(
            provider.extra_body["nested"].get(key).is_none(),
            "body retained {key}"
        );
    }
    for key in ["cookie_name", "cookie_names", "cookie_header_name"] {
        assert!(provider.headers.contains_key(key), "header removed {key}");
    }
    for key in ["cookie_name", "cookie_names"] {
        assert!(
            provider.extra_body["nested"].get(key).is_some(),
            "body removed {key}"
        );
    }
    assert!(!provider.base_url.contains("query-cookie-secret"));
    assert!(provider.base_url.contains("cookie_name=session"));

    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "query-cookie-secret",
        "header-cookie-value-secret",
        "header-cookie-camel-secret",
        "header-set-cookie-secret",
        "header-cookie-header-secret",
        "body-cookie-value-secret",
        "body-cookie-camel-secret",
        "body-set-cookie-secret",
        "body-cookie-header-secret",
    ] {
        assert!(
            !serialized.contains(secret),
            "cookie secret leaked: {secret}"
        );
    }
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
