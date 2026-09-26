use super::*;

#[test]
fn pack_responses_infers_default_instructions_without_system_prompt() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Say hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4",
            "messages": [{ "role": "user", "content": "Say hello" }],
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses(&req, "gpt-5.4", true);
    assert_eq!(
        body.get("instructions").and_then(|value| value.as_str()),
        Some(
            "You are a helpful assistant. Follow the conversation in `input` and respond directly."
        )
    );
    assert!(body
        .get("input")
        .and_then(|value| value.as_array())
        .is_some());
}

#[test]
fn pack_responses_bridge_strips_max_output_tokens_for_minimal_provider_compatibility() {
    let mut extra = std::collections::HashMap::new();
    extra.insert("max_tokens".to_string(), json!(256));
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::Anthropic,
        endpoint_kind: EndpointKind::Messages,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You are terse.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Say hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4",
            "system": "You are terse.",
            "max_tokens": 256,
            "messages": [{ "role": "user", "content": "Say hello" }],
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra,
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", true);
    assert!(body
        .get("input")
        .and_then(|value| value.as_array())
        .is_some());
    assert!(body.get("max_output_tokens").is_none());
}

#[test]
fn pack_responses_bridge_serializes_text_tool_results_as_plain_output_strings() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Text {
                text: "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}".to_string(),
            }],
            name: None,
            tool_call_id: Some("call_1".to_string()),
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(
        body["input"][0],
        json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}"
        })
    );
}

#[test]
fn pack_responses_bridge_serializes_json_tool_results_as_plain_output_strings() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::Anthropic,
        endpoint_kind: EndpointKind::Messages,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::Tool,
            content: vec![ContentPart::Json {
                value: json!({"city": "Hangzhou", "weather": "sunny"}),
            }],
            name: None,
            tool_call_id: Some("call_1".to_string()),
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(
        body["input"][0],
        json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}"
        })
    );
}

#[test]
fn pack_responses_bridge_uses_output_text_for_assistant_history_messages() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![ContentPart::Text {
                text: "Previous assistant summary.".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(
        body["input"][0],
        json!({
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": "Previous assistant summary."
            }]
        })
    );
}

#[test]
fn pack_responses_bridge_skips_empty_assistant_placeholders_before_function_calls() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![ContentPart::Text {
                text: String::new(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: Some("call_weather".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: std::collections::HashMap::new(),
            }],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    let input = body["input"].as_array().cloned().unwrap_or_default();
    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0],
        json!({
            "type": "function_call",
            "call_id": "call_weather",
            "name": "weather",
            "arguments": "{\"city\":\"Hangzhou\"}"
        })
    );
}

#[test]
fn pack_responses_bridge_rewraps_untyped_raw_parts_as_input_text() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Responses,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Raw {
                value: json!({"city": "Hangzhou"}),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(
        body["input"][0],
        json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "{\"city\":\"Hangzhou\"}"
            }]
        })
    );
}

#[test]
fn pack_responses_maps_anthropic_any_tool_choice_to_required() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::Anthropic,
        endpoint_kind: EndpointKind::Messages,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Use the tool".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: None,
            input_schema: Some(json!({
                "type": "object",
                "properties": { "city": { "type": "string" } }
            })),
            raw: std::collections::HashMap::new(),
        }],
        tool_choice: Some(json!({ "type": "any" })),
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4",
            "tool_choice": { "type": "any" }
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(body.get("tool_choice"), Some(&json!("required")));
}

#[test]
fn pack_responses_maps_openai_specific_tool_choice_to_responses_function_object() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("gpt-5.4".to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Use the tool".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: None,
            input_schema: Some(json!({
                "type": "object",
                "properties": { "city": { "type": "string" } }
            })),
            raw: std::collections::HashMap::new(),
        }],
        tool_choice: Some(json!({ "type": "function", "function": { "name": "weather" } })),
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": "gpt-5.4",
            "tool_choice": { "type": "function", "function": { "name": "weather" } }
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let body = pack_responses_bridge(&req, "gpt-5.4", false);
    assert_eq!(
        body.get("tool_choice"),
        Some(&json!({"type": "function", "name": "weather"}))
    );
}
