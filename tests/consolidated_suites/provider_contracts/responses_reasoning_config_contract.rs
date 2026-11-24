//! Generation configuration is not historical reasoning or encrypted provider state.
use neuro_gateway::protocol::responses::{normalize_responses, pack_responses_bridge};
use serde_json::json;

#[test]
fn actual_responses_bridge_keeps_reasoning_generation_configuration() {
    let req = normalize_responses(json!({
        "model": "fixture", "input": "hello",
        "reasoning": {"effort": "high", "summary": "auto"}
    }))
    .unwrap();
    assert_eq!(
        pack_responses_bridge(&req, "sent", false)["reasoning"],
        json!({"effort": "high", "summary": "auto"})
    );
}

#[test]
fn real_chat_to_responses_request_plan_keeps_configuration_and_bridge_filtering() {
    use neuro_gateway::protocol::openai::normalize_chat_completions;
    use neuro_gateway::routing::candidate::ProviderAccountPayload;
    use neuro_gateway::upstream::openai_compatible_official_api_common::build_request_plan;

    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter": "openai_compatible", "base_url": "https://fixture.invalid",
        "api_key": "fixture-secret", "responses_path": "/responses"
    }))
    .unwrap();
    let req = normalize_chat_completions(json!({
        "model": "alias", "messages": [{"role": "user", "content": "hello"}],
        "reasoning": {"effort": "high", "summary": "auto"},
        "unsupported_vendor_field": "must-not-cross-bridge"
    }))
    .unwrap();
    for stream in [false, true] {
        let plan = build_request_plan(&payload, &req, "upstream", stream).unwrap();
        assert_eq!(plan.url, "https://fixture.invalid/responses");
        let body = plan.body.unwrap();
        assert_eq!(
            body["reasoning"],
            json!({"effort": "high", "summary": "auto"})
        );
        assert_eq!(body["stream"], stream);
        assert!(body.get("unsupported_vendor_field").is_none());
    }
}

#[test]
fn absent_and_explicit_null_configuration_are_not_rewritten() {
    for reasoning in [None, Some(serde_json::Value::Null)] {
        let mut body = json!({"model": "fixture", "input": "hello"});
        if let Some(value) = &reasoning {
            body["reasoning"] = value.clone();
        }
        let req = normalize_responses(body).unwrap();
        assert_eq!(
            pack_responses_bridge(&req, "sent", false).get("reasoning"),
            reasoning.as_ref()
        );
    }
}
