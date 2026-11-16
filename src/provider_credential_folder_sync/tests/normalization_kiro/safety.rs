//! Negative material/target contracts and the actual merge→headers consumer.
use super::{legacy, normalize, provider};
use crate::db::provider_credentials::merge_provider_account_and_credential_payloads;
use crate::provider_credential_folder_sync::normalization::normalize_import_payload;
use crate::routing::candidate::deserialize_provider_payload;
use serde_json::{json, Value};

#[test]
fn kiro_import_rejects_partial_or_unknown_registration_without_defaults() {
    for field in [
        "kiroAuthMethod",
        "kiroAuthRegion",
        "startUrl",
        "kiroClientId",
        "kiroClientSecret",
        "clientSecretExpiresAt",
        "scopes",
    ] {
        let mut raw = legacy();
        raw["extraBody"].as_object_mut().unwrap().remove(field);
        assert!(normalize(raw).is_err(), "accepted missing {field}");
    }
    for change in [
        json!({"kiroAuthMethod":"social"}),
        json!({"kiroAuthRegion":"unknown-1"}),
        json!({"clientSecretExpiresAt":"1893456000"}),
        json!({"scopes":[]}),
        json!({"scopes":["sso:account:access"]}),
    ] {
        let mut raw = legacy();
        raw["extraBody"]
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize(raw).is_err());
    }
}

#[test]
fn kiro_import_accepts_equal_aliases_but_rejects_wrong_types_and_conflicts() {
    let mut equal = legacy();
    equal["api_key"] = json!("synthetic-access");
    equal["extra_body"] = equal["extraBody"].clone();
    assert!(normalize(equal).is_ok());
    for change in [
        json!({"access_token":false}),
        json!({"refresh_token":null}),
        json!({"extra_body":{}}),
        json!({"clientSecret":"another-private-secret"}),
        json!({"machineId":"not-a-machine"}),
    ] {
        let mut raw = legacy();
        raw.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize(raw).is_err());
    }
}

#[test]
fn kiro_import_preserves_explicit_machine_and_canonical_context_across_rotation() {
    let mut raw = legacy();
    raw["machineId"] = json!("ABCDEF01-2345-6789-ABCD-EF0123456789");
    let mut result = normalize(raw).unwrap();
    let machine = "abcdef0123456789abcdef0123456789".repeat(2);
    assert_eq!(result["kiro_oauth"]["machine_id"], machine);
    result["refresh_token"] = json!("synthetic-rotated");
    let rotated = normalize(result.clone()).unwrap();
    assert_eq!(rotated, result);
    let mut conflict = rotated;
    conflict["extra_body"]["machineId"] = json!("b".repeat(64));
    assert!(normalize(conflict).is_err());
}

#[test]
fn kiro_import_freezes_old_zero_machine_when_only_root_rt_existed() {
    let mut raw = legacy();
    raw["extraBody"]
        .as_object_mut()
        .unwrap()
        .remove("kiroRefreshToken");
    assert_eq!(
        normalize(raw).unwrap()["kiro_oauth"]["machine_id"],
        "0".repeat(64)
    );
}

#[test]
fn kiro_import_does_not_invent_expiry_or_retain_nested_auth_archives() {
    let mut raw = legacy();
    raw["rawSource"] = json!({"refreshToken":"unused-private-archive"});
    let result = normalize(raw).unwrap();
    assert!(result.get("expires_at").is_none());
    assert_eq!(result["session_auth"]["expiresAt"], Value::Null);
    assert!(result.get("rawSource").is_none());
    for name in ["rawSource", "kiro_oauth", "grok_oauth", "accio_sgcookie"] {
        let mut raw = legacy();
        raw["extraBody"][name] = json!({"client_secret":"private-copy"});
        assert!(normalize(raw).is_err(), "retained secret copy {name}");
    }
}

#[test]
fn kiro_import_keeps_absolute_expiry_and_rejects_contradictory_session_expiry() {
    let mut raw = legacy();
    raw["expiresAt"] = json!("2020-01-01T00:00:00Z");
    let result = normalize(raw.clone()).unwrap();
    assert_eq!(result["expires_at"], "2020-01-01T00:00:00Z");
    assert_eq!(result["session_auth"]["expiresAt"], result["expires_at"]);
    raw["sessionAuth"] = json!({"transport":"bearer", "expiresAt":"2099-01-01T00:00:00Z"});
    assert!(normalize(raw).is_err());
    let mut raw = legacy();
    raw["expiresAt"] = json!("not-an-expiry");
    assert!(normalize(raw).is_err());
}

#[test]
fn kiro_import_retains_bearer_only_without_manufacturing_registration() {
    let result =
        normalize(json!({"apiKey":"synthetic-access", "refresh_token":"root-only-rt"})).unwrap();
    assert!(result.get("kiro_oauth").is_none());
    assert_eq!(result["extra_body"]["kiroMachineId"], "0".repeat(64));
    assert!(normalize(
        json!({"apiKey":"synthetic-access", "refresh_endpoint":"https://foreign.invalid/token"})
    )
    .is_err());
}

#[test]
fn kiro_import_accepts_refresh_only_but_never_inherits_provider_access() {
    let mut raw = legacy();
    raw.as_object_mut().unwrap().remove("apiKey");
    assert_eq!(normalize(raw.clone()).unwrap()["api_key"], "");
    let mut account = provider();
    account.payload["apiKey"] = json!("other-account-access");
    assert!(normalize_import_payload("kiro", &account, raw, None).is_err());
}

#[test]
fn kiro_import_rejects_cross_product_and_untrusted_business_or_refresh_targets() {
    for change in [
        json!({"baseUrl":"https://foreign.invalid"}),
        json!({"adapter":"openai_compatible"}),
        json!({"serviceProviderKey":"grok_platform"}),
        json!({"providerSurfaceKey":"grok"}),
        json!({"credentialMaterialKind":"api_key"}),
        json!({"refreshEndpoint":"https://foreign.invalid/token"}),
        json!({"refreshClientId":"different-client"}),
        json!({"grok_oauth":{}}),
        json!({"sessionAuth":{"transport":"cookie"}}),
        json!({"call_flow":{"special_adapter":"grok_web"}}),
    ] {
        let mut raw = legacy();
        raw.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize(raw).is_err());
    }
    let mut account = provider();
    account.adapter = "openai_compatible".into();
    assert!(normalize_import_payload("kiro", &account, legacy(), None).is_err());
}

#[test]
fn kiro_import_rejects_inherited_auth_copies_and_account_alias_ambiguity() {
    for change in [
        json!({"headers":{"Authorization":"private-copy"}}),
        json!({"extraBody":{"refreshToken":"private-copy"}}),
        json!({"keepalive":{"serviceUrl":"https://foreign.invalid"}}),
        json!({"baseUrl":"https://foreign.invalid"}),
        json!({"base_url":"https://q.us-east-1.amazonaws.com"}),
    ] {
        let mut account = provider();
        account
            .payload
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize_import_payload("kiro", &account, legacy(), None).is_err());
    }
}

#[test]
fn kiro_import_real_merge_and_headers_use_only_new_access_and_fixed_machine() {
    let mut account = provider();
    account.payload["apiKey"] = json!("previous-provider-access");
    account.payload["extraBody"] = json!({"kiroVersion":"0.7.1"});
    account.payload["sessionAuth"] = json!({"transport":"bearer"});
    let result = normalize_import_payload("kiro-compatible", &account, legacy(), None).unwrap();
    let merged = merge_provider_account_and_credential_payloads(&account.payload, &result);
    let parsed = deserialize_provider_payload(&merged, Some("kiro_compatible")).unwrap();
    let headers = crate::upstream::headers::build_upstream_headers(&parsed);
    let machine = result["kiro_oauth"]["machine_id"].as_str().unwrap();
    assert_eq!(headers["authorization"], "Bearer synthetic-access");
    assert!(headers["user-agent"].to_str().unwrap().contains(machine));
    let serialized = serde_json::to_string(parsed.extra_body.as_ref().unwrap()).unwrap();
    assert!(!serialized.contains("synthetic-refresh"));
    assert!(!serialized.contains("synthetic-registered-secret"));
}

#[test]
fn kiro_import_preserves_inherited_explicit_machine_before_legacy_rt_hash() {
    for (name, original, expected) in [
        ("kiroMachineId", "a".repeat(64), "a".repeat(64)),
        ("machineId", "a".repeat(64), "a".repeat(64)),
        (
            "kiroMachineId",
            "ABCDEF01-2345-6789-ABCD-EF0123456789".into(),
            "abcdef0123456789abcdef0123456789".repeat(2),
        ),
        (
            "machineId",
            "ABCDEF01-2345-6789-ABCD-EF0123456789".into(),
            "abcdef0123456789abcdef0123456789".repeat(2),
        ),
    ] {
        let mut account = provider();
        account.payload["extraBody"] = json!({name:original});
        let result = normalize_import_payload("kiro", &account, legacy(), None).unwrap();
        assert_eq!(result["kiro_oauth"]["machine_id"], expected);
        let merged = merge_provider_account_and_credential_payloads(&account.payload, &result);
        let parsed = deserialize_provider_payload(&merged, Some("kiro_compatible")).unwrap();
        let headers = crate::upstream::headers::build_upstream_headers(&parsed);
        assert!(headers["user-agent"].to_str().unwrap().contains(&expected));
    }
}

#[test]
fn kiro_import_canonical_context_keeps_expired_registration_and_rejects_legacy_conflict() {
    let mut result = normalize(legacy()).unwrap();
    result["kiro_oauth"]["client_secret_expires_at"] = json!(1);
    let preserved = normalize(result).unwrap();
    assert_eq!(preserved["kiro_oauth"]["client_secret_expires_at"], 1);
    let mut conflict = preserved;
    conflict["clientSecret"] = json!("different-private-registration");
    assert!(normalize(conflict).is_err());
}

#[test]
fn kiro_import_never_activates_ignored_account_outer_aliases_as_device_identity() {
    for name in ["extra-body", "ExtraBody", "EXTRA_BODY"] {
        let mut account = provider();
        account.payload[name] = json!({"kiroMachineId":"a".repeat(64)});
        let result =
            normalize_import_payload("kiro", &account, json!({"apiKey":"synthetic-access"}), None)
                .unwrap();
        let merged = merge_provider_account_and_credential_payloads(&account.payload, &result);
        let parsed = deserialize_provider_payload(&merged, Some("kiro_compatible")).unwrap();
        let headers = crate::upstream::headers::build_upstream_headers(&parsed);
        assert!(headers["user-agent"]
            .to_str()
            .unwrap()
            .contains(&"0".repeat(64)));
        assert!(!headers["user-agent"]
            .to_str()
            .unwrap()
            .contains(&"a".repeat(64)));
    }
}
