use super::*;

#[test]
fn normalize_string_input() {
    let body = json!({
        "model": "gpt-4o",
        "input": "Tell me a joke.",
    });
    let req = normalize_responses(body).unwrap();
    assert_eq!(req.requested_model.as_deref(), Some("gpt-4o"));
    assert_eq!(req.endpoint_kind, EndpointKind::Responses);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, MessageRole::User);
    assert_eq!(
        req.messages[0].content[0].as_text(),
        Some("Tell me a joke.")
    );
}

#[test]
fn normalize_with_instructions() {
    let body = json!({
        "model": "gpt-4o",
        "instructions": "Be concise.",
        "input": "Hello",
    });
    let req = normalize_responses(body).unwrap();
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, MessageRole::System);
    assert_eq!(req.messages[0].content[0].as_text(), Some("Be concise."));
    assert_eq!(req.messages[1].role, MessageRole::User);
}

#[test]
fn normalize_messages_array_input() {
    let body = json!({
        "model": "gpt-4o",
        "input": [
            {"role": "user", "content": "Hello"},
            {"role": "assistant", "content": "Hi there!"},
            {"role": "user", "content": "How are you?"},
        ],
    });
    let req = normalize_responses(body).unwrap();
    assert_eq!(req.messages.len(), 3);
    assert_eq!(req.messages[0].role, MessageRole::User);
    assert_eq!(req.messages[1].role, MessageRole::Assistant);
    assert_eq!(req.messages[2].role, MessageRole::User);
}

#[test]
fn normalize_missing_input_returns_error() {
    let body = json!({"model": "gpt-4o"});
    assert!(normalize_responses(body).is_err());
}

#[test]
fn normalize_captures_previous_response_id() {
    let body = json!({
        "model": "gpt-4o",
        "input": "Continue",
        "previous_response_id": "resp_abc123",
    });
    let req = normalize_responses(body).unwrap();
    assert_eq!(req.previous_response_id.as_deref(), Some("resp_abc123"));
}

#[test]
fn build_responses_success_structure() {
    let usage = TokenUsage {
        prompt_tokens: 12,
        completion_tokens: 8,
        total_tokens: 20,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: Some(4),
    };
    let resp = build_responses_success("resp_abc", "gpt-4o", "Hello!", Some(&usage), &[], None);
    assert_eq!(resp["id"], "resp_abc");
    assert_eq!(resp["object"], "response");
    assert_eq!(resp["status"], "completed");
    assert_eq!(resp["output"][0]["content"][0]["text"], "Hello!");
    assert_eq!(resp["usage"]["input_tokens"], 12);
    assert_eq!(resp["usage"]["output_tokens"], 8);
    assert_eq!(resp["usage"]["input_tokens_details"]["cached_tokens"], 4);
}

#[test]
fn unpack_responses_response_reads_usage() {
    let body = json!({
        "id": "resp_test",
        "model": "gpt-4o",
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": "Hello"
            }]
        }],
        "usage": {
            "input_tokens": 9,
            "output_tokens": 6,
            "total_tokens": 15,
            "input_tokens_details": {
                "cached_tokens": 3
            }
        }
    });
    let response = unpack_responses_response(&body).unwrap();
    assert_eq!(response.text, "Hello");
    let usage = response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 9);
    assert_eq!(usage.completion_tokens, 6);
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(usage.cache_read_input_tokens, Some(3));
}

#[test]
fn normalize_responses_parses_function_call_and_output_items() {
    let body = json!({
        "model": "gpt-4o",
        "tools": [{
            "type": "function",
            "name": "weather",
            "parameters": {"type": "object"}
        }],
        "tool_choice": "required",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "weather",
                "arguments": {"city": "Hangzhou"}
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"ok\":true}"
            }
        ]
    });
    let req = normalize_responses(body).unwrap();
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.tool_choice, Some(json!("required")));
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, MessageRole::Assistant);
    assert_eq!(req.messages[0].tool_calls.len(), 1);
    assert_eq!(req.messages[1].role, MessageRole::Tool);
    assert_eq!(req.messages[1].tool_call_id.as_deref(), Some("call_1"));
}

#[test]
fn normalize_responses_accepts_top_level_function_tool_shape() {
    let body = json!({
        "model": "gpt-4o",
        "input": "Use the weather tool",
        "tools": [{
            "type": "function",
            "name": "weather",
            "description": "Read weather",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }
        }]
    });

    let req = normalize_responses(body).unwrap();
    assert_eq!(req.tools.len(), 1);
    assert_eq!(req.tools[0].name.as_deref(), Some("weather"));
    assert_eq!(req.tools[0].description.as_deref(), Some("Read weather"));
    assert_eq!(
        req.tools[0]
            .input_schema
            .as_ref()
            .and_then(|schema| schema.get("required"))
            .and_then(|value| value.as_array())
            .map(|value| value.len()),
        Some(1)
    );
}

#[test]
fn build_responses_success_emits_function_call_items() {
    let resp = build_responses_success(
        "resp_tools",
        "gpt-4o",
        "",
        None,
        &[CanonicalToolCall {
            id: Some("call_1".to_string()),
            call_type: "function".to_string(),
            name: Some("weather".to_string()),
            arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
            raw: std::collections::HashMap::new(),
        }],
        Some("tool_calls"),
    );
    assert_eq!(resp["status"], "tool_calls");
    assert_eq!(resp["output"][0]["type"], "function_call");
    assert_eq!(resp["output"][0]["call_id"], "call_1");
}

#[test]
fn unpack_responses_response_reads_function_call_items() {
    let body = json!({
        "id": "resp_tool",
        "model": "gpt-4o",
        "status": "completed",
        "output": [{
            "type": "function_call",
            "call_id": "call_1",
            "name": "weather",
            "arguments": {"city": "Hangzhou"}
        }]
    });
    let response = unpack_responses_response(&body).unwrap();
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
}

#[test]
fn unpack_responses_response_reads_nested_function_call_payload() {
    let body = json!({
        "id": "resp_tool_nested",
        "model": "gpt-4o",
        "status": "completed",
        "output": [{
            "type": "function_call",
            "call_id": "call_weather",
            "function": {
                "name": "weather",
                "arguments": {
                    "city": "Hangzhou"
                }
            }
        }]
    });
    let response = unpack_responses_response(&body).unwrap();
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        response.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}
