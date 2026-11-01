use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use serde_json::{json, Value};

use super::CHATGPT_WEB_REVERSE_ADAPTER;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};

mod classification;
mod stream;

pub use stream::translate_chatgpt_web_stream;

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
    let mut saw_done = false;
    for raw_line in body.lines() {
        if let Some(frame) = parse_sse_line(raw_line, &mut parser) {
            saw_frame = true;
            let data = frame.data.trim();
            if data == "[DONE]" {
                saw_done = true;
                break;
            }
            if data == "\"v1\"" || data == "v1" {
                continue;
            }
            state.observe_status(data);
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
    let finish_reason = state.finish_reason(saw_done)?;
    Ok(CanonicalRelayResponse {
        model_declaration: None,
        model: model.to_string(),
        text: state.text.trim().to_string(),
        usage: None::<TokenUsage>,
        tool_calls: Vec::new(),
        upstream_status: None,
        finish_reason: Some(finish_reason.to_string()),
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

#[derive(Default)]
struct ConversationState {
    text: String,
    conversation_id: Option<String>,
    blocked: bool,
    finished: bool,
    partial: bool,
    failed: bool,
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
    fn failure(&self) -> Option<GatewayError> {
        if self.blocked {
            Some(GatewayError::bad_request(
                "ChatGPT Web reverse request was blocked by upstream moderation or policy filters.",
            ).with_provider(CHATGPT_WEB_REVERSE_ADAPTER).with_code("chatgpt_web_blocked"))
        } else if self.failed {
            Some(
                GatewayError::server_error(
                    "ChatGPT Web reverse upstream reported a failed generation.",
                )
                .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
                .with_code("chatgpt_web_generation_failed"),
            )
        } else {
            None
        }
    }

    fn finish_reason(&self, saw_done: bool) -> Result<&'static str, GatewayError> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        if self.text.trim().is_empty() {
            return Err(GatewayError::server_error(
                "ChatGPT Web reverse SSE stream ended without any caller-visible assistant text.",
            )
            .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
            .with_code("chatgpt_web_missing_text"));
        }
        // HTTP EOF alone does not prove a completed model answer.
        if !saw_done && !self.finished && !self.partial {
            return Err(GatewayError::server_error(
                "ChatGPT Web reverse SSE stream ended before its terminal event.",
            )
            .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
            .with_code("chatgpt_web_incomplete_stream"));
        }
        Ok(if self.partial { "length" } else { "stop" })
    }

    fn observe_status(&mut self, status: &str) {
        // Failure/partial status is sticky; a later transport terminator cannot turn it into success.
        match status.trim() {
            "finished_successfully" => self.finished = true,
            "finished_partial_completion" => self.partial = true,
            "failed" => self.failed = true,
            _ => {}
        }
    }

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
            state.observe_status(text);
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
            if map.get("error").is_some_and(|error| !error.is_null()) {
                state.failed = true;
            }
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
        "replace" | "add" if path == "/message/status" => {
            if let Some(status) = value.and_then(Value::as_str) {
                state.observe_status(status);
            }
        }
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
    if let Some(status) = message_map.get("status").and_then(Value::as_str) {
        state.observe_status(status);
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

#[cfg(test)]
mod stream_tests;
