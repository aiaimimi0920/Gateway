//! New receipts reject custom destinations without changing legacy FreeBuff/Codebuff adapter behavior.
use super::*;
use serde_json::{json, Value};

fn raw() -> Value {
    json!({"providers":[{"id":"freebuff-pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[{"id":"stable-account","api_key":"synthetic-token",
        "freebuff_session":{"fingerprint_id":"synthetic-device","fingerprint_hash":"synthetic-hash"}}]}]})
}
fn valid(value: Value) -> bool {
    let document: RouteConfigYaml = serde_json::from_value(value).unwrap();
    crate::console::document::validate_route_document(document).is_ok()
}
#[test]
fn freebuff_session_typed_receipt_accepts_fixed_product_and_redacted_or_cleared_material() {
    let document: RouteConfigYaml = serde_json::from_value(raw()).unwrap();
    assert!(valid(raw()));
    let redacted = crate::console::secrets::redact_route_document(&document)
        .unwrap()
        .document;
    assert!(crate::console::document::validate_route_document(redacted).is_ok());
    for base in [
        "https://freebuff.com",
        "https://freebuff.com/",
        "https://freebuff.com:443/",
    ] {
        let mut value = raw();
        value["providers"][0]["base_url"] = json!(base);
        assert!(valid(value), "{base}");
    }
}
#[test]
fn freebuff_session_typed_receipt_rejects_non_product_targets_and_conflicting_owners() {
    for (field, value) in [
        ("base_url", json!("https://www.codebuff.com")),
        ("base_url", json!("https://freebuff.com/other")),
        ("base_url", json!("https://freebuff.com?token=private")),
        ("base_url", json!("https://user@freebuff.com/")),
        ("base_url", json!("https://freebuff.com:8443/")),
        ("adapter", json!("openai_compatible")),
        ("api_key", json!("shared-token")),
        ("auth_token", json!("shared-cookie")),
        ("headers", json!({"authorization":"Bearer other"})),
        ("headers", json!({"Host":"evil.invalid"})),
        ("chat_completions_path", json!("/other")),
        ("extra_body", json!({"freebuffStartRunPath":"/other"})),
        (
            "call_flow",
            json!({"call_adapter":"openai","special_adapter":"none"}),
        ),
        ("request_client_profile", json!("codex_cli")),
    ] {
        let mut raw = raw();
        raw["providers"][0][field] = value;
        assert!(!valid(raw), "{field}");
    }
    for (field, value) in [
        ("refresh_token", json!("other-grant")),
        ("refresh_endpoint", json!("https://evil.invalid/token")),
        ("auth_token", json!("other-cookie")),
        ("extra_body", json!({"fingerprintHash":"other-receipt"})),
        ("base_url", json!("https://evil.invalid")),
        ("headers", json!({"authorization":"Bearer other"})),
        ("extra_body", json!({"freebuff_finish_run_path":"/other"})),
    ] {
        let mut raw = raw();
        raw["providers"][0]["credentials"][0][field] = value;
        assert!(!valid(raw), "credential {field}");
    }
}
#[test]
fn freebuff_session_typed_receipt_rejects_partial_oversized_or_header_unsafe_private_material() {
    for value in [
        json!({"fingerprint_id":"device"}),
        json!({"fingerprint_hash":"hash"}),
        json!({"fingerprint_id":"a".repeat(513),"fingerprint_hash":"hash"}),
        json!({"fingerprint_id":"device","fingerprint_hash":"bad\r\nhash"}),
        json!({}),
    ] {
        let mut raw = raw();
        raw["providers"][0]["credentials"][0]["freebuff_session"] = value;
        assert!(!valid(raw));
    }
}
#[test]
fn freebuff_session_legacy_custom_adapter_and_paths_remain_compatible_without_typed_receipt() {
    let mut raw = raw();
    raw["providers"][0]["credentials"][0]
        .as_object_mut()
        .unwrap()
        .remove("freebuff_session");
    raw["providers"][0]["base_url"] = json!("https://www.codebuff.com");
    raw["providers"][0]["extra_body"] = json!({"freebuffStartRunPath":"/custom"});
    assert!(valid(raw));
}

#[test]
fn freebuff_business_target_rejects_inherited_aliased_or_malformed_authority() {
    let target = json!({"origin":"https://codebuff.com","client":"cli"});
    let mut inherited = raw();
    inherited["providers"][0]["extra_body"] = json!({"freebuffBusiness":target});
    assert!(
        !valid(inherited),
        "a business audience must be account-owned"
    );
    for value in [
        json!({"origin":"https://evil.invalid","client":"cli"}),
        json!({"origin":"https://codebuff.com/","client":"cli"}),
        json!({"origin":"https://codebuff.com","client":"web"}),
        json!({"origin":"https://codebuff.com","client":"cli","handle":"fbm1.private"}),
        json!(null),
    ] {
        let mut input = raw();
        input["providers"][0]["credentials"][0]["extra_body"] = json!({"freebuffBusiness":value});
        assert!(
            !valid(input),
            "malformed authority must not silently select legacy execution"
        );
    }
    let mut alias = raw();
    alias["providers"][0]["credentials"][0]["extra_body"] = json!({"freebuff_business":target});
    assert!(!valid(alias));
}

#[test]
fn freebuff_business_target_keeps_login_audience_and_exact_model_ownership_after_yaml_roundtrip() {
    for (origin, client) in [
        ("https://codebuff.com", "cli"),
        ("https://www.codebuff.com", "desktop"),
    ] {
        let mut input = raw();
        let target = json!({"origin":origin,"client":client});
        input["providers"][0]["credentials"][0]["extra_body"] =
            json!({"custom":true,"freebuffBusiness":target});
        input["providers"][0]["credentials"][0]["model_selection"] =
            json!({"fetched":["m-stable"],"manual":["manual"]});
        input["providers"][0]["credentials"][0]["supported_models"] = json!(["m-stable", "manual"]);
        input["providers"][0]["credentials"].as_array_mut().unwrap().push(json!({"id":"sibling","api_key":"synthetic-sibling","supported_models":["sibling-model"]}));
        input["providers"][0]["supported_models"] = json!(["m-stable", "manual", "sibling-model"]);
        let document: RouteConfigYaml = serde_json::from_value(input).unwrap();
        let roundtrip: RouteConfigYaml =
            serde_yaml::from_str(&serde_yaml::to_string(&document).unwrap()).unwrap();
        let store = RouteConfigStore::from_document(roundtrip).unwrap();
        let snapshot = store.snapshot();
        for model in ["m-stable", "manual"] {
            let candidates = snapshot.resolve_candidates(Some(model));
            assert_eq!(candidates.len(), 1);
            assert_eq!(
                candidates[0].payload.credential_id.as_deref(),
                Some("stable-account")
            );
            assert_eq!(candidates[0].payload.base_url, "https://freebuff.com");
            assert_eq!(
                candidates[0].payload.extra_body.as_ref().unwrap()["freebuffBusiness"],
                target
            );
        }
        let sibling = snapshot.resolve_candidates(Some("sibling-model"));
        assert_eq!(sibling[0].payload.credential_id.as_deref(), Some("sibling"));
        assert!(!sibling[0]
            .payload
            .extra_body
            .as_ref()
            .is_some_and(|extra| extra.contains_key("freebuffBusiness")));
    }
}
