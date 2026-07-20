use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use super::{
    CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, CHATGPT_WEB_REVERSE_ADAPTER,
    CHATGPT_WEB_SESSION_INVALID_CODE,
};
use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

pub fn classify_chatgpt_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "ChatGPT Web reverse hit an upstream browser challenge and requires a refreshed browser session or different network path.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }
    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "ChatGPT Web reverse session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code(CHATGPT_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }
    classify_upstream_error(status, body, Some(CHATGPT_WEB_REVERSE_ADAPTER))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("cloudflare")
        || lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("verify you are human")
        || lower.contains("just a moment")
        || lower.contains("attention required");
    matches!(status, 403 | 429 | 503 | 200) && html_like && challenge_like
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("unauthorized")
        || lower.contains("token expired")
        || lower.contains("log in")
        || lower.contains("sign in")
        || lower.contains("session expired")
        || lower.contains("invalid token")
        || lower.contains("not logged in");
    matches!(status, 401 | 403) && (auth_like || (html_like && auth_like))
}

pub fn accumulate_response(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut parser = SseParseState::new();
    let mut state = ConversationState::default();
    let mut saw_frame = false;
    for raw_line in body.lines() {
        if let Some(frame) = parse_sse_line(raw_line, &mut parser) {
            saw_frame = true;
            let data = frame.data.trim();
            if data == "[DONE]" {
                break;
            }
            if data == "\"v1\"" || data == "v1" {
                continue;
            }
            match serde_json::from_str::<Value>(data) {
                Ok(value) => process_event(&value, &mut state),
                Err(_) => {
                    if !data.is_empty() && !is_non_text_control_frame(data) {
                        state.append_text(data);
                    }
                }
            }
        }
    }
    if !saw_frame {
        return Err(GatewayError::server_error(
            "ChatGPT Web reverse response did not contain any parseable SSE frames.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_invalid_sse"));
    }
    if state.blocked && state.text.trim().is_empty() {
        return Err(GatewayError::bad_request(
            "ChatGPT Web reverse request was blocked by upstream moderation or policy filters.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_blocked"));
    }
    if state.text.trim().is_empty() {
        return Err(GatewayError::server_error(
            "ChatGPT Web reverse SSE stream ended without any caller-visible assistant text.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_missing_text"));
    }
    Ok(CanonicalRelayResponse {
        model: model.to_string(),
        text: state.text.trim().to_string(),
        usage: None::<TokenUsage>,
        tool_calls: Vec::new(),
        upstream_status: None,
        finish_reason: Some("stop".to_string()),
    })
}

pub fn translate_to_openai_sse(body: &str, model: &str) -> Result<Vec<Bytes>, GatewayError> {
    let canonical = accumulate_response(body, model)?;
    let response_id = format!("chatcmpl_chatgptweb_{}", uuid::Uuid::new_v4().simple());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": canonical.model,
        "choices": [{
            "index": 0,
            "delta": { "content": canonical.text },
            "finish_reason": canonical.finish_reason,
        }]
    });
    Ok(vec![
        Bytes::from(format!("data: {chunk}\n\n")),
        Bytes::from_static(b"data: [DONE]\n\n"),
    ])
}

pub fn translate_chatgpt_web_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("chatcmpl_chatgptweb_{}", uuid::Uuid::new_v4().simple());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let state = ChatGptWebTranslatorState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        model,
        response_id,
        created,
        emitted_stop: false,
        emitted_done: false,
        conversation: ConversationState::default(),
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut state, done)| async move {
            if done {
                return None;
            }

            loop {
                if let Some(pos) = state.buffer.iter().position(|&byte| byte == b'\n') {
                    let line: Vec<u8> = state.buffer.drain(..=pos).collect();
                    let line_str = match std::str::from_utf8(&line) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let normalized_line = line_str.trim_end_matches('\n');
                    if let Some(frame) = parse_sse_line(normalized_line, &mut state.parser) {
                        let data = frame.data.trim();
                        if data == "[DONE]" {
                            if let Some(stop_chunk) = state.finish_chunk() {
                                return Some((Ok(stop_chunk), (stream, state, false)));
                            }
                            if !state.emitted_done {
                                state.emitted_done = true;
                                return Some((
                                    Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                    (stream, state, true),
                                ));
                            }
                            return None;
                        }
                        if let Some(chunk) = state.process_frame(data) {
                            return Some((Ok(chunk), (stream, state, false)));
                        }
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => state.buffer.extend_from_slice(&chunk),
                    Some(Err(error)) => return Some((Err(error), (stream, state, true))),
                    None => {
                        if let Some(stop_chunk) = state.finish_chunk() {
                            return Some((Ok(stop_chunk), (stream, state, false)));
                        }
                        if !state.emitted_done {
                            state.emitted_done = true;
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (stream, state, true),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

#[derive(Default)]
struct ConversationState {
    text: String,
    conversation_id: Option<String>,
    blocked: bool,
}

struct ChatGptWebTranslatorState {
    buffer: Vec<u8>,
    parser: SseParseState,
    model: String,
    response_id: String,
    created: i64,
    emitted_stop: bool,
    emitted_done: bool,
    conversation: ConversationState,
}

impl ChatGptWebTranslatorState {
    fn process_frame(&mut self, data: &str) -> Option<Bytes> {
        if data.is_empty() || data == "\"v1\"" || data == "v1" {
            return None;
        }

        let before = self.conversation.text.clone();
        match serde_json::from_str::<Value>(data) {
            Ok(value) => process_event(&value, &mut self.conversation),
            Err(_) if !is_non_text_control_frame(data) => self.conversation.append_text(data),
            Err(_) => {}
        }

        let delta = text_delta(&before, &self.conversation.text)?;
        Some(self.content_chunk(&delta, None))
    }

    fn finish_chunk(&mut self) -> Option<Bytes> {
        if self.emitted_stop {
            return None;
        }
        self.emitted_stop = true;
        Some(self.content_chunk("", Some("stop")))
    }

    fn content_chunk(&self, content: &str, finish_reason: Option<&str>) -> Bytes {
        let delta = if content.is_empty() {
            json!({})
        } else {
            json!({ "content": content })
        };
        let chunk = json!({
            "id": self.response_id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": self.model,
            "choices": [{
                "index": 0,
                "delta": delta,
                "finish_reason": finish_reason,
            }]
        });
        Bytes::from(format!("data: {chunk}\n\n"))
    }
}

fn is_plain_status_frame(data: &str) -> bool {
    matches!(
        data.trim().to_ascii_lowercase().as_str(),
        "finished_successfully" | "in_progress" | "finished_partial_completion" | "failed"
    )
}

fn is_non_text_control_frame(data: &str) -> bool {
    is_plain_status_frame(data)
        || is_web_annotation_control_frame(data)
        || is_done_marker_frame(data)
}

fn is_web_annotation_control_frame(data: &str) -> bool {
    let trimmed = data.trim();
    trimmed.starts_with('\u{e200}') && trimmed.contains("entity")
}

fn is_done_marker_frame(data: &str) -> bool {
    let trimmed = data.trim();
    let Some(rest) = trimmed.strip_prefix("done") else {
        return false;
    };
    rest.len() >= 8
        && rest
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn text_delta(previous: &str, current: &str) -> Option<String> {
    let previous = previous.trim_end_matches('\n');
    let current = current.trim_end_matches('\n');
    if current.is_empty() || current == previous {
        return None;
    }
    if let Some(suffix) = current.strip_prefix(previous) {
        return (!suffix.is_empty()).then(|| suffix.to_string());
    }
    Some(current.to_string())
}

impl ConversationState {
    fn append_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        self.text.push_str(value);
    }

    fn replace_text(&mut self, value: &str) {
        self.text.clear();
        self.text.push_str(value);
    }
}

fn process_event(value: &Value, state: &mut ConversationState) {
    match value {
        Value::String(text) => {
            let text = text.trim();
            if !text.is_empty() && text != "v1" && !is_non_text_control_frame(text) {
                state.append_text(text);
            }
        }
        Value::Array(items) => {
            for item in items {
                process_event(item, state);
            }
        }
        Value::Object(map) => {
            if let Some(conversation_id) = map.get("conversation_id").and_then(Value::as_str) {
                if !conversation_id.trim().is_empty() {
                    state.conversation_id = Some(conversation_id.trim().to_string());
                }
            }
            if map.get("type").and_then(Value::as_str) == Some("moderation") {
                if map
                    .get("moderation_response")
                    .and_then(Value::as_object)
                    .and_then(|record| record.get("blocked"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    state.blocked = true;
                }
            }
            if let Some(message) = map.get("message") {
                extract_message_text(message, state);
            }
            if let Some(v) = map.get("v") {
                if let Some(conversation_id) = v
                    .as_object()
                    .and_then(|record| record.get("conversation_id"))
                    .and_then(Value::as_str)
                {
                    if !conversation_id.trim().is_empty() {
                        state.conversation_id = Some(conversation_id.trim().to_string());
                    }
                }
                extract_message_text(v, state);
            }

            let path = map.get("p").and_then(Value::as_str).unwrap_or_default();
            let op = map.get("o").and_then(Value::as_str).unwrap_or_default();
            if !path.is_empty() || !op.is_empty() {
                process_patch(path, op, map.get("v"), state);
            } else if let Some(v) = map.get("v") {
                process_event(v, state);
            }
        }
        _ => {}
    }
}

fn process_patch(path: &str, op: &str, value: Option<&Value>, state: &mut ConversationState) {
    match op {
        "append" if path == "/message/content/parts/0" => {
            if let Some(text) = value.and_then(Value::as_str) {
                state.append_text(text);
            }
        }
        "replace" | "add" if path == "/message/content/parts/0" => {
            if let Some(text) = value.and_then(Value::as_str) {
                state.replace_text(text);
            }
        }
        "patch" => {
            if let Some(Value::Array(items)) = value {
                for item in items {
                    process_event(item, state);
                }
            }
        }
        _ => {
            if let Some(value) = value {
                process_event(value, state);
            }
        }
    }
}

fn extract_message_text(value: &Value, state: &mut ConversationState) {
    let Some(record) = value.as_object() else {
        return;
    };
    let Some(message) = record
        .get("message")
        .filter(|inner| inner.is_object())
        .or(Some(value))
    else {
        return;
    };
    let Some(message_map) = message.as_object() else {
        return;
    };
    let role = message_map
        .get("author")
        .and_then(Value::as_object)
        .and_then(|author| author.get("role"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if role != "assistant" {
        return;
    }
    let parts = message_map
        .get("content")
        .and_then(Value::as_object)
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array);
    let Some(parts) = parts else {
        return;
    };
    let text = parts
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if !text.is_empty() {
        state.replace_text(&text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_chatgpt_web_http_error_detects_browser_challenge() {
        let error = classify_chatgpt_web_http_error(
            403,
            Some("text/html; charset=utf-8"),
            "<!doctype html><html><body>Just a moment... verify you are human</body></html>",
        );
        assert_eq!(
            error.code.as_deref(),
            Some(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE)
        );
        assert_eq!(error.http_status, Some(403));
    }

    #[test]
    fn classify_chatgpt_web_http_error_detects_session_invalid() {
        let error = classify_chatgpt_web_http_error(
            401,
            Some("text/html; charset=utf-8"),
            "<!doctype html><html><body>session expired, please log in again</body></html>",
        );
        assert_eq!(
            error.code.as_deref(),
            Some(CHATGPT_WEB_SESSION_INVALID_CODE)
        );
        assert_eq!(error.http_status, Some(401));
    }

    #[test]
    fn accumulate_response_errors_when_sse_frames_are_missing() {
        let error = accumulate_response("plain text only", "gpt-5").expect_err("invalid sse");
        assert_eq!(error.code.as_deref(), Some("chatgpt_web_invalid_sse"));
    }

    #[test]
    fn accumulate_response_errors_when_stream_is_blocked() {
        let body = concat!(
            "data: {\"type\":\"moderation\",\"moderation_response\":{\"blocked\":true}}\n\n",
            "data: [DONE]\n\n"
        );
        let error = accumulate_response(body, "gpt-5").expect_err("blocked");
        assert_eq!(error.code.as_deref(), Some("chatgpt_web_blocked"));
    }

    #[test]
    fn accumulate_response_errors_when_text_never_arrives() {
        let body = concat!("data: \"v1\"\n\n", "data: [DONE]\n\n");
        let error = accumulate_response(body, "gpt-5").expect_err("missing text");
        assert_eq!(error.code.as_deref(), Some("chatgpt_web_missing_text"));
    }

    #[test]
    fn accumulate_response_extracts_assistant_delta_text() {
        let body = concat!(
            "data: \"v1\"\n\n",
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Hello\"}\n\n",
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n\n",
            "data: [DONE]\n\n"
        );
        let response = accumulate_response(body, "gpt-5").expect("canonical");
        assert_eq!(response.text, "Hello world");
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn accumulate_response_ignores_plain_status_frames() {
        let body = concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
            "data: finished_successfully\n\n",
            "data: [DONE]\n\n"
        );
        let response = accumulate_response(body, "gpt-5").expect("canonical");
        assert_eq!(response.text, "Paris");
    }

    #[test]
    fn accumulate_response_ignores_json_string_status_frames() {
        let body = concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
            "data: \"finished_successfully\"\n\n",
            "data: [DONE]\n\n"
        );
        let response = accumulate_response(body, "gpt-5").expect("canonical");
        assert_eq!(response.text, "Paris");
    }

    #[test]
    fn accumulate_response_ignores_web_annotation_control_frames() {
        let body = concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
            "data: \"\u{e200}entity\u{e202}[\\\"city\\\",\\\"Paris\\\"]\u{e201}\"\n\n",
            "data: \"doneE-D1snLkeVaVIyv6\"\n\n",
            "data: [DONE]\n\n"
        );
        let response = accumulate_response(body, "gpt-5").expect("canonical");
        assert_eq!(response.text, "Paris");
    }

    #[test]
    fn accumulate_response_replaces_text_from_nested_assistant_message() {
        let body = concat!(
            "data: {\"message\":{\"author\":{\"role\":\"assistant\"},\"content\":{\"parts\":[\"draft\"]}}}\n\n",
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"replace\",\"v\":\"final\"}\n\n",
            "data: [DONE]\n\n"
        );
        let response = accumulate_response(body, "gpt-5").expect("canonical");
        assert_eq!(response.text, "final");
    }

    #[test]
    fn translate_to_openai_sse_serializes_text_and_done_chunk() {
        let body = concat!(
            "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"hello\"}\n\n",
            "data: [DONE]\n\n"
        );
        let chunks = translate_to_openai_sse(body, "gpt-5").expect("chunks");
        assert_eq!(chunks.len(), 2);
        let first = std::str::from_utf8(chunks[0].as_ref()).expect("utf8");
        assert!(first.contains("\"content\":\"hello\""));
        let second = std::str::from_utf8(chunks[1].as_ref()).expect("utf8");
        assert_eq!(second, "data: [DONE]\n\n");
    }

    #[tokio::test]
    async fn translate_chatgpt_web_stream_emits_incremental_chunks_and_done() {
        let upstream = futures::stream::iter(vec![
            Ok(Bytes::from_static(
                b"event: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"hello\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"event: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n\n",
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ]);

        let chunks = translate_chatgpt_web_stream(upstream, "gpt-5.4".to_string())
            .collect::<Vec<_>>()
            .await;
        let rendered = chunks
            .into_iter()
            .map(|item| String::from_utf8(item.expect("ok").to_vec()).expect("utf8"))
            .collect::<Vec<_>>();

        assert!(rendered
            .iter()
            .any(|item| item.contains("\"content\":\"hello\"")));
        assert!(rendered
            .iter()
            .any(|item| item.contains("\"content\":\" world\"")));
        assert!(rendered
            .iter()
            .any(|item| item.contains("\"finish_reason\":\"stop\"")));
        assert_eq!(
            rendered.last().map(String::as_str),
            Some("data: [DONE]\n\n")
        );
    }

    #[tokio::test]
    async fn translate_chatgpt_web_stream_handles_fragmented_sse_lines() {
        let upstream = futures::stream::iter(vec![
            Ok(Bytes::from_static(b"event: message\ndata: {\"p\":\"/message/")),
            Ok(Bytes::from_static(
                b"content/parts/0\",\"o\":\"append\",\"v\":\"hel",
            )),
            Ok(Bytes::from_static(
                b"lo\"}\n\nevent: message\ndata: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\" world\"}\n",
            )),
            Ok(Bytes::from_static(b"\n")),
            Ok(Bytes::from_static(b"data: [DO")),
            Ok(Bytes::from_static(b"NE]\n\n")),
        ]);

        let chunks = translate_chatgpt_web_stream(upstream, "gpt-5.4".to_string())
            .collect::<Vec<_>>()
            .await;
        let rendered = chunks
            .into_iter()
            .map(|item| String::from_utf8(item.expect("ok").to_vec()).expect("utf8"))
            .collect::<Vec<_>>();

        assert!(rendered
            .iter()
            .any(|item| item.contains("\"content\":\"hello\"")));
        assert!(rendered
            .iter()
            .any(|item| item.contains("\"content\":\" world\"")));
        assert_eq!(
            rendered
                .iter()
                .filter(|item| item.contains("\"finish_reason\":\"stop\""))
                .count(),
            1
        );
        assert_eq!(
            rendered
                .iter()
                .filter(|item| item.as_str() == "data: [DONE]\n\n")
                .count(),
            1
        );
    }
}
