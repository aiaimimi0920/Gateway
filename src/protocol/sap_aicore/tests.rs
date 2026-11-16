//! Compilation and public-plan admission precede every credential-bearing request.
use super::*;
use crate::{
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};
pub(super) fn document() -> Value {
    json!({"providers":[{"id":"arbitrary","preset":PRESET,"base_url":test_support::BASE,
        "headers":{"AI-Resource-Group":"group-a"},"extra_body":{"sap_api_version":"2024-10-21"},
        "explicit_credentials_only":true,"credentials":[{"id":"stable","api_key":test_support::KEY,
        "supported_models":["deployment1"]}]}]})
}
fn compile(doc: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value::<RouteConfigYaml>(doc)?)
}
pub(super) fn payload() -> ProviderAccountPayload {
    compile(document())
        .unwrap()
        .snapshot()
        .select_credential_probe_target("stable")
        .unwrap()
        .payload
}
#[test]
fn sap_aicore_compiles_explicit_service_key_and_empty_pool() {
    let p = payload();
    let line = crate::implementation_lines::line_for_payload(&p).unwrap();
    assert_eq!(line.canonical_protocol_profile(), PROFILE);
    assert_eq!(line.feature_name(), "line-sap-aicore-chat");
    assert!(p.default_model.is_none());
    let mut doc = document();
    doc["providers"][0]["credentials"] = json!([]);
    assert!(compile(doc).is_ok());
}
#[test]
fn sap_aicore_rejects_target_secret_and_transport_overrides() {
    for (field,value) in [
        ("base_url", json!("https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com.evil.invalid/v2/inference/deployments/deployment1")),
        ("auth_mode",json!("bearer")),("headers",json!({"AI-Resource-Group":"group-a","cookie":"foreign"})),
        ("extra_body",json!({"sap_api_version":"2024-02-30"})), ("responses_path",json!("/responses")),
        ("request_client_profile",json!("codex")), ("call_flow",json!({"call_adapter":"anthropic"})),
    ] { let mut doc=document();doc["providers"][0][field]=value; assert!(compile(doc).is_err(),"{field}"); }
    for (field, value) in [
        ("url", json!("https://evil.invalid")),
        ("clientsecret", json!("bad\r\nsecret")),
        ("clientid", json!("")),
        (
            "serviceurls",
            json!({"AI_API_URL":"https://api.ai.prod.us-east-1.aws.ml.hana.ondemand.com"}),
        ),
    ] {
        let mut key: Value = serde_json::from_str(test_support::KEY).unwrap();
        key[field] = value;
        let mut doc = document();
        doc["providers"][0]["credentials"][0]["api_key"] = json!(key.to_string());
        assert!(compile(doc).is_err(), "{field}");
    }
    for suffix in ["/../elsewhere", "%2Felsewhere", "?x=1", "#fragment", "/"] {
        assert!(Endpoint::parse(&format!("{}{suffix}", test_support::BASE)).is_none());
    }
}
#[test]
fn sap_aicore_three_public_plans_keep_tools_and_explicit_version() {
    let p = payload();
    for req in [
        crate::protocol::openai::normalize_chat_completions(json!({"model":"gpt-alias","messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::anthropic::normalize_messages(json!({"model":"claude-alias","max_tokens":32,"messages":[{"role":"user","content":"hello"}]})).unwrap(),
        crate::protocol::responses::normalize_responses(json!({"model":"gpt-alias","input":"hello","store":false})).unwrap(),
    ] {
        assert!(!UpstreamClient::uses_native_codex(&p,&req,"gpt-alias"));
        assert!(!UpstreamClient::uses_native_claude(&p,&req,"claude-alias"));
        let plan=UpstreamClient::build_request_plan(&p,&req,"deployment1",true).unwrap();
        assert_eq!(plan.response_kind,crate::protocol::canonical::EndpointKind::ChatCompletions);
        assert_eq!(plan.query,vec![("api-version".into(),"2024-10-21".into())]);
        assert!(plan.body.as_ref().unwrap().get("model").is_none());
        assert_eq!(plan.body.unwrap()["stream_options"]["include_usage"],true);
        assert!(runtime::plan(&p,&req,"other",true).is_err());
    }
}
#[test]
fn sap_aicore_legacy_version_preserves_explicit_stream_options() {
    let mut p = payload();
    p.extra_body
        .as_mut()
        .unwrap()
        .insert("sap_api_version".into(), json!("2023-05-15"));
    let mut raw = json!({"model":"deployment1","messages":[{"role":"user","content":"hello"}]});
    let req = crate::protocol::openai::normalize_chat_completions(raw.clone()).unwrap();
    assert!(runtime::plan(&p, &req, "deployment1", true)
        .unwrap()
        .body
        .unwrap()
        .get("stream_options")
        .is_none());
    raw["stream_options"] = json!({"include_usage":true});
    let req = crate::protocol::openai::normalize_chat_completions(raw).unwrap();
    assert_eq!(
        runtime::plan(&p, &req, "deployment1", true)
            .unwrap()
            .body
            .unwrap()["stream_options"]["include_usage"],
        true
    );
}

#[test]
fn sap_aicore_service_key_never_enters_generic_headers_or_discovery_jobs() {
    let p = payload();
    let extra =
        std::collections::HashMap::from([("authorization".into(), "Bearer foreign".into())]);
    assert!(crate::upstream::headers::build_upstream_headers_with(&p, Some(&extra)).is_empty());
    for preset in [true, false] {
        let mut doc = document();
        if !preset {
            doc["providers"][0]
                .as_object_mut()
                .unwrap()
                .remove("preset");
            doc["providers"][0]["adapter"] = json!("openai_compatible");
            doc["providers"][0]["auth_mode"] = json!(AUTH_MODE);
        }
        doc["providers"][0]["credentials"][0]["discovery_job"] =
            json!({"id":"00000000-0000-4000-8000-000000000001","status":"pending"});
        let error = compile(doc)
            .err()
            .expect("SAP discovery jobs must be rejected");
        assert!(error.to_string().contains("SAP AI Core"), "{error:#}");
    }
}
