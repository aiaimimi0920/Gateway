//! Local-only configuration and deterministic archive ownership contracts.
use super::*;
use crate::routing::config::ProviderCredentialYaml;
use serde_json::json;

fn connection(value: serde_json::Value) -> CredentialStorageConnection {
    serde_json::from_value(value).unwrap()
}

#[test]
fn tls_and_destination_auth_validation_are_explicit() {
    for endpoint in [
        "http://storage.invalid",
        "https://user:password@storage.invalid",
        "https://storage.invalid?token=secret",
        "https://storage.invalid#fragment",
        "https://storage.invalid/../escape",
        "https://storage.invalid/%2e%2e",
        "file:///tmp",
    ] {
        let c = connection(json!({"type":"webdav", "endpoint":endpoint, "directory":""}));
        assert!(validate_connection(&c).is_err(), "{endpoint}");
    }
    assert!(validate_connection(&connection(json!({"type":"webdav", "endpoint":"http://127.0.0.1:8080", "directory":"", "allow_insecure_http":true}))).is_ok());
    assert!(validate_connection(&connection(json!({"type":"webdav", "endpoint":"https://storage.invalid/dav/", "directory":"cloud/folder"}))).is_ok());
}

#[test]
fn prefixes_keys_and_provider_names_cannot_escape_or_collide() {
    for prefix in [
        "../escape",
        "/absolute",
        "a//b",
        "a/./b",
        "a\\b",
        "%2froot",
        "%252e%252e",
        "x?token",
        "a:b",
    ] {
        assert!(configuration::validate_prefix(prefix).is_err());
    }
    for key in ["../x.json", "%2e%2e/x.json", "x.txt", "/x.json", "x.json/"] {
        assert!(validate_object_key(key).is_err());
    }
    let c = connection(
        json!({"type":"s3","endpoint":"https://s3.invalid","bucket":"credentials","region":"auto","prefix":"vault"}),
    );
    assert_ne!(namespace(&c, "a:b", true), namespace(&c, "a_b", true));
    assert_ne!(namespace(&c, "a", true), namespace(&c, "a", false));
    assert!(namespace(&c, "../provider", true).starts_with("vault/gateway-pool/"));
}

#[test]
fn archive_key_is_stable_and_exact_envelope_ownership_is_required() {
    let credential: ProviderCredentialYaml = serde_json::from_value(
        json!({"id":"synthetic", "api_key":"synthetic-credential", "headers":{"b":"2", "a":"1"}}),
    )
    .unwrap();
    let key = archive_record::object_key("synthetic", &credential).unwrap();
    let second: ProviderCredentialYaml = serde_json::from_value(
        json!({"id":"synthetic", "api_key":"synthetic-credential", "headers":{"a":"1", "b":"2"}}),
    )
    .unwrap();
    assert_eq!(
        key,
        archive_record::object_key("synthetic", &second).unwrap()
    );
    let mut value = json!({"schemaVersion":1,"providerId":"provider-a","credentialId":"synthetic","archivedAt":"2026-10-02T00:00:00Z","reason":"permanent_driver_rejection","credential":credential});
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(archive_record::owned_record(&bytes, &key, "provider-a").is_some());
    assert!(archive_record::owned_record(&bytes, &key, "provider-b").is_none());
    assert!(archive_record::owned_record(&bytes, "arbitrary.json", "provider-a").is_none());
    value["credential"]["api_key"] = json!("repaired-key");
    assert!(
        archive_record::owned_record(&serde_json::to_vec(&value).unwrap(), &key, "provider-a")
            .is_none()
    );
}

#[test]
fn connection_debug_never_prints_auth_or_endpoint() {
    let c = connection(
        json!({"type":"s3","endpoint":"https://s3.invalid","bucket":"bucket","region":"auto","access_key_id":"synthetic-access", "secret_access_key":"synthetic-secret", "session_token":"synthetic-token"}),
    );
    let debug = format!("{c:?}");
    assert!(!debug.contains("synthetic"));
    assert!(!debug.contains("s3.invalid"));
}

#[path = "tests/s3_tests.rs"]
mod s3_tests;

#[test]
fn archive_revision_keys_do_not_reuse_legacy_or_other_revision_objects() {
    let credential: ProviderCredentialYaml =
        serde_json::from_value(json!({"id":"a","api_key":"fixture"})).unwrap();
    let r = "r1-000000000000";
    let next = "r2-000000000000";
    let key = archive_record::revision_object_key("a", &credential, r).unwrap();
    assert_ne!(key, archive_record::object_key("a", &credential).unwrap());
    assert_ne!(
        key,
        archive_record::revision_object_key("a", &credential, next).unwrap()
    );
    assert_eq!(
        key,
        archive_record::revision_object_key("a", &credential, r).unwrap()
    );
    let mut record = json!({"schemaVersion":2,"sourceRevision":r,"providerId":"p","credentialId":"a","archivedAt":"2026-10-02T00:00:00Z","reason":"permanent_driver_rejection","credential":credential});
    assert!(
        archive_record::owned_record(&serde_json::to_vec(&record).unwrap(), &key, "p").is_some()
    );
    record["sourceRevision"] = json!(next);
    assert!(
        archive_record::owned_record(&serde_json::to_vec(&record).unwrap(), &key, "p").is_none()
    );
}
