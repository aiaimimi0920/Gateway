//! Synthetic ordinary Qwen configuration: no catalog, inference, refresh or live credentials.
use neuro_gateway::{
    preset::get_builtin_preset,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};

const BASE: &str = "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const OTHER: &str = "https://workspace-b.eu-central-1.maas.aliyuncs.com/compatible-mode/v1";
fn provider() -> Value {
    json!({"id":"arbitrary", "preset":"qwen-official-api", "base_url":BASE,
        "explicit_credentials_only":true, "credentials":[],
        "call_flow":{"call_adapter":"auto", "fingerprint_adapter":"auto", "special_adapter":"none"}})
}
fn compile(provider: Value) -> Result<RouteConfigStore, anyhow::Error> {
    let document: RouteConfigYaml =
        serde_json::from_value(json!({"providers":[provider], "model_routes":[], "aliases":{}}))
            .unwrap();
    RouteConfigStore::from_document(document)
}

#[test]
fn qwen_workspace_preset_is_gated_independently_from_retired_and_plan_names() {
    assert_eq!(
        get_builtin_preset("qwen-official-api").is_some(),
        cfg!(feature = "line-qwen-official-api")
    );
    assert!(get_builtin_preset("qwen").is_none());
    assert!(get_builtin_preset("qwen-dashscope-openai").is_none());
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_empty_pool_does_not_create_a_default_account_or_model_route() {
    for base in ["", BASE] {
        let mut value = provider();
        value["base_url"] = json!(base);
        let store = compile(value).unwrap();
        assert!(store.resolve_candidates(Some("fixture-model")).is_empty());
        assert!(store.account_group_inventory().accounts.is_empty());
        let providers = store.get_providers();
        assert_eq!(providers[0].protocol_profile, "qwen_official_api");
        assert_eq!(providers[0].payload.base_url, base);
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_final_accounts_can_override_an_unbound_parent_and_roundtrip() {
    let mut value = provider();
    value["base_url"] = json!("");
    value["credentials"] = json!([{"id":"a", "base_url":BASE, "api_key":"synthetic-a", "supported_models":["fixture-model"]},
        {"id":"b", "base_url":OTHER, "api_key":"synthetic-b", "enabled":false}]);
    let store = compile(value.clone()).unwrap();
    let providers = store.get_providers();
    assert_eq!(providers[0].credential_pool[0].payload.base_url, BASE);
    assert_eq!(providers[0].credential_pool[1].payload.base_url, OTHER);
    assert_eq!(
        providers[0].credential_pool[0].payload.api_key,
        "synthetic-a"
    );
    let yaml = serde_yaml::to_string(&json!({"providers":[value]})).unwrap();
    let reload: RouteConfigYaml = serde_yaml::from_str(&yaml).unwrap();
    assert!(RouteConfigStore::from_document(reload).is_ok());
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_default_account_requires_a_real_target_not_a_placeholder() {
    let mut value = provider();
    value["explicit_credentials_only"] = json!(false);
    value["api_key"] = json!("synthetic-default");
    assert!(compile(value.clone()).is_ok());
    value["base_url"] = json!("");
    assert!(compile(value).is_err());
    let mut value = provider();
    value["base_url"] = json!("");
    value["credentials"] = json!([{"id":"a", "api_key":"synthetic-a"}]);
    assert!(compile(value).is_err());
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_final_identity_cannot_be_lost_by_base_auth_or_adapter_overrides() {
    for enabled in [true, false] {
        for change in [
            json!({"base_url":"https://coding.dashscope.aliyuncs.com/v1", "auth_mode":"bearer"}),
            json!({"base_url":"https://evil.invalid/v1", "auth_mode":"bearer"}),
            json!({"auth_mode":"x-api-key"}),
            json!({"refresh_endpoint":"https://evil.invalid/token"}),
            json!({"refresh_token":"synthetic-refresh"}),
            json!({"refresh_client_id":"client"}),
            json!({"token_expires_in_secs":123}),
            json!({"discovery_job":{"id":"new", "status":"pending"}}),
            json!({"codex_basispoints_enabled":true}),
            json!({"keepalive":{"serviceUrl":"https://evil.invalid"}}),
        ] {
            let mut value = provider();
            let mut account = json!({"id":"a", "api_key":"synthetic-private", "enabled":enabled});
            for (key, item) in change.as_object().unwrap() {
                account[key] = item.clone();
            }
            value["credentials"] = json!([account]);
            assert!(compile(value).is_err(), "must reject {change}");
        }
    }
    for change in [
        json!({"adapter":"anthropic_compatible", "auth_mode":"bearer", "base_url":"https://evil.invalid"}),
        json!({"protocol_profile":"qwen_coding_plan_openai"}),
        json!({"protocol_family":"anthropic_messages"}),
        json!({"responses_path":"/responses"}),
        json!({"chat_completions_path":"/other"}),
        json!({"codex_basispoints_enabled":true}),
        json!({"call_flow":{"call_adapter":"anthropic", "fingerprint_adapter":"auto", "special_adapter":"none"}}),
        json!({"call_flow":{"call_adapter":"auto", "fingerprint_adapter":"auto", "special_adapter":"basispoints"}}),
    ] {
        let mut value = provider();
        for (key, item) in change.as_object().unwrap() {
            value[key] = item.clone();
        }
        value["credentials"] = json!([{"id":"a", "api_key":"synthetic-private"}]);
        assert!(compile(value).is_err(), "must reject {change}");
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_six_regions_plain_flow_no_flow_and_key_clear_keep_runtime_policy() {
    for region in [
        "cn-beijing",
        "ap-southeast-1",
        "ap-northeast-1",
        "eu-central-1",
        "us-east-1",
        "cn-hongkong",
    ] {
        for flow in [true, false] {
            let mut value = provider();
            value["base_url"] = json!(format!(
                "https://workspace.{region}.maas.aliyuncs.com/compatible-mode/v1"
            ));
            if !flow {
                value.as_object_mut().unwrap().remove("call_flow");
            }
            value["credentials"] = json!([{"id":"a", "api_key":"synthetic-a"}]);
            assert!(compile(value.clone()).is_ok());
            value["credentials"][0]["api_key"] = json!(null);
            let store = compile(value).unwrap();
            assert!(neuro_gateway::protocol::qwen::workspace_api::validate_key(
                &store.get_providers()[0].credential_pool[0].payload
            )
            .is_err());
        }
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_unbound_template_still_rejects_forbidden_declarations() {
    for change in [
        json!({"auth_mode":"x-api-key"}),
        json!({"auth_token":"synthetic-not-allowed"}),
        json!({"headers":{"Host":"evil.invalid"}}),
        json!({"headers":{"Cookie":"synthetic-not-allowed"}}),
        json!({"responses_path":"/responses"}),
        json!({"chat_completions_path":"/other"}),
        json!({"protocol_profile":"qwen_coding_plan_openai"}),
        json!({"codex_basispoints_enabled":true}),
        json!({"call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"none"}}),
    ] {
        let mut value = provider();
        value["base_url"] = json!("");
        for (key, item) in change.as_object().unwrap() {
            value[key] = item.clone();
        }
        assert!(compile(value).is_err(), "empty parent must reject {change}");
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_validates_key_format_but_keeps_software_fingerprint_independent() {
    for key in [
        "synthetic\nkey",
        "has a space",
        "非ASCII",
        &"a".repeat(16 * 1024 + 1),
    ] {
        let mut value = provider();
        value["credentials"] = json!([{"id":"a","api_key":key}]);
        assert!(
            compile(value).is_err(),
            "invalid Key format must be rejected"
        );
    }
    for profile in ["auto", "codex_cli", "claude_cli", "cpa"] {
        let mut value = provider();
        value["call_flow"]["fingerprint_adapter"] = json!(profile);
        value["credentials"] = json!([{"id":"a","api_key":"synthetic-a"}]);
        let store = compile(value).unwrap();
        let providers = store.get_providers();
        let account = &providers[0].credential_pool[0];
        assert_eq!(account.payload.base_url, BASE);
        assert_eq!(account.payload.adapter, "openai_compatible");
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_accepts_only_native_catalog_bound_to_the_final_account() {
    let mut value = provider();
    value["base_url"] = json!("");
    value["credentials"] = json!([{"id":"a","base_url":BASE,"api_key":"synthetic-a","model_catalog":{
        "sourceUrl":"https://workspace.cn-beijing.maas.aliyuncs.com/api/v1/models",
        "accountBaseUrl":BASE,"checkedAt":"2026-11-02T00:00:00Z","models":["fixture-model"]}}]);
    assert!(compile(value.clone()).is_ok());
    value["credentials"][0]["base_url"] = json!(OTHER);
    assert!(compile(value).is_err());
}

#[cfg(not(feature = "line-qwen-official-api"))]
#[test]
fn qwen_workspace_compiled_out_preset_cannot_create_a_generic_fallback() {
    assert!(compile(provider()).is_err());
}
