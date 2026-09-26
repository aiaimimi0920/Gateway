use super::{
    build_messages_delta, build_messages_stop, build_messages_success, unpack_anthropic_response,
};
use crate::error::ErrorKind;
use crate::protocol::canonical::{CanonicalToolCall, TokenUsage};
use serde_json::json;

fn response_body(stop_reason: serde_json::Value) -> serde_json::Value {
    json!({
        "model": "claude-contract",
        "content": [
            {"type": "text", "text": "first"},
            {"type": "thinking", "thinking": "ignored"},
            {"type": "text", "text": "second"}
        ],
        "stop_reason": stop_reason,
        "_upstream_status": 206
    })
}

fn tool_call(arguments: Option<&str>) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some("toolu_contract".to_string()),
        call_type: "function".to_string(),
        name: Some("lookup".to_string()),
        arguments: arguments.map(str::to_string),
        raw: std::collections::HashMap::new(),
    }
}

#[test]
fn unpack_preserves_finish_reason_mapping_text_order_and_status() {
    for (anthropic, canonical) in [
        ("end_turn", Some("stop")),
        ("stop_sequence", Some("stop")),
        ("tool_use", Some("tool_calls")),
        ("max_tokens", Some("length")),
        ("vendor_reason", Some("vendor_reason")),
        ("", None),
    ] {
        let response = unpack_anthropic_response(&response_body(json!(anthropic))).unwrap();
        assert_eq!(response.text, "firstsecond");
        assert_eq!(response.finish_reason.as_deref(), canonical);
        assert_eq!(response.upstream_status, Some(206));
    }
}

#[test]
fn unpack_missing_content_preserves_server_error_contract() {
    let error = unpack_anthropic_response(&json!({"model": "claude-contract"})).unwrap_err();
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.code, None);
    assert_eq!(error.message, "Anthropic response missing `content` array");
}

#[test]
fn build_success_emits_tool_only_content_and_tool_stop_reason() {
    let response = build_messages_success(
        "msg_contract",
        "claude-contract",
        "",
        None,
        &[tool_call(Some("{\"city\":\"Hangzhou\"}"))],
        None,
    );

    assert_eq!(response["content"].as_array().unwrap().len(), 1);
    assert_eq!(response["content"][0]["type"], "tool_use");
    assert_eq!(response["content"][0]["id"], "toolu_contract");
    assert_eq!(response["content"][0]["name"], "lookup");
    assert_eq!(response["content"][0]["input"], json!({"city": "Hangzhou"}));
    assert_eq!(response["stop_reason"], "tool_use");
    assert!(response.get("usage").is_none());
}

#[test]
fn build_success_preserves_invalid_arguments_and_finish_mapping() {
    let invalid = build_messages_success(
        "msg_invalid",
        "claude-contract",
        "tool",
        None,
        &[tool_call(Some("not-json"))],
        Some("function_call"),
    );
    assert_eq!(invalid["content"][1]["input"], json!({}));
    assert_eq!(invalid["stop_reason"], "tool_use");

    let length = build_messages_success(
        "msg_length",
        "claude-contract",
        "truncated",
        None,
        &[],
        Some("length"),
    );
    assert_eq!(length["stop_reason"], "max_tokens");
}

#[test]
fn builders_preserve_usage_cache_fields_and_delta_wire() {
    let usage = TokenUsage {
        prompt_tokens: 11,
        completion_tokens: 7,
        total_tokens: 18,
        cache_creation_input_tokens: Some(5),
        cache_read_input_tokens: Some(3),
    };

    let stop = build_messages_stop(Some(&usage));
    assert_eq!(stop["type"], "message_delta");
    assert_eq!(stop["delta"]["stop_reason"], "end_turn");
    assert_eq!(stop["usage"]["input_tokens"], 11);
    assert_eq!(stop["usage"]["output_tokens"], 7);
    assert_eq!(stop["usage"]["cache_creation_input_tokens"], 5);
    assert_eq!(stop["usage"]["cache_read_input_tokens"], 3);

    assert_eq!(
        build_messages_delta("delta"),
        json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "delta"}
        })
    );
}
