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
fn sap_aicore_azure_eu20_binds_service_key_to_exact_origin() {
    let base = test_support::azure_base();
    let key = test_support::azure_key();
    let mut doc = document();
    doc["providers"][0]["base_url"] = json!(base);
    doc["providers"][0]["credentials"][0]["api_key"] = json!(key);
    assert!(
        compile(doc.clone()).is_ok(),
        "SAP Azure EU20 deployment must compile"
    );
    doc["providers"][0]["credentials"][0]["api_key"] = json!(test_support::KEY);
    assert!(
        compile(doc).is_err(),
        "a valid AWS key must not bind to Azure"
    );
    let aws = Endpoint::parse(test_support::BASE).unwrap();
    assert!(ServiceKey::parse(&key, &aws).is_err());
    for invalid in [
        base.replace("eu20", "eu21"),
        base.replace("westeurope", "northeurope"),
        base.replace("azure.ml", "gcp.ml"),
        base.replace(".com/", ".com.evil.invalid/"),
        base.replace(".com/", ".com:443/"),
        base.replace(".com/", ".com./"),
        base.replace("https://", "https://user@"),
        format!("{base}?x=1"),
        format!("{base}#fragment"),
        format!("{base}/"),
    ] {
        assert!(Endpoint::parse(&invalid).is_none(), "{invalid}");
    }
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

#[test]
fn sap_aicore_native_responses_images_require_explicit_deployment_opt_in() {
    for (responses, images, valid) in [
        (None, json!(false), true),
        (None, json!(true), false),
        (Some(false), json!(true), false),
        (Some(true), json!(true), true),
        (Some(true), json!("true"), false),
    ] {
        let mut doc = document();
        let extra = &mut doc["providers"][0]["extra_body"];
        extra[NATIVE_RESPONSES_IMAGE_INPUT] = images;
        if let Some(responses) = responses {
            extra[NATIVE_RESPONSES] = json!(responses);
        }
        assert_eq!(compile(doc).is_ok(), valid);
    }
}

#[test]
fn sap_aicore_native_responses_admission_is_bound_to_deployment() {
    let mut doc = document();
    doc["providers"][0]["extra_body"][NATIVE_RESPONSES] = json!(true);
    let compiled = compile(doc.clone()).unwrap();
    let p = compiled
        .snapshot()
        .select_credential_probe_target("stable")
        .unwrap()
        .payload;
    let mut req = crate::protocol::responses::normalize_responses(json!({
        "model":"public","input":[{"type":"reasoning","encrypted_content":"opaque","summary":[]},
            {"role":"user","content":"test"}],
        "reasoning":{"effort":"high","summary":"concise"},"include":["reasoning.encrypted_content"],
        "max_output_tokens":100,"store":false
    }))
    .unwrap();
    let original_input = req.raw_body["input"].clone();
    req.extra.insert("max_output_tokens".into(), json!(70));
    let result = runtime::plan(&p, &req, "deployment1", false).unwrap();
    let wire = result.body.unwrap();
    assert_eq!(wire["max_output_tokens"], 70);
    assert_eq!(wire["reasoning"], req.raw_body["reasoning"]);
    assert_eq!(wire["input"], original_input);
    assert_eq!(wire["include"], req.raw_body["include"]);
    assert!(runtime::plan(&p, &req, "another_deployment", false).is_err());
    for overlay in [
        json!({"extra_body":{"sap_native_responses":false}}),
        json!({"extra_body":{"sap_native_responses_image_input":true}}),
        json!({"extra_body":{"sap_api_version":"2024-12-01-preview"}}),
        json!({"headers":{"AI-Resource-Group":"foreign"}}),
        json!({"base_url":test_support::BASE.replace("deployment1", "deployment2")}),
    ] {
        let mut doc = doc.clone();
        let cred = doc["providers"][0]["credentials"][0]
            .as_object_mut()
            .unwrap();
        cred.extend(overlay.as_object().unwrap().clone());
        assert!(compile(doc).is_err(), "{overlay}");
    }
}

#[test]
fn sap_aicore_bedrock_pool_mode_is_exclusive_and_part_of_keep_binding() {
    let azure = document();
    let mut bedrock = azure.clone();
    bedrock["providers"][0]["extra_body"] = json!({"sap_bedrock_converse":true});
    assert!(compile(bedrock.clone()).is_ok());
    let pool = |doc: Value| {
        serde_json::from_value::<RouteConfigYaml>(doc)
            .unwrap()
            .providers
            .remove(0)
    };
    let new = pool(bedrock.clone());
    let mut old = pool(azure.clone());
    // Keep must recognize the protocol change even with the same URL, group and no version.
    old.extra_body.clear();
    assert_ne!(
        config_binding(&new, new.credentials.first()),
        config_binding(&old, old.credentials.first())
    );
    for extra in [
        json!({"sap_bedrock_converse":false}),
        json!({"sap_bedrock_converse":"true"}),
        json!({"sap_bedrock_converse":true,"sap_api_version":"2024-10-21"}),
        json!({"sap_bedrock_converse":true,"sap_orchestration_model":"gpt-4.1"}),
    ] {
        let mut doc = bedrock.clone();
        doc["providers"][0]["extra_body"] = extra;
        assert!(compile(doc).is_err());
    }
    for overlay in [
        json!({"extra_body":{"sap_bedrock_converse":false}}),
        json!({"headers":{"AI-Resource-Group":"other"}}),
    ] {
        let mut doc = bedrock.clone();
        doc["providers"][0]["credentials"][0]
            .as_object_mut()
            .unwrap()
            .extend(overlay.as_object().unwrap().clone());
        assert!(compile(doc).is_err(), "{overlay}");
    }
}
