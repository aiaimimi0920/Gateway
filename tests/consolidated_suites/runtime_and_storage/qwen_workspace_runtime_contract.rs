//! Ordinary Qwen workspace calls cannot inherit Coding Plan or native Responses routes.
use neuro_gateway::{
    implementation_lines,
    protocol::{canonical::EndpointKind, responses},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

fn payload() -> Value {
    json!({"adapter":"openai_compatible", "auth_mode":"qwen_workspace_api",
        "base_url":"https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "api_key":"synthetic-qwen-key", "chat_completions_path":"/chat/completions"})
}

fn plan(value: Value, endpoint: EndpointKind, stream: bool) -> Result<Value, String> {
    let payload: ProviderAccountPayload = serde_json::from_value(value).unwrap();
    let mut req = responses::normalize_responses(json!({
        "model":"qwen-contract", "input":"hello", "stream":stream
    }))
    .unwrap();
    req.endpoint_kind = endpoint;
    UpstreamClient::build_request_plan(&payload, &req, "qwen-contract", stream)
        .map(|plan| json!({"url":plan.url,"body":plan.body}))
        .map_err(|error| error.code.unwrap_or_default())
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_text_endpoints_always_pack_chat_not_responses() {
    for endpoint in [
        EndpointKind::ChatCompletions,
        EndpointKind::Responses,
        EndpointKind::Messages,
        EndpointKind::Completions,
    ] {
        for stream in [false, true] {
            let result = plan(payload(), endpoint, stream).unwrap();
            assert_eq!(result["url"], "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/chat/completions");
            assert_eq!(result["body"]["messages"][0]["content"], "hello");
            assert_eq!(result["body"]["stream"], stream);
            assert!(result["body"].get("input").is_none());
        }
    }
    let mut no_path = payload();
    no_path
        .as_object_mut()
        .unwrap()
        .remove("chat_completions_path");
    assert!(plan(no_path, EndpointKind::Responses, false).unwrap()["body"]["messages"].is_array());
}

#[test]
fn qwen_workspace_payload_has_its_own_compile_gate() {
    let payload: ProviderAccountPayload = serde_json::from_value(payload()).unwrap();
    let line = implementation_lines::line_for_payload(&payload).expect("workspace belongs to Qwen");
    assert_eq!(line.canonical_protocol_profile(), "qwen_official_api");
    assert_eq!(
        implementation_lines::ensure_payload_compiled(&payload).is_ok(),
        cfg!(feature = "line-qwen-official-api")
    );
}

#[test]
fn qwen_workspace_marker_does_not_lose_product_identity_when_destination_changes() {
    for base in [
        "https://coding.dashscope.aliyuncs.com/v1",
        "https://evil.invalid/v1",
        "http://127.0.0.1:1/v1",
        "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1/responses",
    ] {
        let mut value = payload();
        value["base_url"] = json!(base);
        assert!(
            plan(value, EndpointKind::Responses, false).is_err(),
            "{base}"
        );
    }
}

#[test]
fn qwen_workspace_paths_adapters_and_flow_cannot_restore_native_wire() {
    for patch in [
        json!({"responses_path":"/responses"}),
        json!({"chat_completions_path":"/wrong"}),
        json!({"adapter":"anthropic_compatible"}),
        json!({"execution_mode":"browser_backed"}),
        json!({"keepalive":{"serviceUrl":"http://127.0.0.1:1"}}),
        json!({"discovered_protocols":[{"protocol":"responses",
            "api_base":"https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "verified_models":["qwen-contract"]}]}),
        json!({"call_flow":{"call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"}}),
    ] {
        let mut value = payload();
        value
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(
            plan(value, EndpointKind::Responses, true).is_err(),
            "{patch}"
        );
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_rejects_wrong_auth_key_and_non_text_surfaces() {
    for patch in [
        json!({"auth_mode":"x-api-key"}),
        json!({"auth_token":"synthetic-token"}),
        json!({"headers":{"Cookie":"synthetic-cookie"}}),
        json!({"api_key":""}),
        json!({"api_key":"synthetic\ninvalid"}),
    ] {
        let mut value = payload();
        value
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert_eq!(
            plan(value, EndpointKind::ChatCompletions, false).unwrap_err(),
            "invalid_qwen_workspace_api"
        );
    }
    for endpoint in [
        EndpointKind::Embeddings,
        EndpointKind::ImagesGenerations,
        EndpointKind::AudioSpeech,
    ] {
        assert_eq!(
            plan(payload(), endpoint, false).unwrap_err(),
            "unsupported_qwen_official_openai_endpoint"
        );
    }
}

#[cfg(feature = "line-qwen-official-api")]
#[test]
fn qwen_workspace_all_regions_and_unmarked_raw_payloads_keep_chat_semantics() {
    for region in [
        "cn-beijing",
        "ap-southeast-1",
        "ap-northeast-1",
        "eu-central-1",
        "us-east-1",
        "cn-hongkong",
    ] {
        let base = format!("https://workspace.{region}.maas.aliyuncs.com/compatible-mode/v1");
        let mut value = payload();
        value["base_url"] = json!(base);
        value.as_object_mut().unwrap().remove("auth_mode");
        value
            .as_object_mut()
            .unwrap()
            .remove("chat_completions_path");
        assert_eq!(
            plan(value.clone(), EndpointKind::Responses, true).unwrap()["url"],
            format!("{base}/chat/completions")
        );
        value["adapter"] = json!("anthropic_compatible");
        assert!(plan(value, EndpointKind::Messages, false).is_err());
    }
}

#[cfg(not(feature = "line-qwen-official-api"))]
#[test]
fn qwen_workspace_disabled_never_falls_back_to_generic_openai() {
    for endpoint in [
        EndpointKind::ChatCompletions,
        EndpointKind::Responses,
        EndpointKind::Messages,
    ] {
        assert_eq!(
            plan(payload(), endpoint, false).unwrap_err(),
            "gateway_provider_line_compiled_out"
        );
    }
}

#[cfg(not(feature = "line-qwen-official-api"))]
#[test]
fn qwen_workspace_api_off_also_keeps_coding_plan_planners_compiled_out() {
    let mut value = payload();
    value["base_url"] = json!("https://coding.dashscope.aliyuncs.com/v1");
    value["auth_mode"] = json!("bearer");
    value["responses_path"] = json!("/responses");
    for endpoint in [EndpointKind::ChatCompletions, EndpointKind::Responses] {
        assert_eq!(
            plan(value.clone(), endpoint, false).unwrap_err(),
            "gateway_provider_line_compiled_out"
        );
    }
}
