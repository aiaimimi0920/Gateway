//! Native Key must not silently reuse the IDE/OIDC wire or inherit an OAuth refresh owner.
use super::*;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"kiro_compatible","base_url":"https://runtime.us-east-1.kiro.dev",
        "auth_mode":"kiro_api_key","responses_path":"/","api_key":"ksk_synthetic_only"}),
    )
    .unwrap()
}
fn request() -> CanonicalRelayRequest {
    openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]}))
        .unwrap()
}
#[test]
fn kiro_api_key_planner_uses_cli_origin_and_exact_model() {
    let plan =
        UpstreamClient::build_request_plan(&payload(), &request(), "claude-sonnet-contract", false)
            .unwrap();
    assert_eq!(plan.url, "https://runtime.us-east-1.kiro.dev/");
    let body = plan.body.unwrap();
    let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
    assert_eq!(current["origin"], "KIRO_CLI");
    assert_eq!(current["modelId"], "claude-sonnet-contract");
    assert!(body["conversationState"]
        .get("agentContinuationId")
        .is_none());
    assert!(body.get("profileArn").is_none());
}
#[test]
fn kiro_api_key_headers_cannot_be_replaced_by_forwarded_identity() {
    let extra = std::collections::HashMap::from([
        ("TokenType".into(), "OAUTH".into()),
        ("x-amz-target".into(), "wrong".into()),
        ("cookie".into(), "private".into()),
    ]);
    let headers = build_inference_headers(&payload(), &request(), "exact", Some(&extra));
    assert_eq!(
        headers.get("tokentype").and_then(|v| v.to_str().ok()),
        Some("API_KEY")
    );
    assert_eq!(headers["content-type"], "application/x-amz-json-1.0");
    assert_eq!(
        headers["x-amz-target"],
        "AmazonCodeWhispererStreamingService.GenerateAssistantResponse"
    );
    assert!(!headers.contains_key("cookie"));
    assert!(!headers["user-agent"].to_str().unwrap().contains("KiroIDE"));
}
#[test]
fn kiro_api_key_product_binding_rejects_legacy_or_generic_executor() {
    for (field, value) in [
        ("adapter", json!("openai_compatible")),
        ("base_url", json!("https://q.us-east-1.amazonaws.com")),
        ("auth_mode", json!("bearer")),
        ("responses_path", json!("/generateAssistantResponse")),
    ] {
        let mut raw = serde_json::to_value(payload()).unwrap();
        raw[field] = value;
        let bad: ProviderAccountPayload = serde_json::from_value(raw).unwrap();
        assert!(
            crate::protocol::call_flow::prepare(&bad).is_err(),
            "accepted {field}"
        );
    }
}

#[test]
fn kiro_api_key_request_refuses_unknown_wire_reasoning_forced_tools_and_oversized_model() {
    for patch in [
        json!({"unknown_extension":{"token":"private"}}),
        json!({"reasoning_effort":"high"}),
        json!({"tool_choice":"required"}),
        json!({"tools":[{"type":"web_search_preview"}]}),
    ] {
        let mut raw = json!({"model":"exact","messages":[{"role":"user","content":"hello"}]});
        raw.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let req = openai::normalize_chat_completions(raw).unwrap();
        assert!(UpstreamClient::build_request_plan(&payload(), &req, "exact", false).is_err());
    }
    assert!(
        UpstreamClient::build_request_plan(&payload(), &request(), &"x".repeat(257), false)
            .is_err()
    );
}

#[test]
fn kiro_api_key_function_tools_and_images_share_existing_bounded_pack_without_ide_origin() {
    let request=openai::normalize_chat_completions(json!({"model":"exact","tool_choice":"auto",
        "tools":[{"type":"function","function":{"name":"get_weather","parameters":{"type":"object","properties":{}}}}],
        "messages":[{"role":"user","content":[{"type":"text","text":"hello"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,aGVsbG8="}}]}]})).unwrap();
    let body = UpstreamClient::build_request_plan(&payload(), &request, "exact", false)
        .unwrap()
        .body
        .unwrap();
    let current = &body["conversationState"]["currentMessage"]["userInputMessage"];
    assert_eq!(current["origin"], "KIRO_CLI");
    assert_eq!(current["images"][0]["source"]["bytes"], "aGVsbG8=");
    assert_eq!(
        current["userInputMessageContext"]["tools"][0]["toolSpecification"]["name"],
        "get_weather"
    );
}
