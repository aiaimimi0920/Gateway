use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};

use super::CHATGPT_WEB_REVERSE_ADAPTER;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

mod classification;

pub use classification::{
    classify_chatgpt_web_http_error, response_indicates_browser_challenge,
    response_indicates_session_invalid,
};

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
mod tests;
