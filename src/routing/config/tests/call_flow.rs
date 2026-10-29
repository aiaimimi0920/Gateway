//! Additive config, credential precedence and request-local discovery policy.
use super::*;
use crate::protocol::{
    call_flow::{CallAdapter, FingerprintAdapter, SpecialAdapter},
    request_profile::RequestClientProfile,
    responses::normalize_responses,
};

fn document(flow: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{
        "id":"flow-fixture", "adapter":"openai_compatible", "base_url":"https://fixture.test",
        "api_key":"provider-secret", "call_flow":flow,
        "credentials":[{"id":"flow-credential", "api_key":"credential-secret", "request_client_profile":"codex_cli"}]
    }]})).unwrap()
}

#[test]
fn call_flow_compiles_without_losing_independent_policy_or_credential_overrides() {
    let cfg = document(
        json!({"call_adapter":"anthropic", "fingerprint_adapter":"claude_cli", "special_adapter":"none"}),
    );
    let serialized = serde_json::to_value(&cfg).unwrap();
    assert_eq!(
        serialized["providers"][0]["call_flow"]["call_adapter"],
        "anthropic"
    );
    let provider = compile_provider(cfg.providers[0].clone()).unwrap();
    assert_eq!(provider.payload.adapter, "anthropic_compatible");
    assert_eq!(
        provider.payload.request_client_profile,
        Some(RequestClientProfile::ClaudeCli)
    );
    let credential = &provider.credential_pool[0].payload;
    assert_eq!(credential.api_key, "credential-secret");
    assert_eq!(
        credential.request_client_profile,
        Some(RequestClientProfile::CodexCli)
    );
    let flow = credential.call_flow.unwrap();
    assert_eq!(flow.call_adapter, CallAdapter::Anthropic);
    assert_eq!(flow.fingerprint_adapter, FingerprintAdapter::CodexCli);
    assert_eq!(flow.special_adapter, SpecialAdapter::None);
}

#[test]
fn call_flow_missing_in_legacy_documents_stays_absent() {
    let mut cfg = document(Value::Null);
    cfg.providers[0].request_client_profile = Some(RequestClientProfile::ClaudeCli);
    let provider = compile_provider(cfg.providers[0].clone()).unwrap();
    assert!(provider.payload.call_flow.is_none());
    assert_eq!(
        provider.payload.request_client_profile,
        Some(RequestClientProfile::ClaudeCli)
    );
    assert!(serde_json::to_value(&cfg).unwrap()["providers"][0]
        .get("call_flow")
        .is_none());
}

#[test]
fn call_flow_rejects_incompatible_executor_before_publishing_configuration() {
    let cfg = document(json!({"special_adapter":"gemini_web"}));
    assert!(compile_provider(cfg.providers[0].clone()).is_err());
}

fn candidate(mode: &str) -> RouteCandidate {
    let store = make_store_from_yaml(&format!(
        r#"
providers:
  - id: flow-fixture
    adapter: openai_compatible
    base_url: https://fixture.test
    call_flow:
      call_adapter: {mode}
      special_adapter: none
"#
    ));
    let mut candidate = store.resolve_candidates(Some("fixture-model")).remove(0);
    candidate.supported_protocol_families.clear();
    candidate.payload.discovered_protocols = serde_json::from_value(json!([
        {"protocol":"responses","api_base":"https://fixture.test/v1","verified_models":["fixture-model"]},
        {"protocol":"messages","api_base":"https://fixture.test/anthropic/v1","verified_models":["fixture-model"]}
    ])).unwrap();
    candidate
}

#[test]
fn call_flow_fixed_format_survives_protocol_discovery_and_debug_override() {
    let req = normalize_responses(json!({"model":"fixture-model","input":"hello"})).unwrap();
    let mut c = candidate("anthropic");
    assert!(crate::provider_discovery::select_protocol_with_override(
        &mut c,
        &req,
        Some("openai_responses")
    )
    .is_none());
    assert!(crate::provider_discovery::select_protocol(&mut c, &req).is_some());
    assert_eq!(c.payload.adapter, "anthropic_compatible");
    assert!(c.payload.base_url.ends_with("/anthropic/v1"));
    assert_eq!(c.payload.discovered_protocols.len(), 2);
}

#[test]
fn call_flow_auto_uses_existing_discovery_and_preserves_negative_model_evidence() {
    let req = normalize_responses(json!({"model":"fixture-model","input":"hello"})).unwrap();
    let mut c = candidate("auto");
    assert!(crate::provider_discovery::select_protocol(&mut c, &req).is_some());
    assert_eq!(c.payload.adapter, "openai_compatible");
    let mut c = candidate("anthropic");
    c.payload.discovered_protocols[1]
        .failed_models
        .push("fixture-model".into());
    assert!(crate::provider_discovery::select_protocol(&mut c, &req).is_none());
    assert_eq!(
        c.payload.discovered_protocols[1].failed_models,
        ["fixture-model"]
    );
}

#[cfg(feature = "line-chatgpt-codex-oauth-official")]
#[test]
fn call_flow_basispoints_channel_preserves_credential_disable_override() {
    let cfg: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
        "id":"flow-fixture", "preset":"chatgpt-codex-oauth-official-api",
        "base_url":"https://chatgpt.com/backend-api/codex",
        "call_flow":{"call_adapter":"auto", "fingerprint_adapter":"session", "special_adapter":"basispoints"},
        "credentials":[{"id":"native", "api_key":"native-secret", "codex_basispoints_enabled":false},
            {"id":"alternate", "api_key":"alternate-secret"}]
    }]})).unwrap();
    let p = compile_provider(cfg.providers[0].clone()).unwrap();
    assert_eq!(
        p.payload.adapter,
        crate::protocol::chatgpt::basispoints::ADAPTER
    );
    assert_eq!(p.credential_pool[0].payload.adapter, "openai_compatible");
    assert_eq!(
        p.credential_pool[0]
            .payload
            .call_flow
            .unwrap()
            .special_adapter,
        SpecialAdapter::None
    );
    assert_eq!(
        p.credential_pool[1].payload.adapter,
        crate::protocol::chatgpt::basispoints::ADAPTER
    );
}
