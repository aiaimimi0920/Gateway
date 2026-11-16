//! 固定目标、独立身份和三入口实际 Chat wire 的本地配置合同。
use super::*;
use crate::{
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
fn document() -> Value {
    json!({"providers":[{"id":"arbitrary","preset":PRESET,"base_url":BASE,
        "explicit_credentials_only":true,"credentials":[{"id":"stable","api_key":"synthetic-sensenova",
        "supported_models":["sensenova-6.8-flash-lite"]}]}]})
}
fn compile(doc: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value::<RouteConfigYaml>(doc)?)
}
fn payload() -> ProviderAccountPayload {
    compile(document())
        .unwrap()
        .snapshot()
        .select_credential_probe_target("stable")
        .unwrap()
        .payload
}
#[test]
fn sensenova_has_own_implementation_and_no_default_model() {
    let p = payload();
    let line = crate::implementation_lines::line_for_payload(&p).unwrap();
    assert_eq!(line.canonical_protocol_profile(), PROFILE);
    assert_eq!(line.feature_name(), "line-sensenova-token-plan");
    assert!(p.default_model.is_none());
    let mut doc = document();
    doc["providers"][0]["credentials"] = json!([]);
    assert!(compile(doc).is_ok());
}
#[test]
fn sensenova_rejects_target_identity_and_transport_overrides() {
    for (field, value) in [
        (
            "base_url",
            json!("https://token.sensenova.cn.evil.invalid/v1"),
        ),
        ("auth_mode", json!("bearer")),
        ("headers", json!({"cookie":"foreign"})),
        ("extra_body", json!({"model":"other"})),
        ("responses_path", json!("/responses")),
        ("chat_completions_path", json!("/elsewhere")),
        ("call_flow", json!({"call_adapter":"anthropic"})),
        ("request_client_profile", json!("codex")),
    ] {
        let mut doc = document();
        doc["providers"][0][field] = value;
        assert!(compile(doc).is_err(), "{field}");
    }
    for (field, value) in [
        ("base_url", json!("https://platform.sensenova.cn")),
        ("auth_mode", json!("bearer")),
        ("refresh_token", json!("synthetic")),
        ("api_key", json!("bad\r\nkey")),
        ("headers", json!({"authorization":"foreign"})),
    ] {
        let mut doc = document();
        doc["providers"][0]["credentials"][0][field] = value;
        assert!(compile(doc).is_err(), "credential {field}");
    }
}
#[test]
fn sensenova_plans_public_text_as_chat_and_binds_final_identity() {
    let p = payload();
    for req in [
        crate::protocol::openai::normalize_chat_completions(json!({"model":"gpt-alias","messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::anthropic::normalize_messages(json!({"model":"claude-alias","max_tokens":32,"messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::responses::normalize_responses(json!({"model":"gpt-alias","input":"hello"})).unwrap(),
    ] {
        assert!(!UpstreamClient::uses_native_codex(&p,&req,"gpt-alias"));
        assert!(!UpstreamClient::uses_native_claude(&p,&req,"claude-alias"));
        let mut plan = UpstreamClient::build_request_plan(&p,&req,"sensenova-6.8-flash-lite",true).unwrap();
        assert_eq!(plan.url,format!("{BASE}/chat/completions"));
        assert_eq!(plan.response_kind,EndpointKind::ChatCompletions);
        assert!(plan.body.as_ref().unwrap()["messages"].is_array());
        let mut headers = rquest::header::HeaderMap::new();
        for name in ["cookie","authorization","x-api-key","originator"] { headers.insert(name,"foreign".parse().unwrap()); }
        finalize_headers(&p,&plan,&mut headers).unwrap();
        assert_eq!(headers.len(),4);
        assert_eq!(headers["authorization"],"Bearer synthetic-sensenova");
        plan.url.push_str("?secret=x"); assert!(validate_plan(&p,&plan).is_err());
    }
}
