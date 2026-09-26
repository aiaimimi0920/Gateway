//! Canonical document bytes and revision identity remain one persistence contract.

use super::{document, provider_document};
use neuro_gateway::console::document::{
    canonicalize_route_document, inspect_route_document, validate_route_document,
};
use neuro_gateway::console::revision::{RevisionActor, RevisionMetadata};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use time::OffsetDateTime;

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
