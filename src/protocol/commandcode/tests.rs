//! 固定目标、独立身份和三入口实际 Chat wire 的本地配置合同。
use super::*;
use crate::{
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
fn document() -> Value {
    json!({"providers":[{"id":"arbitrary","preset":PRESET,"base_url":BASE,
        "explicit_credentials_only":true,"credentials":[{"id":"stable","api_key":"synthetic-commandcode",
        "supported_models":["o3-synthetic-chat"]}]}]})
}
fn compile(doc: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value::<RouteConfigYaml>(doc)?)
}
#[test]
fn commandcode_zdr_account_configuration_and_wire_header() {
    let mut doc = document();
    doc["providers"][0]["credentials"][0]["headers"] = json!({"x-cmd-zdr":"1"});
    assert!(compile(doc).is_ok());
    let mut p = payload();
    p.headers.insert("x-cmd-zdr".into(), "1".into());
    let req = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"alias", "messages":[{"role":"user","content":"test"}]
    }))
    .unwrap();
    let plan = runtime::plan(&p, &req, "opaque", false).unwrap();
    let mut headers = rquest::header::HeaderMap::new();
    finalize_headers(&p, &plan, &mut headers).unwrap();
    assert_eq!(headers["x-cmd-zdr"], "1");
}
#[test]
fn commandcode_catalog_preserves_manual_chat_without_widening_native_declarations() {
    let mut p = payload();
    let catalog = serde_json::from_value(json!({
        "sourceUrl":SOURCE,"checkedAt":"2026-11-13T00:00:00Z",
        "models":["messages-only","responses-only"],
        "supportedEndpoints":{"messages-only":["/messages"],"responses-only":["/responses"]}
    }))
    .unwrap();
    catalog::configure(&mut p, &catalog);
    for (model, selected) in [
        ("manual", "/chat/completions"),
        ("messages-only", "/messages"),
        ("responses-only", "/responses"),
    ] {
        for route in ["/chat/completions", "/messages", "/responses"] {
            assert_eq!(
                catalog::supports(&p, model, route).unwrap(),
                route == selected
            );
        }
        let expected = match selected {
            "/messages" => EndpointKind::Messages,
            "/responses" => EndpointKind::Responses,
            _ => EndpointKind::ChatCompletions,
        };
        assert_eq!(catalog::probe_endpoint(&p, model).unwrap(), expected);
    }
}
#[test]
fn commandcode_preserves_system_order_text_parts_and_model_independent_parameters() {
    let raw = json!({"model":"alias","max_tokens":17,"temperature":0.3,"messages":[
        {"role":"system","content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]},
        {"role":"user","content":"x"},{"role":"system","content":"c"}],
        "tools":[{"type":"function","function":{"name":"echo","strict":true,"parameters":{
            "type":"object","properties":{"x":{"type":"string"}},"additionalProperties":false}}}]});
    let req = crate::protocol::openai::normalize_chat_completions(raw.clone()).unwrap();
    let plan = runtime::plan(&payload(), &req, "o3-synthetic-chat", false).unwrap();
    let body = plan.body.unwrap();
    assert_eq!(body["messages"], raw["messages"]);
    assert_eq!(body["tools"], raw["tools"]);
    assert_eq!(body["max_tokens"], 17);
    assert_eq!(body["temperature"], 0.3);
    assert!(body.get("max_completion_tokens").is_none());
    let mut p = payload();
    p.base_url.push('/');
    assert_eq!(
        runtime::plan(&p, &req, "opaque", true).unwrap().url,
        format!("{BASE}/chat/completions")
    );
    let response_history=crate::protocol::responses::normalize_responses(json!({"model":"alias","input":[
        {"role":"assistant","content":[{"type":"output_text","text":"previous answer","annotations":[]}]},
        {"role":"user","content":"next question"}]})).unwrap();
    let next = runtime::plan(&p, &response_history, "opaque", false)
        .unwrap()
        .body
        .unwrap();
    assert_eq!(next["messages"][0]["content"], "previous answer");
    assert_eq!(next["messages"][1]["content"], "next question");
}

#[test]
fn commandcode_rejects_lossy_or_unimplemented_requests_before_transport() {
    use crate::protocol::{anthropic, openai, responses};
    for raw in [
        json!({"input":"x","reasoning":{"effort":"high"}}),
        json!({"input":"x","previous_response_id":"old"}),
        json!({"input":"x","store":true}),
        json!({"input":"x","conversation":"old"}),
        json!({"input":"x","tools":[{"type":"web_search"}]}),
        json!({"input":[{"type":"reasoning","summary":[]}]}),
        json!({"input":[{"role":"developer","content":"instruction"}]}),
        json!({"input":[{"role":"user","content":[{"type":"input_text","text":"x","annotations":[{}]}]}]}),
        json!({"input":[{"type":"function_call_output","call_id":"c","output":[{"type":"output_text","text":"x","annotations":[{}]}]}]}),
    ] {
        let mut raw = raw;
        raw["model"] = json!("alias");
        let req = responses::normalize_responses(raw.clone()).unwrap();
        assert!(
            runtime::plan(&payload(), &req, "opaque", false).is_err(),
            "{raw}"
        );
    }
    for extra in [
        json!({"modalities":["text","audio"],"audio":{"voice":"alloy","format":"wav"}}),
        json!({"web_search_options":{}}),
        json!({"n":2}),
        json!({"reasoning_effort":"high"}),
        json!({"previous_response_id":"old"}),
        json!({"background":true}),
        json!({"gateway_vertex_history":null}),
        json!({"gateway_bedrock_history":null}),
    ] {
        let mut raw = json!({"model":"alias","messages":[{"role":"user","content":"x"}]});
        raw.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let req = openai::normalize_chat_completions(raw.clone()).unwrap();
        assert!(
            runtime::plan(&payload(), &req, "opaque", false).is_err(),
            "{raw}"
        );
    }
    for message in [
        json!({"role":"assistant","content":"x","reasoning_content":"hidden"}),
        json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"https://image.invalid/a"}}]}),
        json!({"role":"assistant","tool_calls":[{"id":"c","type":"custom","custom":{"name":"x","input":"x"}}]}),
        json!({"role":"assistant","tool_calls":[{"id":"c","type":"function","function":{"name":"x","arguments":"{\"text\":\"&lt;\"}"}}]}),
    ] {
        let req = openai::normalize_chat_completions(json!({"model":"alias","messages":[message]}))
            .unwrap();
        assert!(runtime::plan(&payload(), &req, "opaque", false).is_err());
    }
    for extra in [
        json!({"tools":[{"type":"web_search_20250305","name":"web_search"}]}),
        json!({"thinking":{"type":"enabled","budget_tokens":1024}}),
        json!({"system":[{"type":"text","text":"a"},{"type":"thinking","thinking":"hidden"}]}),
        json!({"messages":[{"role":"user","content":[{"type":"tool_result","tool_use_id":"c","content":"x","is_error":true}]}]}),
    ] {
        let mut raw =
            json!({"model":"alias","max_tokens":32,"messages":[{"role":"user","content":"x"}]});
        raw.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let req = anthropic::normalize_messages(raw).unwrap();
        assert!(runtime::plan(&payload(), &req, "opaque", false).is_err());
    }
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
fn commandcode_has_own_implementation_and_no_default_model() {
    let p = payload();
    let line = crate::implementation_lines::line_for_payload(&p).unwrap();
    assert_eq!(line.canonical_protocol_profile(), PROFILE);
    assert_eq!(line.feature_name(), "line-commandcode-provider-api");
    assert!(p.default_model.is_none());
    let mut doc = document();
    doc["providers"][0]["credentials"] = json!([]);
    assert!(compile(doc).is_ok());
}
#[test]
fn commandcode_key_never_enters_generic_headers_or_discovery_jobs() {
    let extra =
        std::collections::HashMap::from([("authorization".into(), "Bearer foreign".into())]);
    assert!(
        crate::upstream::headers::build_upstream_headers_with(&payload(), Some(&extra)).is_empty()
    );
    for explicit in [false, true] {
        let mut doc = document();
        if explicit {
            doc["providers"][0]
                .as_object_mut()
                .unwrap()
                .remove("preset");
            doc["providers"][0]["adapter"] = json!("openai_compatible");
            doc["providers"][0]["auth_mode"] = json!(AUTH_MODE);
        }
        doc["providers"][0]["credentials"][0]["discovery_job"] =
            json!({"id":"00000000-0000-4000-8000-000000000001","status":"pending"});
        let error = compile(doc).err().expect("generic discovery must reject");
        assert!(error.to_string().contains("Command Code"), "{error:#}");
    }
}
#[test]
fn commandcode_rejects_target_identity_and_transport_overrides() {
    for (field, value) in [
        (
            "base_url",
            json!("https://token.commandcode.cn.evil.invalid/v1"),
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
        ("base_url", json!("https://platform.commandcode.cn")),
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
fn commandcode_plans_public_text_as_chat_and_binds_final_identity() {
    let p = payload();
    for req in [
        crate::protocol::openai::normalize_chat_completions(json!({"model":"gpt-alias","messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::anthropic::normalize_messages(json!({"model":"claude-alias","max_tokens":32,"messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::responses::normalize_responses(json!({"model":"gpt-alias","input":"hello"})).unwrap(),
    ] {
        assert!(!UpstreamClient::uses_native_codex(&p,&req,"gpt-alias"));
        assert!(!UpstreamClient::uses_native_claude(&p,&req,"claude-alias"));
        let mut plan = UpstreamClient::build_request_plan(&p,&req,"o3-synthetic-chat",true).unwrap();
        assert_eq!(plan.url,format!("{BASE}/chat/completions"));
        assert_eq!(plan.response_kind,EndpointKind::ChatCompletions);
        assert!(plan.body.as_ref().unwrap()["messages"].is_array());
        let mut headers = rquest::header::HeaderMap::new();
        for name in ["cookie","authorization","x-api-key","originator"] { headers.insert(name,"foreign".parse().unwrap()); }
        finalize_headers(&p,&plan,&mut headers).unwrap();
        assert_eq!(headers.len(),4);
        assert_eq!(headers["authorization"],"Bearer synthetic-commandcode");
        plan.url.push_str("?secret=x"); assert!(validate_plan(&p,&plan).is_err());
    }
}
