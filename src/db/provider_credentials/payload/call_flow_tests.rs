//! Credential policy is merged before provenance is lost; no database or upstream I/O.
use super::*;
use crate::protocol::request_profile::RequestClientProfile;
use crate::routing::{call_flow::prepare_owned, candidate::deserialize_provider_payload};
use serde_json::json;

fn account() -> Value {
    json!({"adapter":"openai_compatible","base_url":"https://fixture.test",
        "api_key":"provider-fixture","request_client_profile":"workbuddy",
        "call_flow":{"call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"}})
}

#[test]
fn call_flow_credential_profile_overrides_only_inherited_fingerprint() {
    let account = account();
    let before = account.clone();
    for key in ["request_client_profile", "requestClientProfile"] {
        for (value, expected) in [
            ("auto", RequestClientProfile::Auto),
            ("codex_cli", RequestClientProfile::CodexCli),
        ] {
            let mut credential = json!({"api_key":"chosen-fixture"});
            credential[key] = json!(value);
            let merged = merge_provider_account_and_credential_payloads(&account, &credential);
            let payload =
                prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
            assert_eq!(
                payload.request_client_profile,
                Some(expected),
                "{key}={value}"
            );
            assert_eq!(
                payload.call_flow.unwrap().fingerprint_adapter.profile(),
                Some(expected)
            );
            assert_eq!(payload.adapter, "anthropic_compatible");
            assert_eq!(payload.api_key, "chosen-fixture");
        }
    }
    assert_eq!(account, before);
}

#[test]
fn call_flow_mixed_policy_aliases_merge_as_one_field() {
    for (account_key, credential_key) in [("call_flow", "callFlow"), ("callFlow", "call_flow")] {
        for (account_profile, credential_profile) in [
            ("request_client_profile", "requestClientProfile"),
            ("requestClientProfile", "request_client_profile"),
        ] {
            let mut account = account();
            let flow = account
                .as_object_mut()
                .unwrap()
                .remove("call_flow")
                .unwrap();
            account[account_key] = flow;
            account
                .as_object_mut()
                .unwrap()
                .remove("request_client_profile");
            account[account_profile] = json!("workbuddy");
            let mut credential = json!({"api_key":"chosen-fixture"});
            credential[credential_key] = json!({"call_adapter":"openai"});
            credential[credential_profile] = json!("codex_cli");
            let merged = merge_provider_account_and_credential_payloads(&account, &credential);
            let payload =
                prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
            assert_eq!(payload.adapter, "openai_compatible");
            assert_eq!(
                payload.request_client_profile,
                Some(RequestClientProfile::CodexCli)
            );
            assert_eq!(merged["call_flow"]["special_adapter"], "none");
        }
    }
}

#[test]
fn call_flow_explicit_credential_fingerprint_beats_same_layer_legacy_projection() {
    let merged = merge_provider_account_and_credential_payloads(
        &account(),
        &json!({
            "request_client_profile":"codex_cli",
            "call_flow":{"fingerprint_adapter":"auto"}
        }),
    );
    let payload = prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
    assert_eq!(
        payload.request_client_profile,
        Some(RequestClientProfile::Auto)
    );
    assert_eq!(payload.adapter, "anthropic_compatible");
}

#[test]
fn call_flow_merge_keeps_provider_flow_authoritative_without_credential_override() {
    let merged = merge_provider_account_and_credential_payloads(
        &account(),
        &json!({"api_key":"chosen-fixture"}),
    );
    let payload = prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
    assert_eq!(
        payload.request_client_profile,
        Some(RequestClientProfile::ClaudeCli)
    );
    assert_eq!(payload.api_key, "chosen-fixture");
}

#[test]
fn call_flow_merge_preserves_no_flow_and_explicit_null_contracts() {
    let mut legacy = account();
    legacy.as_object_mut().unwrap().remove("call_flow");
    let credential = json!({"request_client_profile":"codex_cli"});
    let merged = merge_provider_account_and_credential_payloads(&legacy, &credential);
    let payload = prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
    assert!(payload.call_flow.is_none());
    assert_eq!(
        payload.request_client_profile,
        Some(RequestClientProfile::CodexCli)
    );
    for credential in [
        json!({"call_flow":null}),
        json!({"request_client_profile":null}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account(), &credential);
        let payload = prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap();
        assert_eq!(
            payload.call_flow.is_none(),
            credential.get("call_flow").is_some()
        );
    }
}

#[test]
fn call_flow_merge_does_not_hide_invalid_policy_or_same_layer_alias_duplicates() {
    for credential in [
        json!({"call_flow":{"fingerprint_adaptor":"auto"},"request_client_profile":"codex_cli"}),
        json!({"call_flow":{"fingerprint_adapter":null},"request_client_profile":"codex_cli"}),
        json!({"call_flow":{"fingerprint_adapter":"unknown"},"request_client_profile":"codex_cli"}),
        json!({"call_flow":{"special_adapter":"unknown"}}),
        json!({"call_flow":"invalid","request_client_profile":"codex_cli"}),
        json!({"request_client_profile":"unknown"}),
        json!({"call_flow":{},"callFlow":{}}),
        json!({"request_client_profile":"auto","requestClientProfile":"codex_cli"}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&account(), &credential);
        assert!(
            deserialize_provider_payload(&merged, None).is_err(),
            "{credential}"
        );
    }
    for invalid in [Value::Null, json!("unknown")] {
        let mut account = account();
        account["call_flow"]["fingerprint_adapter"] = invalid;
        let merged = merge_provider_account_and_credential_payloads(
            &account,
            &json!({"request_client_profile":"codex_cli"}),
        );
        assert!(deserialize_provider_payload(&merged, None).is_err());
    }
}

#[test]
fn call_flow_session_owner_rejects_inherited_credential_api_profile() {
    let account = json!({"adapter":"gemini_web_compatible","base_url":"https://fixture.test",
        "api_key":"provider-fixture","call_flow":{"special_adapter":"gemini_web","fingerprint_adapter":"session"}});
    let merged = merge_provider_account_and_credential_payloads(
        &account,
        &json!({"request_client_profile":"codex_cli"}),
    );
    let error = prepare_owned(deserialize_provider_payload(&merged, None).unwrap()).unwrap_err();
    assert!(error.message.starts_with("Invalid call_flow:"));
}
