//! Catalog policy is optional, but malformed present policy must not silently become a default.
use super::{tests::body, CatalogError, ModelCatalog};
use serde_json::{json, Value};

fn rejected(field: &str, invalid: Value) {
    let mut value = body();
    value["rows"][0][field] = invalid;
    assert_eq!(
        ModelCatalog::parse(&serde_json::to_vec(&value).unwrap()).err(),
        Some(CatalogError::InvalidResponse),
        "{field} must be interpreted, not discarded"
    );
}
fn policy() -> Value {
    json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000})
}

#[test]
fn freebuff_root_model_policy_rejects_missing_wrong_and_null_fields() {
    for invalid in [Value::Null, json!(false), json!([]), json!({})] {
        rejected("compaction", invalid);
    }
    for field in ["cacheExpiryMs", "cacheExpiryMinTokens"] {
        let mut missing = policy();
        missing.as_object_mut().unwrap().remove(field);
        rejected("compaction", missing);
        for invalid in [Value::Null, json!("1"), json!(true), json!(-1), json!(0.5)] {
            let mut value = policy();
            value[field] = invalid;
            rejected("compaction", value);
        }
    }
    for (field, invalid) in [
        ("maxContextLength", Value::Null),
        ("maxContextLength", json!("100")),
        ("maxContextLength", json!(-1)),
        ("maxContextLength", json!(0.5)),
        ("lean", Value::Null),
        ("lean", json!("true")),
        ("lean", json!(1)),
    ] {
        let mut value = policy();
        value[field] = invalid;
        rejected("compaction", value);
    }
}

#[test]
fn freebuff_root_model_policy_rejects_zero_and_unsafe_numeric_bounds() {
    for (field, invalid) in [
        ("cacheExpiryMs", json!(0)),
        ("maxContextLength", json!(0)),
        ("cacheExpiryMs", json!(9_007_199_254_740_992u64)),
        ("cacheExpiryMinTokens", json!(9_007_199_254_740_992u64)),
        ("maxContextLength", json!(9_007_199_254_740_992u64)),
    ] {
        let mut value = policy();
        value[field] = invalid;
        rejected("compaction", value);
    }
}

#[test]
fn freebuff_root_model_legacy_digest_boundary_rejects_non_array_and_wrong_elements() {
    for invalid in [
        Value::Null,
        json!("digest"),
        json!({}),
        json!([1]),
        json!([null]),
        json!([true]),
        json!(["line\nbreak"]),
    ] {
        rejected("legacyDigests", invalid);
    }
}

#[test]
fn freebuff_root_model_policy_metadata_has_explicit_local_size_bounds() {
    rejected("legacyDigests", json!(vec!["unknown"; 65]));
    rejected("legacyDigests", json!(["x".repeat(1025)]));
}
