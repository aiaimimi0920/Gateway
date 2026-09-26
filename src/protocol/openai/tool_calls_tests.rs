use serde_json::{json, Value};

use super::{normalize_chat_completions, unpack_openai_response};

fn normalize_message(message: Value) -> crate::protocol::canonical::CanonicalMessage {
    normalize_chat_completions(json!({
        "model": "gpt-4o",
        "messages": [message]
    }))
    .expect("fixture must normalize")
    .messages
    .into_iter()
    .next()
    .expect("fixture must contain one message")
}

fn unpack_message(message: Value) -> crate::protocol::canonical::CanonicalRelayResponse {
    unpack_openai_response(&json!({
        "model": "gpt-4o",
        "choices": [{
            "message": message,
            "finish_reason": "tool_calls"
        }]
    }))
    .expect("fixture must unpack")
}

#[test]
fn standard_tool_calls_take_precedence_and_default_to_function() {
    let message = normalize_message(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": "call_standard",
            "function": {"name": "standard", "arguments": "{}"}
        }],
        "function_call": {"name": "legacy", "arguments": "{}"}
    }));

    assert_eq!(message.tool_calls.len(), 1);
    assert_eq!(message.tool_calls[0].id.as_deref(), Some("call_standard"));
    assert_eq!(message.tool_calls[0].call_type, "function");
    assert_eq!(message.tool_calls[0].name.as_deref(), Some("standard"));
    assert!(message.tool_calls[0].raw.is_empty());
}

#[test]
fn legacy_function_call_keeps_call_id_and_serializes_object_arguments() {
    let message = normalize_message(json!({
        "role": "assistant",
        "content": null,
        "function_call": {
            "call_id": "call_legacy",
            "name": "lookup",
            "arguments": {"city": "Shanghai"}
        }
    }));

    assert_eq!(message.tool_calls.len(), 1);
    assert_eq!(message.tool_calls[0].id.as_deref(), Some("call_legacy"));
    assert_eq!(message.tool_calls[0].call_type, "function");
    assert_eq!(message.tool_calls[0].name.as_deref(), Some("lookup"));
    assert_eq!(
        message.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Shanghai\"}")
    );
    assert!(message.tool_calls[0].raw.is_empty());
}

#[test]
fn empty_standard_array_falls_back_to_legacy_function_call() {
    let response = unpack_message(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [],
        "function_call": {
            "id": "call_fallback",
            "name": "fallback",
            "arguments": "{}"
        }
    }));

    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_fallback"));
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("fallback"));
    assert!(response.tool_calls[0].raw.is_empty());
}

#[test]
fn string_arguments_decode_all_supported_provider_entities() {
    let response = unpack_message(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": "call_entities",
            "type": "function",
            "function": {
                "name": "decode",
                "arguments": "&quot;&#34;&apos;&#39;&lt;&gt;&amp;"
            }
        }]
    }));

    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(
        response.tool_calls[0].arguments.as_deref(),
        Some("\"\"''<>&")
    );
}
