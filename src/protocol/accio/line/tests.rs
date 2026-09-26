use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalTool, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use bytes::Bytes;
use serde_json::json;

fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("claude-sonnet-4-6".into()),
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    }
}

fn text_msg(role: MessageRole, text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content: vec![ContentPart::Text { text: text.into() }],
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }
}

fn aws_eventstream_frame(payload: Value) -> Vec<u8> {
    let payload = serde_json::to_vec(&payload).unwrap();
    let total_len = 16 + payload.len();
    let mut frame = Vec::with_capacity(total_len);
    frame.extend_from_slice(&(total_len as u32).to_be_bytes());
    frame.extend_from_slice(&(0u32).to_be_bytes());
    frame.extend_from_slice(&(0u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&(0u32).to_be_bytes());
    frame
}

#[test]
fn pack_includes_current_accio_fields() {
    let mut req = make_request(vec![
        text_msg(MessageRole::System, "system"),
        text_msg(MessageRole::User, "hello"),
    ]);
    req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some("weather".into()),
        description: Some("Lookup weather".into()),
        input_schema: Some(json!({"type":"object"})),
        raw: HashMap::new(),
    });
    req.reasoning = Some(json!({"type":"enabled","budget_tokens":5000}));
    req.explicit_session_key = Some("user_123".into());
    req.extra
        .insert("conversation_id".into(), json!("conv_123"));
    req.extra
        .insert("session_key".into(), json!("agent:demo:main:cid:conv_123"));
    req.extra.insert("accountId".into(), json!("emp-123"));
    req.extra.insert("tenant".into(), json!("tenant-alpha"));
    req.extra.insert("iaiTag".into(), json!("phoenix-desktop"));

    let body = pack_accio(&req, "claude-sonnet-4-6", true);
    assert_eq!(body["system_instruction"], "system");
    assert_eq!(body["conversation_id"], "conv_123");
    assert_eq!(body["conversationId"], "conv_123");
    assert_eq!(body["session_key"], "agent:demo:main:cid:conv_123");
    assert_eq!(body["sessionKey"], "agent:demo:main:cid:conv_123");
    assert_eq!(body["empid"], "emp-123");
    assert_eq!(body["tenant"], "tenant-alpha");
    assert_eq!(body["iaiTag"], "phoenix-desktop");
    assert_eq!(body["incremental"], json!(true));
    assert_eq!(body["thinking_level"], "medium");
    assert_eq!(body["tools"][0]["name"], "weather");
    assert_eq!(body["properties"]["openai_user"], "user_123");
}

#[test]
fn pack_sets_incremental_false_for_non_stream_requests() {
    let req = make_request(vec![text_msg(MessageRole::User, "hello")]);
    let body = pack_accio(&req, "claude-sonnet-4-6", false);
    assert_eq!(body["incremental"], json!(false));
}

#[test]
fn pack_auto_applies_cache_control_for_claude_when_missing() {
    let req = make_request(vec![text_msg(MessageRole::User, "hello")]);
    let body = pack_accio(&req, "claude-sonnet-4-6", false);
    assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
}

#[test]
fn pack_preserves_existing_cache_control_from_raw_body() {
    let mut req = make_request(vec![text_msg(MessageRole::User, "hello")]);
    req.raw_body = json!({
        "messages": [{
            "role": "user",
            "content": [{
                "type": "text",
                "text": "hello",
                "cache_control": {"type": "ephemeral", "ttl": "1h"}
            }]
        }]
    });
    let body = pack_accio(&req, "claude-sonnet-4-6", false);
    assert_eq!(
        body["cache_control"],
        json!({"type": "ephemeral", "ttl": "1h"})
    );
}

#[test]
fn pack_respects_gateway_auto_cache_opt_out() {
    let mut req = make_request(vec![text_msg(MessageRole::User, "hello")]);
    req.raw_body = json!({
        "gateway_auto_cache": false
    });
    let body = pack_accio(&req, "claude-sonnet-4-6", false);
    assert!(body.get("cache_control").is_none());
    let telemetry = inspect_prompt_cache_telemetry(&req, "claude-sonnet-4-6");
    assert!(!telemetry.client_has_cache_control);
    assert!(!telemetry.auto_cache_applied);
}

#[test]
fn unpack_reads_function_call_parts() {
    let body = json!({
        "candidates": [{
            "content": {
                "parts": [
                    {"text": "Need tool"},
                    {"functionCall": {"id":"call_1","name":"weather","argsJson":"{\"city\":\"Hangzhou\"}"}}
                ]
            },
            "finishReason": "tool_use"
        }],
        "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15},
        "modelVersion": "claude-sonnet-4-6"
    });
    let response = unpack_accio_response(&body).unwrap();
    assert_eq!(response.text, "Need tool");
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(response.usage.unwrap().cache_read_input_tokens, None);
}

#[test]
fn unpack_reads_bedrock_tool_use_blocks() {
    let body = json!({
        "output": {
            "message": {
                "content": [
                    {"text": "Need a tool"},
                    {"toolUse": {"toolUseId": "toolu_1", "name": "weather", "input": {"city": "Hangzhou"}}}
                ]
            }
        },
        "stopReason": "tool_use",
        "usage": {"inputTokens": 11, "outputTokens": 7, "totalTokens": 18},
        "model": "bedrock-claude"
    });
    let response = unpack_accio_response(&body).unwrap();
    assert_eq!(response.text, "Need a tool");
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("toolu_1"));
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(response.usage.unwrap().total_tokens, 18);
}

#[test]
fn unpack_reads_cohere_tool_calls() {
    let body = json!({
        "message": {
            "content": [{"text": "Let me call a tool"}],
            "tool_calls": [{
                "id": "call_1",
                "function": {
                    "name": "weather",
                    "arguments": {"city": "Hangzhou"}
                }
            }]
        },
        "finish_reason": "tool_call",
        "model": "command-r"
    });
    let response = unpack_accio_response(&body).unwrap();
    assert_eq!(response.text, "Let me call a tool");
    assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        response.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

#[test]
fn translate_bedrock_stream_tool_use_to_openai_chunk() {
    let line = br#"data: {"type":"contentBlockStart","index":0,"start":{"toolUse":{"toolUseId":"toolu_1","name":"weather"}}}"#;
    let outputs = parse_sse_line(line).unwrap();
    assert!(matches!(&outputs[0], ParsedEvent::ToolStart { .. }));
}

#[tokio::test]
async fn translate_eventstream_bedrock_frames_to_openai_chunks() {
    use futures::StreamExt;

    let frames = vec![
        aws_eventstream_frame(json!({
            "messageStart": { "model": "bedrock-claude" }
        })),
        aws_eventstream_frame(json!({
            "contentBlockStart": {
                "contentBlockIndex": 0,
                "start": { "toolUse": { "toolUseId": "toolu_1", "name": "weather" } }
            }
        })),
        aws_eventstream_frame(json!({
            "contentBlockDelta": {
                "contentBlockIndex": 0,
                "delta": { "partial_json": "{\"city\":\"Hangzhou\"}" }
            }
        })),
        aws_eventstream_frame(json!({
            "messageStop": { "stopReason": "tool_use" }
        })),
        aws_eventstream_frame(json!({
            "metadata": { "usage": { "inputTokens": 8, "outputTokens": 5, "totalTokens": 13 } }
        })),
    ];

    let chunks: Vec<Result<Bytes, rquest::Error>> = frames
        .into_iter()
        .map(|frame| Ok(Bytes::from(frame)))
        .collect();
    let mut translated = Box::pin(translate_anthropic_like_stream_to_openai(
        futures::stream::iter(chunks),
        "bedrock-claude".to_string(),
    ));

    let mut collected = Vec::new();
    while let Some(Ok(bytes)) = translated.next().await {
        collected.push(String::from_utf8(bytes.to_vec()).unwrap());
    }

    let full_output = collected.join("");
    assert!(full_output.contains("\"tool_calls\""));
    assert!(full_output.contains("\"weather\""));
    assert!(
        full_output.contains("\"tool_calls\"")
            || full_output.contains("\"finish_reason\":\"tool_calls\"")
    );
}

#[tokio::test]
async fn translate_stream_synthesizes_done_on_eof_after_finish() {
    use futures::StreamExt;

    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{"content":"Hello"},"index":0}],"object":"chat.completion.chunk","created":1,"model":"MiniMax-M2.5","id":"chatcmpl-test"}"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}],"object":"chat.completion.chunk","created":1,"model":"MiniMax-M2.5","id":"chatcmpl-test"}"#,
        )),
    ];

    let mut translated = Box::pin(translate_anthropic_like_stream_to_openai(
        futures::stream::iter(chunks),
        "MiniMax-M2.5".to_string(),
    ));

    let mut collected = String::new();
    while let Some(Ok(bytes)) = translated.next().await {
        collected.push_str(&String::from_utf8(bytes.to_vec()).unwrap());
    }

    assert!(collected.contains("\"content\":\"Hello\""));
    assert!(collected.contains("\"finish_reason\":\"stop\""));
    assert!(collected.contains("data: [DONE]"));
}

#[test]
fn translate_text_delta_to_openai_chunk() {
    let line = br#"data:{"partial":true,"raw_response_json":"{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}"}"#;
    let output = String::from_utf8(
        translate_accio_sse_to_openai(line, "claude-sonnet-4-6", "chatcmpl-test", 1700000000)
            .unwrap(),
    )
    .unwrap();
    let payload: Value = serde_json::from_str(output.trim_start_matches("data: ").trim()).unwrap();
    assert_eq!(payload["choices"][0]["delta"]["content"], "Hello");
}

#[test]
fn unpack_rejects_accio_provider_error_payload() {
    let body = json!({
        "turn_complete": true,
        "error_code": "5015",
        "error_message": "user not activated"
    });

    let err = unpack_accio_response(&body).unwrap_err();
    assert_eq!(err.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(err.http_status, Some(503));
    assert_eq!(err.code.as_deref(), Some("5015"));
    assert_eq!(err.provider_name.as_deref(), Some("accio_compatible"));
    assert!(matches!(
        err.fallback_hint,
        FallbackHint::FallbackProvider { .. }
    ));
    assert!(err.message.contains("Accio account unavailable"));
}

#[test]
fn detect_provider_error_from_wrapped_sse_line() {
    let line =
        br#"data:{"turn_complete":true,"error_code":"5015","error_message":"user not activated"}"#;

    let err = detect_accio_provider_error(line).unwrap();
    assert_eq!(err.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(err.http_status, Some(503));
    assert_eq!(err.code.as_deref(), Some("5015"));
    assert_eq!(err.provider_name.as_deref(), Some("accio_compatible"));
    assert!(matches!(
        err.fallback_hint,
        FallbackHint::FallbackProvider { .. }
    ));
}

#[test]
fn detect_provider_error_from_html_challenge_body() {
    let html = br#"<!DOCTYPE html><html><body><punish-component></punish-component><script src="https://g.alicdn.com/AWSC/CAPTCHA/0.0.1/awsc.js"></script></body></html>"#;

    let err = detect_accio_html_challenge(html).expect("challenge body should be detected");
    assert_eq!(err.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(err.http_status, Some(503));
    assert_eq!(err.code.as_deref(), Some("accio_upstream_html_challenge"));
    assert_eq!(err.provider_name.as_deref(), Some("accio_compatible"));
    assert!(matches!(
        err.fallback_hint,
        FallbackHint::FallbackProvider { .. }
    ));
    assert!(err.message.contains("anti-bot challenge"));
}
