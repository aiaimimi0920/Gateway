use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, ContentPart, EndpointKind,
    MessageRole, TokenUsage,
};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

pub const QWEN_WEB_DEFAULT_CREATE_CHAT_PATH: &str = "/api/v2/chats/new";
pub const QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH: &str = "/api/v2/chat/completions";
pub const QWEN_WEB_DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
pub const QWEN_WEB_DEFAULT_SEC_CH_UA: &str =
    "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not A(Brand\";v=\"24\"";
pub const QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7";
pub const QWEN_WEB_DEFAULT_TIMEZONE: &str = "Mon Dec 08 2025 17:28:55 GMT+0800";
pub const QWEN_WEB_DEFAULT_VERSION: &str = "0.1.13";
pub const QWEN_WEB_DEFAULT_BX_VERSION: &str = "2.5.31";
pub const QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE: &str = "qwen_web_browser_challenge_required";
pub const QWEN_WEB_SESSION_INVALID_CODE: &str = "qwen_web_session_invalid";

pub fn pack_create_chat(model: &str) -> Value {
    let timestamp = current_timestamp_secs();
    json!({
        "title": format!("api_{timestamp}"),
        "models": [model],
        "chat_mode": "normal",
        "chat_type": "t2t",
        "timestamp": timestamp,
    })
}

pub fn pack_qwen_web(
    req: &CanonicalRelayRequest,
    model: &str,
    chat_id: &str,
) -> Result<Value, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    ) {
        return Err(GatewayError::bad_request(
            "Qwen Web adapters currently support only text chat endpoints.",
        )
        .with_code("unsupported_qwen_web_endpoint"));
    }
    let combined_text = combine_messages_for_qwen_web(&req.messages)?;
    if combined_text.trim().is_empty() {
        return Err(GatewayError::bad_request(
            "Qwen Web requests require at least one text message part.",
        )
        .with_code("qwen_web_missing_text"));
    }
    let timestamp = current_timestamp_secs();
    let message_id = uuid::Uuid::new_v4().to_string();
    let child_message_id = uuid::Uuid::new_v4().to_string();
    let request_id = uuid::Uuid::new_v4().to_string();
    let session_id = uuid::Uuid::new_v4().to_string();
    let feature_config = json!({
        "thinking_enabled": false,
        "output_schema": "phase",
        "research_mode": "normal",
        "web_search_enabled": false,
        "function_calling": !req.tools.is_empty(),
    });

    Ok(json!({
        "stream": true,
        "version": "2.1",
        "incremental_output": true,
        "chat_id": chat_id,
        "chat_mode": "normal",
        "model": model,
        "parent_id": Value::Null,
        "chat_type": "t2t",
        "messages": [{
            "fid": message_id,
            "parentId": Value::Null,
            "childrenIds": [child_message_id],
            "role": "user",
            "content": combined_text,
            "user_action": "chat",
            "files": [],
            "timestamp": timestamp,
            "models": [model],
            "chat_type": "t2t",
            "feature_config": feature_config,
            "extra": {
                "meta": {
                    "subChatType": "t2t"
                }
            },
            "sub_chat_type": "t2t",
            "parent_id": Value::Null
        }],
        "session_id": session_id,
        "id": request_id,
        "sub_chat_type": "t2t",
    }))
}

pub fn classify_qwen_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    let provider = "qwen_web_compatible";
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "Qwen Web direct replay hit an upstream browser challenge and requires a fresh browser-backed session refresh.",
        )
        .with_provider(provider)
        .with_code(QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }

    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "Qwen Web browser session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(provider)
        .with_code(QWEN_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }

    classify_upstream_error(status, body, Some(provider))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("security checkpoint")
        || lower.contains("verify you are human")
        || lower.contains("verify that you are human")
        || lower.contains("bot verification")
        || lower.contains("bot check")
        || lower.contains("just a moment")
        || lower.contains("waf")
        || lower.contains("x5sec")
        || lower.contains("access denied");

    (status == 403 || status == 429 || status == 503 || status == 200)
        && html_like
        && challenge_like
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("unauthorized")
        || lower.contains("token expired")
        || lower.contains("login")
        || lower.contains("log in")
        || lower.contains("sign in")
        || lower.contains("session expired")
        || lower.contains("invalid token")
        || lower.contains("\"code\":401")
        || lower.contains("\"status\":401");

    matches!(status, 401 | 403) && (auth_like || (html_like && auth_like))
}

fn combine_messages_for_qwen_web(messages: &[CanonicalMessage]) -> Result<String, GatewayError> {
    let mut rendered = Vec::new();
    for message in messages {
        let mut parts = Vec::new();
        for part in &message.content {
            match part {
                ContentPart::Text { text } => {
                    if !text.trim().is_empty() {
                        parts.push(text.trim().to_string());
                    }
                }
                ContentPart::ImageUrl { .. } => {
                    return Err(GatewayError::bad_request(
                        "Qwen Web adapters do not currently support image content.",
                    )
                    .with_code("unsupported_qwen_web_image"));
                }
                ContentPart::Json { .. } | ContentPart::Raw { .. } => {
                    return Err(GatewayError::bad_request(
                        "Qwen Web adapters do not currently support structured non-text content.",
                    )
                    .with_code("unsupported_qwen_web_non_text"));
                }
            }
        }
        let text = parts.join("\n");
        if text.is_empty() {
            continue;
        }
        if messages.len() == 1 && matches!(message.role, MessageRole::User) {
            rendered.push(text);
        } else {
            rendered.push(format!("{}:{}", qwen_role_label(message.role), text));
        }
    }
    Ok(rendered.join(";"))
}

fn qwen_role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

pub async fn accumulate_qwen_web_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = response
        .text()
        .await
        .map_err(|error| GatewayError::server_error(format!("read qwen web body: {error}")))?;
    let mut parser = SseParseState::new();
    let mut text = String::new();
    let mut reported_model = model.to_string();
    let mut usage: Option<TokenUsage> = None;
    let mut finish_reason: Option<String> = Some("stop".to_string());

    for raw_line in body.lines() {
        if let Some(frame) = parse_sse_line(raw_line, &mut parser) {
            if frame.data == "[DONE]" {
                break;
            }
            let data: Value = match serde_json::from_str(&frame.data) {
                Ok(value) => value,
                Err(_) => continue,
            };

            if let Some(candidate_model) = data.get("model").and_then(|value| value.as_str()) {
                reported_model = candidate_model.to_string();
            }

            if let Some(token_usage) = parse_token_usage(data.get("usage")) {
                usage = Some(token_usage);
            }

            if let Some(choice) = data
                .get("choices")
                .and_then(|value| value.as_array())
                .and_then(|choices| choices.first())
            {
                if let Some(reason) = choice.get("finish_reason").and_then(|value| value.as_str()) {
                    finish_reason = Some(reason.to_string());
                }
                if let Some(content) = extract_delta_text(choice) {
                    text.push_str(content);
                }
            }
        }
    }

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls: Vec::new(),
        upstream_status: Some(200),
        finish_reason,
    })
}

fn parse_token_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let usage = value?;
    let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64())?;
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_default();
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens + completion_tokens);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

fn extract_delta_text(choice: &Value) -> Option<&str> {
    let delta = choice.get("delta")?;
    let phase = delta.get("phase").and_then(|value| value.as_str());
    if matches!(phase, Some("think") | Some("thinking_summary")) {
        return None;
    }
    delta.get("content").and_then(|value| value.as_str())
}

pub fn translate_qwen_web_frame_to_openai_sse(
    data_str: &str,
    model: &str,
    response_id: &str,
    created: i64,
) -> Option<Vec<u8>> {
    if data_str.trim().is_empty() {
        return None;
    }
    if data_str.trim() == "[DONE]" {
        return Some(b"data: [DONE]\n\n".to_vec());
    }
    let data: Value = serde_json::from_str(data_str).ok()?;
    let choice = data
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())?;
    let content = extract_delta_text(choice);
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    if content.is_none() && finish_reason.is_none() {
        return None;
    }

    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": data.get("model").and_then(|value| value.as_str()).unwrap_or(model),
        "choices": [{
            "index": 0,
            "delta": content.map(|value| json!({ "content": value })).unwrap_or_else(|| json!({})),
            "finish_reason": finish_reason,
        }]
    });

    Some(format!("data: {chunk}\n\n").into_bytes())
}

pub fn translate_qwen_web_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let state = QwenWebTranslatorState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        model,
        response_id,
        created,
        emitted_done: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(pos) = st.buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = st.buffer.drain(..=pos).collect();
                    let line_str = match std::str::from_utf8(&line) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let normalized_line = line_str.trim_end_matches('\n');
                    if let Some(frame) = parse_sse_line(normalized_line, &mut st.parser) {
                        if let Some(translated) = translate_qwen_web_frame_to_openai_sse(
                            &frame.data,
                            &st.model,
                            &st.response_id,
                            st.created,
                        ) {
                            if translated == b"data: [DONE]\n\n".to_vec() {
                                st.emitted_done = true;
                                return Some((Ok(Bytes::from(translated)), (stream, st, true)));
                            }
                            return Some((Ok(Bytes::from(translated)), (stream, st, false)));
                        }
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => st.buffer.extend_from_slice(&chunk),
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
                    None => {
                        if !st.emitted_done {
                            st.emitted_done = true;
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (stream, st, true),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

#[derive(Debug)]
struct QwenWebTranslatorState {
    buffer: Vec<u8>,
    parser: SseParseState,
    model: String,
    response_id: String,
    created: i64,
    emitted_done: bool,
}

fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalTool, ContentPart, ProtocolFamily};
    use futures::StreamExt;

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
}
