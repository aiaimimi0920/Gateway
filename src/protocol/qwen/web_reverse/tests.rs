use super::accumulation::accumulate_qwen_web_body;
use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, EndpointKind, MessageRole,
};
use crate::protocol::canonical::{CanonicalTool, ContentPart, ProtocolFamily};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;

fn text_message(role: MessageRole, text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role,
        content: vec![ContentPart::Text {
            text: text.to_string(),
        }],
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    }
}

fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("qwen3-coder-plus".to_string()),
        stream: false,
        messages,
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

fn weather_tool() -> CanonicalTool {
    CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("weather".to_string()),
        description: Some("Return weather.".to_string()),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "city": {"type": "string"}
            },
            "required": ["city"]
        })),
        raw: std::collections::HashMap::new(),
    }
}

#[test]
fn pack_qwen_web_collapses_multi_turn_history() {
    let req = make_request(vec![
        text_message(MessageRole::System, "You are helpful."),
        text_message(MessageRole::User, "Hello"),
        text_message(MessageRole::Assistant, "Hi"),
        text_message(MessageRole::User, "Please help"),
    ]);
    let body = pack_qwen_web(&req, "qwen3-coder-plus", "chat-1").unwrap();
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(content.contains("system:You are helpful."));
    assert!(content.contains("user:Hello"));
    assert!(content.contains("assistant:Hi"));
    assert!(content.contains("user:Please help"));
}

#[test]
fn pack_qwen_web_allows_tool_requests_after_text_bridge_injection() {
    let mut req = make_request(vec![text_message(
        MessageRole::User,
        "Use only the weather tool for Hangzhou.",
    )]);
    req.tools = vec![weather_tool()];

    let body = pack_qwen_web(&req, "qwen3-coder-plus", "chat-1").unwrap();
    let content = body["messages"][0]["content"].as_str().unwrap();
    assert!(content.contains("Use only the weather tool for Hangzhou."));
    assert_eq!(
        body["messages"][0]["feature_config"]["function_calling"].as_bool(),
        Some(true)
    );
    assert_eq!(body["version"].as_str(), Some("2.1"));
    assert_eq!(body["chat_id"].as_str(), Some("chat-1"));
}

#[test]
fn translate_qwen_web_frame_skips_thinking_tokens() {
    let translated = translate_qwen_web_frame_to_openai_sse(
        r#"{"choices":[{"delta":{"content":"internal","phase":"think"},"finish_reason":null}]}"#,
        "qwen3-coder-plus",
        "chatcmpl-test",
        1700000000,
    );
    assert!(translated.is_none());
}

#[test]
fn translate_qwen_web_frame_emits_answer_tokens() {
    let translated = translate_qwen_web_frame_to_openai_sse(
        r#"{"choices":[{"delta":{"content":"hello","phase":"answer"},"finish_reason":null}]}"#,
        "qwen3-coder-plus",
        "chatcmpl-test",
        1700000000,
    )
    .unwrap();
    let text = String::from_utf8(translated).unwrap();
    assert!(text.contains("\"content\":\"hello\""));
}

#[test]
fn accumulate_qwen_web_body_flushes_final_sse_frame_at_eof() {
    let response = accumulate_qwen_web_body(
        r#"data: {"model":"qwen3-coder-plus","choices":[{"delta":{"content":"巴黎","phase":"answer"},"finish_reason":"stop"}]}"#,
        "qwen3-coder-plus",
    )
    .unwrap();
    assert_eq!(response.text, "巴黎");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}

#[test]
fn accumulate_qwen_web_body_accepts_regular_message_json() {
    let response = accumulate_qwen_web_body(
        r#"{"model":"qwen3-coder-plus","choices":[{"message":{"role":"assistant","content":"法国的首都是巴黎。"},"finish_reason":"stop"}]}"#,
        "qwen3-coder-plus",
    )
    .unwrap();
    assert_eq!(response.text, "法国的首都是巴黎。");
}

#[test]
fn accumulate_qwen_web_body_accepts_data_lines_without_blank_delimiters() {
    let response = accumulate_qwen_web_body(
        concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"法国的首都\",\"phase\":\"answer\"},\"finish_reason\":null}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"是巴黎。\",\"phase\":\"answer\"},\"finish_reason\":\"stop\"}]}\n",
            "data: [DONE]\n",
        ),
        "qwen3-coder-plus",
    )
    .unwrap();
    assert_eq!(response.text, "法国的首都是巴黎。");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}

#[test]
fn accumulate_qwen_web_body_rejects_empty_success() {
    let error = accumulate_qwen_web_body(
        r#"{"choices":[{"message":{"role":"assistant","content":""},"finish_reason":"stop"}]}"#,
        "qwen3-coder-plus",
    )
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("qwen_web_empty_response"));
}

#[test]
fn classify_qwen_web_html_challenge_as_browser_challenge() {
    let err = classify_qwen_web_http_error(
        403,
        Some("text/html; charset=utf-8"),
        "<!DOCTYPE html><html><title>Security checkpoint</title><body>Please verify you are human</body></html>",
    );
    assert_eq!(
        err.code.as_deref(),
        Some(QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
    );
    assert_eq!(err.provider_name.as_deref(), Some("qwen_web_compatible"));
}

#[test]
fn classify_qwen_web_rgv587_json_as_browser_challenge() {
    let body = r#"{"ret":["RGV587_ERROR::SM"],"data":{"url":"https://challenge.invalid/"}}"#;
    assert!(response_indicates_browser_challenge(
        200,
        Some("application/json"),
        body,
    ));
    let error = classify_qwen_web_http_error(200, Some("application/json"), body);
    assert_eq!(
        error.code.as_deref(),
        Some(QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
    );
}

#[test]
fn classify_qwen_web_auth_failure_as_session_invalid() {
    let err = classify_qwen_web_http_error(
        401,
        Some("application/json"),
        r#"{"message":"token expired","code":401}"#,
    );
    assert_eq!(err.code.as_deref(), Some(QWEN_WEB_SESSION_INVALID_CODE));
    assert_eq!(err.provider_name.as_deref(), Some("qwen_web_compatible"));
}

#[tokio::test]
async fn translate_qwen_web_stream_emits_done() {
    let upstream = futures::stream::iter(vec![Ok(Bytes::from(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hello\",\"phase\":\"answer\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n",
    ))]);
    let chunks = translate_qwen_web_stream(upstream, "qwen3-coder-plus".to_string())
        .collect::<Vec<_>>()
        .await;
    let joined = chunks
        .into_iter()
        .map(|item| String::from_utf8(item.unwrap().to_vec()).unwrap())
        .collect::<String>();
    assert!(joined.contains("\"content\":\"hello\""));
    assert!(joined.contains("data: [DONE]"));
}
