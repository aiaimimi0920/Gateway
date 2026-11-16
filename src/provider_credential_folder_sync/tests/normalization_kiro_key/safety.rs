//! Exercise actual DB merge, export materialization and both sides of the product boundary.
use super::*;
use crate::routing::candidate::deserialize_provider_payload;

#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_key_folder_real_merge_keeps_only_the_new_key_and_exact_model_metadata() {
    let mut account = provider();
    account.payload["apiKey"] = json!("previous-private-key");
    let raw = json!({"KIRO_API_KEY":"synthetic-new-key", "account_name":"Preserved",
        "defaultModel":"Exact.Model/Version", "supported_models":["Exact.Model/Version"],
        "rawSource":{"refreshToken":"discarded-private-archive"}});
    let payload = normalize_import_payload(key::PRESET_ID, &account, raw, None).unwrap();
    assert_eq!(payload["apiKey"], "synthetic-new-key");
    assert!(payload.get("api_key").is_none());
    assert_eq!(payload["supportedModels"], json!(["Exact.Model/Version"]));
    let merged = db::provider_credentials::merge_provider_account_and_credential_payloads(
        &account.payload,
        &payload,
    );
    let parsed = deserialize_provider_payload(&merged, Some(&account.adapter)).unwrap();
    assert_eq!(parsed.api_key, "synthetic-new-key");
    assert_eq!(parsed.default_model.as_deref(), Some("Exact.Model/Version"));
    assert_eq!(parsed.account_name.as_deref(), Some("Preserved"));
    key::validate(&parsed, &account.adapter).unwrap();
    crate::routing::call_flow::prepare_owned(parsed).unwrap();
    let exported = materialize_export_payload(&account, &payload).unwrap();
    assert_eq!(exported, payload);
    assert_eq!(
        normalize_import_payload(key::PRESET_ID, &account, exported, None).unwrap(),
        payload
    );
    let serialized = serde_json::to_string(&payload).unwrap();
    assert!(!serialized.contains("previous-private-key"));
    assert!(!serialized.contains("discarded-private-archive"));
}

#[test]
fn kiro_key_folder_rejects_empty_malformed_non_atomic_or_oversized_keys() {
    for value in [
        json!(""),
        json!("has space"),
        json!("line\nbreak"),
        json!("non-ascii-密钥"),
        json!("x".repeat(32_769)),
        Value::Null,
        json!(["private-key"]),
    ] {
        assert!(normalize(json!({"api_key":value})).is_err());
    }
    assert!(normalize(json!({"accessToken":"private-oauth-token"})).is_err());
    assert!(normalize(json!({"extraBody":{"apiKey":"private-nested-key"}})).is_err());
    for hint in ["bearer_token", "session_auth", "browser_state"] {
        assert!(normalize_import_payload(
            key::PRESET_ID,
            &provider(),
            json!({"apiKey":"synthetic-key"}),
            Some(hint)
        )
        .is_err());
    }
}

#[test]
fn kiro_key_folder_rejects_raw_target_flow_header_session_and_unknown_overrides() {
    for change in [
        json!({"baseUrl":"https://foreign.invalid"}),
        json!({"baseUrl":"https://runtime.eu-central-1.kiro.dev"}),
        json!({"auth_mode":"bearer"}),
        json!({"responsesPath":"/generateAssistantResponse"}),
        json!({"adapter":"openai_compatible"}),
        json!({"serviceProviderKey":"xai_platform"}),
        json!({"headers":{"Authorization":"private-header"}}),
        json!({"callFlow":{"special_adapter":"none"}}),
        json!({"call_flow":{"fingerprint_adapter":"codex"}}),
        json!({"executionMode":"browser_context_fetch"}),
        json!({"discovery":{}}),
        json!({"kiro_oauth":null}),
        json!({"unknown":true}),
        json!({"supportedModels":[false]}),
        json!({"defaultModel":" leading-space"}),
    ] {
        let mut raw = json!({"apiKey":"synthetic-new-key"});
        raw.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize(raw).is_err(), "accepted {change}");
    }
}

#[test]
fn kiro_key_folder_rejects_inherited_registration_auth_aliases_and_conflicting_configuration() {
    for change in [
        json!({"refresh_token":"private-rt"}),
        json!({"kiro_oauth":{"client_secret":"private-client"}}),
        json!({"extraBody":{"kiroMachineId":"a".repeat(64)}}),
        json!({"headers":{"Cookie":"private-session"}}),
        json!({"sessionAuth":{"transport":"bearer"}}),
        json!({"auth_mode":"bearer"}),
        json!({"apiKey":"","api_key":"private-ambiguous"}),
        json!({"base_url":key::BASE_URL}),
        json!({"responsesPath":"/generateAssistantResponse"}),
        json!({"callFlow":{"special_adapter":"grok_web"}}),
        json!({"refreshEndpoint":"https://foreign.invalid/token"}),
    ] {
        let mut account = provider();
        account
            .payload
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(normalize_import_payload(
            key::PRESET_ID,
            &account,
            json!({"apiKey":"synthetic-key"}),
            None
        )
        .is_err());
    }
}

#[test]
fn kiro_key_folder_export_rejects_cross_product_credential_overrides_and_false_markers() {
    let mut oauth = provider();
    oauth.payload = json!({"baseUrl":"https://q.us-east-1.amazonaws.com","responsesPath":"/generateAssistantResponse"});
    let mixed_key = json!({"api_key":"synthetic-native-key","baseUrl":key::BASE_URL,
        "responsesPath":"/","auth_mode":key::AUTH_MODE});
    let merged = db::provider_credentials::merge_provider_account_and_credential_payloads(
        &oauth.payload,
        &mixed_key,
    );
    let parsed = deserialize_provider_payload(&merged, Some(&oauth.adapter)).unwrap();
    assert!(key::owns_payload(&parsed));
    assert!(materialize_export_payload(&oauth, &mixed_key).is_err());
    for raw in [
        json!({"apiKey":"synthetic-key","baseUrl":"https://q.us-east-1.amazonaws.com","auth_mode":"bearer"}),
        json!({"apiKey":"synthetic-key","providerSurfaceKey":"kiro-compatible"}),
        json!({"apiKey":"synthetic-key","credentialMaterialKind":"bearer_token"}),
        json!({"apiKey":"synthetic-key","serviceProviderKey":"grok_platform"}),
    ] {
        assert!(materialize_export_payload(&provider(), &raw).is_err());
    }
    // Unrelated export semantics and the legacy OAuth surface are not rewritten.
    let old = materialize_export_payload(&oauth, &json!({"api_key":"legacy-access"})).unwrap();
    assert_eq!(old["providerSurfaceKey"], "kiro-compatible");
    assert_eq!(old["credentialMaterialKind"], "bearer_token");
}

#[test]
fn kiro_key_folder_lightweight_metadata_uses_inline_base_and_never_guesses_missing_product() {
    let account = provider();
    let mut metadata = db::ProviderAccountFolderSyncMetadata {
        id: account.id,
        label: account.label,
        service_provider_key: account.service_provider_key,
        service_provider_label: account.service_provider_label,
        adapter: account.adapter,
        protocol_family: account.protocol_family,
        protocol_profile: account.protocol_profile,
        source_kind: None,
        web_reverse_access_mode: None,
        payload_base_url: Some(key::BASE_URL.into()),
    };
    assert_eq!(derive_provider_family_slug(&metadata), key::PRESET_ID);
    assert_eq!(derive_provider_surface_slug(&metadata), key::PRESET_ID);
    metadata.payload_base_url = None;
    let maps = build_provider_maps(&[metadata]);
    let descriptor =
        resolve_import_path_descriptor("kiro-platform/kiro-api-key/api-key/key.json", &json!({}))
            .unwrap();
    assert!(select_provider_account_for_path(&maps, &descriptor).is_none());
}

#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[test]
fn kiro_key_folder_disabled_transport_is_not_enabled_by_import_or_export() {
    let raw = json!({"api_key":"synthetic-native-key"});
    for result in [
        normalize(raw.clone()),
        materialize_export_payload(&provider(), &raw),
    ] {
        assert_eq!(
            result.unwrap_err().code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
    }
}

#[test]
fn kiro_key_folder_explicit_native_fields_and_one_level_material_claims_cannot_enter_other_pools() {
    let generic = build_test_provider_account(
        "Generic",
        "openai_compatible",
        "openai",
        "openai",
        "https://example.invalid",
        None,
        None,
    );
    assert!(normalize_import_payload(
        "openai",
        &generic,
        json!({"KIRO_API_KEY":"synthetic-native-key"}),
        None
    )
    .is_err());
    let mut oauth = provider();
    oauth.payload = json!({"baseUrl":"https://q.us-east-1.amazonaws.com"});
    for container in ["extraBody", "extra_body", "EXTRA-BODY"] {
        for claim in [
            json!({"KIRO_API_KEY":"synthetic-native-key"}),
            json!({"auth_mode":key::AUTH_MODE}),
            json!({"baseUrl":key::BASE_URL}),
            json!({"providerSurfaceKey":key::PRESET_ID}),
            json!({"preset":key::PRESET_ID}),
        ] {
            let raw = json!({"apiKey":"synthetic-legacy-access",container:claim});
            assert!(
                normalize_import_payload("kiro-compatible", &oauth, raw.clone(), None).is_err()
            );
            assert!(normalize_import_payload("openai", &generic, raw.clone(), None).is_err());
            assert!(materialize_export_payload(&oauth, &raw).is_err());
        }
    }
}

#[test]
fn kiro_key_folder_does_not_guess_opaque_bearer_identity_or_scan_archival_raw_source() {
    let mut oauth = provider();
    oauth.payload = json!({"baseUrl":"https://q.us-east-1.amazonaws.com"});
    let raw = json!({"apiKey":"ksk_synthetic-opaque-bearer", "extraBody":{"kiroVersion":"0.7.1"},
        "rawSource":{"KIRO_API_KEY":"discarded-private-archive","auth_mode":key::AUTH_MODE}});
    let result = normalize_import_payload("kiro-compatible", &oauth, raw, None).unwrap();
    assert_eq!(result["api_key"], "ksk_synthetic-opaque-bearer");
    assert_eq!(result["credentialMaterialKind"], "bearer_token");
    assert!(result.get("rawSource").is_none());
}
