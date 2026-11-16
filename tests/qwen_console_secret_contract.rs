//! Qwen workspace Key audience is independent from labels, models and quota assumptions.
#[path = "qwen_console_secret_contract/support.rs"]
mod support;
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use serde_json::json;
use support::*;

#[test]
fn qwen_keep_rejects_provider_product_destination_and_execution_changes() {
    for shared in [false, true] {
        for change in [
            json!({"base_url":OTHER}),
            json!({"base_url":"https://workspace-b.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"}),
            json!({"base_url":"https://workspace-a.eu-central-1.maas.aliyuncs.com/compatible-mode/v1"}),
            json!({"base_url":"https://coding.dashscope.aliyuncs.com/v1"}),
            json!({"base_url":"https://chat.qwen.ai"}),
            json!({"preset":"qwen-coding-plan-openai"}),
            json!({"auth_mode":"bearer"}),
            json!({"adapter":"anthropic"}),
            json!({"protocol_profile":"openai"}),
            json!({"protocol_family":"anthropic"}),
            json!({"call_flow":{"call_adapter":"openai", "fingerprint_adapter":"auto", "special_adapter":"none"}}),
            json!({"chat_completions_path":"/other"}),
            json!({"session_auth":{"transport":"bearer"}}),
            json!({"keepalive":{"serviceUrl":"https://untrusted.invalid"}}),
            json!({"runtime_state_object_key":"other-state"}),
            json!({"execution_mode":"browser_backed"}),
        ] {
            let active = fixture(shared);
            let (mut draft, keeps) = redacted(&active);
            for (key, value) in change.as_object().unwrap() {
                draft["providers"][0][key] = value.clone();
            }
            reject(&active, draft, &keeps);
        }
    }
}

#[test]
fn qwen_keep_binds_effective_credential_overrides_and_refresh_targets() {
    for shared in [false, true] {
        for change in [
            json!({"base_url":OTHER}),
            json!({"auth_mode":"bearer"}),
            json!({"request_client_profile":"codex_cli"}),
            json!({"codex_basispoints_enabled":true}),
            json!({"refresh_endpoint":"https://untrusted.invalid/token"}),
            json!({"refresh_client_id":"different-client"}),
            json!({"session_auth":{"transport":"bearer"}}),
            json!({"keepalive":{"serviceUrl":"https://untrusted.invalid"}}),
            json!({"runtime_state_object_key":"different-state"}),
        ] {
            let active = fixture(shared);
            let (mut draft, keeps) = redacted(&active);
            for (key, value) in change.as_object().unwrap() {
                draft["providers"][0]["credentials"][0][key] = value.clone();
            }
            reject(&active, draft, &keeps);
        }
    }
}

#[test]
fn qwen_identity_is_detected_by_preset_marker_or_effective_workspace_not_provider_id() {
    for signal in ["preset", "marker", "base"] {
        let mut active = fixture(false);
        let provider = &mut active.providers[0];
        provider.preset = (signal == "preset").then(|| "qwen-official-api".into());
        provider.auth_mode = (signal == "marker").then(|| "qwen_workspace_api".into());
        if signal != "base" {
            provider.base_url = "https://legacy.example.test/v1".into();
        }
        let (mut draft, keeps) = redacted(&active);
        draft["providers"][0]["base_url"] = json!(OTHER);
        reject(&active, draft, &keeps);

        // Entry from an unprotected product is checked too, not only departure from Qwen.
        let mut legacy = fixture(false);
        legacy.providers[0].preset = Some("qwen-coding-plan-openai".into());
        legacy.providers[0].auth_mode = Some("bearer".into());
        legacy.providers[0].base_url = "https://coding.dashscope.aliyuncs.com/v1".into();
        let (mut draft, keeps) = redacted(&legacy);
        match signal {
            "preset" => draft["providers"][0]["preset"] = json!("qwen-official-api"),
            "marker" => {
                draft["providers"][0]["credentials"][0]["auth_mode"] = json!("qwen_workspace_api")
            }
            _ => draft["providers"][0]["credentials"][0]["base_url"] = json!(BASE),
        }
        reject(&legacy, draft, &keeps);
    }
    let active = fixture(false);
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["preset"] = json!("qwen-web-reverse");
    draft["providers"][0]["auth_mode"] = json!("bearer");
    draft["providers"][0]["base_url"] = json!("https://chat.qwen.ai");
    reject(&active, draft, &keeps);
}

#[test]
fn qwen_keep_allows_metadata_stable_reordering_and_yaml_reload() {
    let mut active = fixture(false);
    let mut second = active.providers[0].credentials[0].clone();
    second.id = Some("b".into());
    second.api_key = Some("synthetic-second-key".into());
    active.providers[0].credentials.push(second);
    active.providers.push(
        document(json!({"providers":[{"id":"other", "base_url":"https://example.test"}]}))
            .providers
            .remove(0),
    );
    let (mut draft, _) = redacted(&active);
    draft["providers"][0]["label"] = json!("任意展示名称");
    draft["providers"][0]["model_map"] = json!({"public-model":"fixture-model"});
    draft["providers"][0]["credentials"][0]["account_name"] = json!("new name");
    draft["providers"][0]["credentials"][0]["supported_models"] = json!(["fixture-model"]);
    draft["providers"][0]["credentials"][0]["enabled"] = json!(false);
    draft["providers"][0]["credentials"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    draft["providers"].as_array_mut().unwrap().swap(0, 1);
    let keeps = [0, 1].map(|i| SecretPatch::keep(format!("/providers/1/credentials/{i}/api_key")));
    let resolved = resolve_secret_patches(&active, document(draft), &keeps).unwrap();
    let saved = serde_yaml::to_string(&resolved).unwrap();
    let reloaded = serde_yaml::from_str(&saved).unwrap();
    let public = redact_route_document(&reloaded).unwrap();
    assert_eq!(
        resolved.providers[1].credentials[0].api_key.as_deref(),
        Some("synthetic-second-key")
    );
    assert_eq!(
        resolved.providers[1].credentials[1].api_key.as_deref(),
        Some("synthetic-account-key")
    );
    assert!(!serde_json::to_string(&public.document)
        .unwrap()
        .contains("synthetic-"));
}

#[test]
fn qwen_explicit_replace_or_clear_can_change_audience_without_mutating_active() {
    for shared in [false, true] {
        for clear in [false, true] {
            let active = fixture(shared);
            let before = serde_json::to_value(&active).unwrap();
            let (mut draft, _) = redacted(&active);
            draft["providers"][0]["base_url"] = json!(OTHER);
            let path = if shared {
                "/providers/0/api_key"
            } else {
                "/providers/0/credentials/0/api_key"
            };
            let patch = if clear {
                SecretPatch::clear(path)
            } else {
                SecretPatch::replace(path, json!("synthetic-new-key"))
            };
            let resolved = resolve_secret_patches(&active, document(draft), &[patch]).unwrap();
            assert_eq!(resolved.providers[0].base_url, OTHER);
            assert_eq!(serde_json::to_value(&active).unwrap(), before);
        }
    }
}
