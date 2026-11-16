//! 合成 Key 检查区域、目标、协议和最终发送身份，不访问真实上游。
use super::*;
use crate::{
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
fn document(china: bool) -> Value {
    json!({"providers":[{"id":"arbitrary","preset":if china { PRESET_CN } else { PRESET },
        "base_url":if china { BASE_CN } else { BASE },"explicit_credentials_only":true,
        "credentials":[{"id":"stable","api_key":"synthetic-minimax","supported_models":["MiniMax-M3.1-Flash"]}]}]})
}
fn compile(doc: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value::<RouteConfigYaml>(doc)?)
}
fn payload(china: bool) -> ProviderAccountPayload {
    compile(document(china))
        .unwrap()
        .snapshot()
        .select_credential_probe_target("stable")
        .unwrap()
        .payload
}
#[test]
fn minimax_regions_have_one_implementation_and_no_default_model() {
    for china in [false, true] {
        let p = payload(china);
        assert!(owns_payload(&p));
        assert_eq!(
            crate::implementation_lines::line_for_payload(&p)
                .unwrap()
                .canonical_protocol_profile(),
            "minimax_m_plan"
        );
        assert!(p.default_model.is_none());
        assert_eq!(
            crate::implementation_lines::line_for_payload(&p)
                .unwrap()
                .feature_name(),
            "line-minimax-m-plan"
        );
        let mut doc = document(china);
        doc["providers"][0]["credentials"] = json!([]);
        assert!(compile(doc).is_ok());
    }
}
#[test]
fn minimax_rejects_region_identity_and_transport_overrides() {
    for china in [false, true] {
        for (field, value) in [
            ("base_url", json!(if china { BASE } else { BASE_CN })),
            ("base_url", json!("https://api.minimaxi.com/anthropic")),
            ("auth_mode", json!("bearer")),
            ("headers", json!({"cookie":"foreign"})),
            ("extra_body", json!({"model":"other"})),
            ("messages_path", json!("/elsewhere")),
            ("call_flow", json!({"call_adapter":"openai"})),
            ("request_client_profile", json!("claude_code")),
        ] {
            let mut doc = document(china);
            doc["providers"][0][field] = value;
            assert!(compile(doc).is_err(), "{field}");
        }
        for (field, value) in [
            ("base_url", json!(if china { BASE } else { BASE_CN })),
            ("auth_mode", json!("bearer")),
            ("refresh_token", json!("synthetic")),
            ("api_key", json!("bad\r\nkey")),
            ("headers", json!({"authorization":"foreign"})),
        ] {
            let mut doc = document(china);
            doc["providers"][0]["credentials"][0][field] = value;
            assert!(compile(doc).is_err(), "credential {field}");
        }
    }
}
#[test]
fn minimax_messages_keep_product_sender_and_bound_identity() {
    for china in [false, true] {
        let p = payload(china);
        let req = crate::protocol::anthropic::normalize_messages(
            json!({"model":"claude-alias","stream":true,
            "max_tokens":32,"messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap();
        assert!(!UpstreamClient::uses_native_claude(
            &p,
            &req,
            "claude-alias"
        ));
        let plan =
            UpstreamClient::build_request_plan(&p, &req, "MiniMax-M3.1-Flash", true).unwrap();
        assert_eq!(
            plan.url,
            format!("{}/v1/messages", if china { BASE_CN } else { BASE })
        );
        let mut headers = rquest::header::HeaderMap::new();
        for name in ["cookie", "authorization", "x-api-key", "anthropic-beta"] {
            headers.insert(name, "foreign".parse().unwrap());
        }
        finalize_headers(&p, &plan, &mut headers).unwrap();
        assert_eq!(headers.len(), 4);
        assert_eq!(headers["authorization"], "Bearer synthetic-minimax");
        let mut bad =
            UpstreamClient::build_request_plan(&p, &req, "MiniMax-M3.1-Flash", true).unwrap();
        bad.url = format!("{BASE_CN}/v1/messages?key=x");
        assert!(validate_plan(&p, &bad).is_err());
        let mut bad =
            UpstreamClient::build_request_plan(&p, &req, "MiniMax-M3.1-Flash", true).unwrap();
        bad.body.as_mut().unwrap()["tool_choice"] = json!({"type":"any"});
        assert!(validate_plan(&p, &bad).is_err());
        let mut bad = p.clone();
        bad.api_key.clear();
        assert!(validate_plan(&bad, &plan).is_err());
    }
}
#[test]
fn minimax_preserves_native_thinking_history_and_effort() {
    let p = payload(false);
    let raw = json!({"model":"MiniMax-M3.1-Flash","max_tokens":128,"output_config":{"effort":"high"},
        "messages":[{"role":"user","content":"hello"},{"role":"assistant","content":[
            {"type":"thinking","thinking":"synthetic reasoning","signature":"synthetic-signature"},
            {"type":"tool_use","id":"call1","name":"lookup","input":{"q":"abc"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"call1","content":"found"}]}]});
    let req = crate::protocol::anthropic::normalize_messages(raw.clone()).unwrap();
    let plan = UpstreamClient::build_request_plan(&p, &req, "MiniMax-M3.1-Flash", false).unwrap();
    let body = plan.body.unwrap();
    assert_eq!(body["messages"], raw["messages"]);
    assert_eq!(body["output_config"], raw["output_config"]);
}
