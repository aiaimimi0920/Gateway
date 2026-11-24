//! 固定目标、独立身份和公共入口实际 wire 的本地配置合同。
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
fn sensenova_plans_fixed_text_endpoints_and_binds_final_identity() {
    let p = payload();
    for req in [
        crate::protocol::openai::normalize_chat_completions(json!({"model":"gpt-alias","messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::anthropic::normalize_messages(json!({"model":"claude-alias","max_tokens":32,"messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::responses::normalize_responses(json!({"model":"gpt-alias","input":"hello"})).unwrap(),
    ] {
        assert!(!UpstreamClient::uses_native_codex(&p,&req,"gpt-alias"));
        assert!(!UpstreamClient::uses_native_claude(&p,&req,"claude-alias"));
        let mut plan = UpstreamClient::build_request_plan(&p,&req,"sensenova-6.8-flash-lite",true).unwrap();
        let (path, kind) = match req.endpoint_kind {
            EndpointKind::Messages => ("/messages", EndpointKind::Messages),
            EndpointKind::Responses => ("/responses", EndpointKind::Responses),
            _ => ("/chat/completions", EndpointKind::ChatCompletions),
        };
        assert_eq!(plan.url, format!("{BASE}{path}"));
        assert_eq!(plan.response_kind, kind);
        let wire = plan.body.as_ref().unwrap();
        if kind == EndpointKind::Responses {
            assert_eq!(wire["input"], "hello");
        } else {
            assert!(wire["messages"].is_array());
        }
        let mut headers = rquest::header::HeaderMap::new();
        for name in ["cookie","authorization","x-api-key","originator"] { headers.insert(name,"foreign".parse().unwrap()); }
        finalize_headers(&p,&plan,&mut headers).unwrap();
        assert_eq!(headers.len(),4);
        assert_eq!(headers["authorization"],"Bearer synthetic-sensenova");
        plan.url.push_str("?secret=x"); assert!(validate_plan(&p,&plan).is_err());
    }
}

#[test]
fn sensenova_native_messages_resolves_image_capability_and_policy_before_send() {
    let payload = payload();
    let mut req = crate::protocol::anthropic::normalize_messages(json!({
        "model":"caller-alias", "max_tokens":1024, "gateway_debug":true,
        "thinking":{"type":"enabled"},
        "messages":[{"role":"user","content":[{"type":"image","source":{
            "type":"url","url":"https://example.invalid/image.png"}}]}]
    }))
    .unwrap();
    req.extra.insert("max_tokens".into(), json!(16));
    req.reasoning = Some(json!({"type":"disabled"}));
    let plan = UpstreamClient::build_request_plan(&payload, &req, "deepseek-flash", false).unwrap();
    let wire = plan.body.unwrap();
    assert_eq!(wire["max_tokens"], 16);
    assert_eq!(wire["thinking"]["type"], "disabled");
    assert_eq!(wire["messages"], req.raw_body["messages"]);
    assert!(wire.get("gateway_debug").is_none());
    let denied =
        UpstreamClient::build_request_plan(&payload, &req, "unverified-image-model", false)
            .err()
            .unwrap();
    assert_eq!(
        denied.code.as_deref(),
        Some("sensenova_messages_image_model_unsupported")
    );
}

#[test]
fn sensenova_native_responses_model_and_stateless_admission() {
    use crate::protocol::registry::OPENAI_RESPONSES_FAMILY;
    let payload = payload();
    let request = |raw| crate::protocol::responses::normalize_responses(raw).unwrap();
    let req =
        request(json!({"model":"public-model","input":"hello","reasoning":{"effort":"none"}}));
    for model in ["sensenova-6.8-flash-lite", "deepseek-flash", "kimi-k3"] {
        assert!(families(Some(model))
            .iter()
            .any(|f| f == OPENAI_RESPONSES_FAMILY));
        let wire = UpstreamClient::build_request_plan(&payload, &req, model, false)
            .unwrap()
            .body
            .unwrap();
        assert_eq!(wire["model"], model);
        assert_eq!(wire["reasoning"]["effort"], "none");
    }
    let model = "sensenova-6.8-flash";
    assert!(!families(Some(model))
        .iter()
        .any(|f| f == OPENAI_RESPONSES_FAMILY));
    let error = UpstreamClient::build_request_plan(&payload, &req, model, false)
        .err()
        .unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("sensenova_responses_model_unsupported")
    );
    for (key, value) in [
        ("background", json!(true)),
        ("previous_response_id", json!(123)),
        ("context_management", json!([{"type":"compaction"}])),
        ("tools", json!([{"type":"mcp","server_label":"demo"}])),
        ("input", json!([{"type":"item_reference","id":"item"}])),
        (
            "input",
            json!([{"role":"user","content":[{"type":"input_file","file_id":"file"}]}]),
        ),
        (
            "input",
            json!([{"role":"user","content":[{"type":"input_image","file_id":"file"}]}]),
        ),
        (
            "input",
            json!([{"role":"user","content":[{"type":"input_video","video_url":"https://example.invalid/clip"}]}]),
        ),
    ] {
        let mut raw = req.raw_body.clone();
        raw[key] = value;
        assert!(
            UpstreamClient::build_request_plan(&payload, &request(raw), "deepseek-flash", false)
                .is_err(),
            "{key}"
        );
    }
    let mut raw = req.raw_body.clone();
    raw["text"] = json!({"format":{"type":"json_schema","name":"test","schema":{"type":"object"}}});
    let req = request(raw);
    assert!(
        UpstreamClient::build_request_plan(&payload, &req, "sensenova-6.8-flash-lite", false)
            .is_err()
    );
    assert!(UpstreamClient::build_request_plan(&payload, &req, "deepseek-flash", false).is_ok());
}

#[test]
fn sensenova_chat_reasoning_preserves_system_policy_but_never_restores_rewritten_turns() {
    use crate::protocol::canonical::ContentPart;
    let account = payload();
    let mut req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"caller-alias","messages":[
            {"role":"system","content":"old policy"},
            {"role":"assistant","content":"tool history","reasoning_content":"exact history"},
            {"role":"user","content":"continue"}
        ]
    })).unwrap();
    req.messages[0].content = vec![ContentPart::Text { text: "new policy".into() }];
    let plan = UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false).unwrap();
    let body = plan.body.unwrap();
    assert_eq!(body["messages"][0]["content"], "new policy");
    assert_eq!(body["messages"][1]["reasoning_content"], "exact history");
    req.messages[1].content = vec![ContentPart::Text { text: "rewritten".into() }];
    let failure = UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false).unwrap_err();
    assert_eq!(failure.http_status, Some(400));
    assert!(failure.message.contains("rewritten history"));
}
