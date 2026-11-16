//! 固定产品身份、配置准入和最终发送边界；所有材料均为合成 Key。
use super::*;
use crate::{
    protocol::openai,
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

fn document() -> Value {
    json!({"providers":[{"id":"arbitrary", "preset":PRESET, "base_url":BASE,
        "explicit_credentials_only":true,
        "credentials":[{"id":"stable", "api_key":"cpk-synthetic", "supported_models":["agnes-live-text"]}]}]})
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
fn agnes_template_and_explicit_key_share_fixed_identity_without_refresh() {
    let p = payload();
    assert!(owns_payload(&p));
    assert_eq!(p.api_key, "cpk-synthetic");
    assert_eq!(
        crate::implementation_lines::line_for_payload(&p)
            .unwrap()
            .feature_name(),
        "line-agnes-official-api"
    );
    assert!(!p.bridges_openai_text_endpoint_to_responses(EndpointKind::Messages));
    let mut doc = document();
    doc["providers"][0]["credentials"] = json!([]);
    assert!(
        compile(doc).is_ok(),
        "empty template is not an executable account"
    );
}

#[test]
fn agnes_responses_keep_product_transport_for_json_and_sse() {
    let p = payload();
    for stream in [false, true] {
        let req = crate::protocol::responses::normalize_responses(json!({
            "model":"agnes-live-text", "input":"hello", "stream":stream, "store":false
        }))
        .unwrap();
        assert!(!UpstreamClient::uses_native_codex(
            &p,
            &req,
            "agnes-live-text"
        ));
        let plan = UpstreamClient::build_request_plan(&p, &req, "agnes-live-text", stream).unwrap();
        assert_eq!(plan.url, format!("{BASE}/responses"));
        assert_eq!(plan.response_kind, EndpointKind::Responses);
        validate_plan(&p, &plan).unwrap();
    }
}

#[test]
fn agnes_configuration_rejects_target_identity_refresh_and_header_overrides() {
    for (field, value) in [
        ("base_url", json!("https://foreign.invalid/v1")),
        (
            "base_url",
            json!("https://apihub.agnes-ai.com/v1?key=synthetic"),
        ),
        ("auth_mode", json!("bearer")),
        ("headers", json!({"cookie":"foreign"})),
        ("extra_body", json!({"store":true})),
        ("request_client_profile", json!("codex_cli")),
        ("call_flow", json!({"call_adapter":"anthropic"})),
        ("chat_completions_path", json!("/elsewhere")),
    ] {
        let mut doc = document();
        doc["providers"][0][field] = value;
        assert!(compile(doc).is_err(), "provider {field}");
    }
    for (field, value) in [
        ("base_url", json!("https://foreign.invalid/v1")),
        ("auth_mode", json!("bearer")),
        ("refresh_token", json!("synthetic-refresh")),
        ("api_key", json!("synthetic\r\nheader")),
        ("headers", json!({"authorization":"foreign"})),
    ] {
        let mut doc = document();
        doc["providers"][0]["credentials"][0][field] = value;
        assert!(compile(doc).is_err(), "credential {field}");
    }
}

#[test]
fn agnes_final_plan_rejects_late_mutation_and_uses_only_bound_key() {
    let p = payload();
    let req = openai::normalize_chat_completions(
        json!({"model":"agnes-live-text","messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    let plan = UpstreamClient::build_request_plan(&p, &req, "agnes-live-text", true).unwrap();
    let new_plan =
        || UpstreamClient::build_request_plan(&p, &req, "agnes-live-text", true).unwrap();
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert("cookie", "foreign".parse().unwrap());
    headers.insert("authorization", "Bearer caller".parse().unwrap());
    headers.insert("x-api-key", "foreign".parse().unwrap());
    finalize_headers(&p, &plan, &mut headers).unwrap();
    assert_eq!(headers.len(), 4);
    assert_eq!(headers["authorization"], "Bearer cpk-synthetic");
    assert_eq!(headers["accept"], "text/event-stream");
    assert!(!headers.contains_key("cookie") && !headers.contains_key("x-api-key"));
    for model in ["agnes-image-live", "AGNES-VIDEO-live"] {
        let mut bad = new_plan();
        bad.body.as_mut().unwrap()["model"] = json!(model);
        assert!(validate_plan(&p, &bad).is_err());
    }
    let mut bad = new_plan();
    bad.url = "https://foreign.invalid/v1/chat/completions".into();
    assert!(validate_plan(&p, &bad).is_err());
    let mut bad = new_plan();
    bad.query.push(("key".into(), "synthetic".into()));
    assert!(validate_plan(&p, &bad).is_err());
    let mut bad = p.clone();
    bad.api_key.clear();
    assert!(validate_plan(&bad, &plan).is_err());
}
