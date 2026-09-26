use std::collections::HashMap;

use serde_json::{json, Value};

use super::{
    inspect_prompt_cache_telemetry, normalize_messages, pack_anthropic,
    pack_anthropic_with_telemetry,
};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

fn text_message(role: MessageRole, text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content: vec![ContentPart::Text {
            text: text.to_string(),
        }],
        name: None,
        tool_call_id: None,
        tool_calls: Vec::new(),
    }
}

fn request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("source-model".to_string()),
        stream: false,
        messages,
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

#[test]
fn preserves_built_in_fields_when_extra_keys_collide() {
    let mut req = request(vec![text_message(MessageRole::User, "hello")]);
    req.extra.insert("model".to_string(), json!("shadow-model"));
    req.extra.insert(
        "messages".to_string(),
        json!([{"role": "assistant", "content": "shadow"}]),
    );
    req.extra.insert("stream".to_string(), json!(false));
    req.extra
        .insert("max_tokens".to_string(), json!("not-an-unsigned-integer"));

    let packed = pack_anthropic(&req, "claude-target", true);

    assert_eq!(packed["model"], json!("claude-target"));
    assert_eq!(
        packed["messages"],
        json!([{"role": "user", "content": "hello"}])
    );
    assert_eq!(packed["stream"], json!(true));
    assert_eq!(packed["max_tokens"], json!(4096));
}

#[test]
fn uses_first_system_message_and_omits_all_system_messages_from_history() {
    let req = request(vec![
        text_message(MessageRole::System, "first"),
        text_message(MessageRole::User, "question"),
        text_message(MessageRole::System, "second"),
    ]);

    let packed = pack_anthropic(&req, "claude-target", false);

    assert_eq!(packed["system"], json!("first"));
    assert_eq!(
        packed["messages"],
        json!([{"role": "user", "content": "question"}])
    );
}

#[test]
fn recognizes_false_like_cache_opt_outs_and_strips_control_fields() {
    let values = [
        json!(false),
        json!(" false "),
        json!("0"),
        json!("OFF"),
        json!("No"),
    ];

    for key in ["gateway_auto_cache", "neuro_auto_cache"] {
        for value in &values {
            let mut req = request(vec![text_message(MessageRole::User, "hello")]);
            req.extra.insert(key.to_string(), value.clone());

            let packed = pack_anthropic_with_telemetry(&req, "claude-target", false);

            assert!(packed.body.get("cache_control").is_none(), "{key}={value}");
            assert!(packed.body.get(key).is_none(), "{key}={value}");
            assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
            assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
        }
    }
}

#[test]
fn truthy_cache_control_flag_is_stripped_after_auto_marker_is_applied() {
    let mut req = request(vec![text_message(MessageRole::User, "hello")]);
    req.extra
        .insert("neuro_auto_cache".to_string(), json!(true));

    let packed = pack_anthropic_with_telemetry(&req, "claude-target", false);

    assert_eq!(packed.body["cache_control"], json!({"type": "ephemeral"}));
    assert!(packed.body.get("neuro_auto_cache").is_none());
    assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
    assert!(packed.prompt_cache_telemetry.auto_cache_applied);
}

#[test]
fn nested_controls_remain_and_nested_cache_marker_suppresses_auto_marker() {
    let nested = json!({
        "gateway_auto_cache": false,
        "cache_control": {"type": "ephemeral"}
    });
    let mut req = request(vec![text_message(MessageRole::User, "hello")]);
    req.extra.insert("metadata".to_string(), nested.clone());

    let packed = pack_anthropic_with_telemetry(&req, "claude-target", false);

    assert_eq!(packed.body["metadata"], nested);
    assert!(packed.body.get("cache_control").is_none());
    assert!(packed.prompt_cache_telemetry.client_has_cache_control);
    assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
}

#[test]
fn public_packing_and_telemetry_entry_points_agree() {
    let req = request(vec![text_message(MessageRole::User, "hello")]);

    let packed = pack_anthropic_with_telemetry(&req, "claude-target", true);
    let body = pack_anthropic(&req, "claude-target", true);
    let telemetry = inspect_prompt_cache_telemetry(&req, "claude-target", true);

    assert_eq!(body, packed.body);
    assert_eq!(telemetry, packed.prompt_cache_telemetry);
    assert_eq!(body["stream"], Value::Bool(true));
}

// Legacy packing coverage migrated from the root module.

#[test]
fn pack_extracts_system_to_top_level() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "system": "You are a bot.",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1024,
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(packed["system"], json!("You are a bot."));
    let msgs = packed["messages"].as_array().unwrap();
    assert!(msgs.iter().all(|m| m["role"] != "system"));
}

#[test]
fn pack_defaults_max_tokens_to_4096() {
    let body = json!({
        "model": "claude-3-haiku-20240307",
        "messages": [{"role": "user", "content": "hi"}],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-haiku-20240307", false);
    assert_eq!(packed["max_tokens"], json!(4096));
}

#[test]
fn pack_preserves_existing_system_cache_control_from_anthropic_raw_body() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "system": [{
            "type": "text",
            "text": "You are a bot.",
            "cache_control": {"type": "ephemeral"}
        }],
        "messages": [{"role": "user", "content": "hi"}],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(
        packed["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn pack_preserves_existing_tool_cache_control_from_anthropic_raw_body() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{
            "name": "search",
            "description": "Search",
            "input_schema": {"type": "object"},
            "cache_control": {"type": "ephemeral"}
        }],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(
        packed["tools"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn pack_auto_applies_cache_control_for_claude_when_missing() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "system": "You are a bot.",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{
            "name": "search",
            "description": "Search",
            "input_schema": {"type": "object"}
        }],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(packed["cache_control"], json!({"type": "ephemeral"}));
}

#[test]
fn pack_reports_client_supplied_cache_markers_in_telemetry() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "system": [{
            "type": "text",
            "text": "You are a bot.",
            "cache_control": {"type": "ephemeral"}
        }],
        "messages": [{"role": "user", "content": "hi"}],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
    assert!(packed.prompt_cache_telemetry.client_has_cache_control);
    assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
}

#[test]
fn pack_reports_auto_applied_cache_markers_in_telemetry() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "system": "You are a bot.",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{
            "name": "search",
            "description": "Search",
            "input_schema": {"type": "object"}
        }],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
    assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
    assert!(packed.prompt_cache_telemetry.auto_cache_applied);
}

#[test]
fn pack_does_not_auto_apply_cache_control_for_non_claude_models() {
    let body = json!({
        "model": "gpt-4o",
        "system": "You are a bot.",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{
            "name": "search",
            "description": "Search",
            "input_schema": {"type": "object"}
        }],
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "gpt-4o", false);
    assert!(packed["system"].is_string());
    assert!(packed.get("cache_control").is_none());
}

#[test]
fn pack_preserves_top_level_cache_control_from_openai_passthrough_extra() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [{"role": "user", "content": "hi"}],
        "cache_control": {"type": "ephemeral"}
    });
    let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(packed["cache_control"], json!({"type": "ephemeral"}));
}

#[test]
fn pack_respects_gateway_auto_cache_opt_out() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [{"role": "user", "content": "hi"}],
        "gateway_auto_cache": false
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
    assert!(packed.body.get("cache_control").is_none());
    assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
    assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
}

#[test]
fn normalize_and_repack_preserves_message_block_cache_control() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "text",
                "text": "hi",
                "cache_control": {"type": "ephemeral"}
            }]
        }]
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(
        packed["messages"][0]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn openai_system_text_block_cache_control_survives_anthropic_pack() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [
            {
                "role": "system",
                "content": [
                    {
                        "type": "text",
                        "text": "system prefix",
                        "cache_control": { "type": "ephemeral" }
                    }
                ]
            },
            {
                "role": "user",
                "content": "hi"
            }
        ]
    });
    let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(
        packed["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
}

#[test]
fn pack_tool_result_preserves_json_content() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::GeminiGenerateContent,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("models/gemini-2.5-pro".to_string()),
        stream: false,
        messages: vec![
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use the weather tool.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Json {
                    value: json!({"city": "Hangzhou", "condition": "sunny"}),
                }],
                name: Some("weather".to_string()),
                tool_call_id: Some("toolu_weather".to_string()),
                tool_calls: vec![],
            },
        ],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let packed = pack_anthropic(&req, "claude-3-5-sonnet", false);
    assert_eq!(
        packed["messages"][1]["content"][0]["type"],
        json!("tool_result")
    );
    assert_eq!(
        packed["messages"][1]["content"][0]["content"],
        json!([{
            "type": "text",
            "text": "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}"
        }])
    );
}

#[test]
fn pack_openai_required_tool_choice_to_anthropic_any() {
    let body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "weather",
                "parameters": {"type":"object"}
            }
        }],
        "tool_choice": "required"
    });
    let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(packed["tool_choice"], json!({"type": "any"}));
}

#[test]
fn pack_openai_responses_specific_tool_choice_to_anthropic_tool() {
    let body = json!({
        "model": "gpt-4o",
        "input": "Use only the weather tool.",
        "tools": [{
            "type": "function",
            "name": "weather",
            "parameters": {"type":"object"}
        }],
        "tool_choice": {
            "type": "function",
            "name": "weather"
        }
    });
    let req = crate::protocol::responses::normalize_responses(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(
        packed["tool_choice"],
        json!({"type": "tool", "name": "weather"})
    );
}

#[test]
fn pack_merges_extra_fields_back_into_body() {
    let body = json!({
        "model": "claude-3-5-sonnet-20241022",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1024,
        "temperature": 0.5,
        "context_management": {"enabled": true},
    });
    let req = normalize_messages(body).unwrap();
    let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
    assert_eq!(packed["temperature"], json!(0.5));
    assert_eq!(packed["context_management"], json!({"enabled": true}));
}
