//! Bounded SSE decoder: phase completion is not response completion; errors win over done.
use super::failure;
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::canonical::TokenUsage,
};
use bytes::{Bytes, BytesMut};
use serde_json::Value;

pub(super) enum Action {
    Text(String),
    Done,
}
pub(super) struct Decoder {
    buffer: BytesMut,
    data: String,
    total: usize,
    frames: usize,
    text: usize,
    done: bool,
    pending_done: bool,
    answered: bool,
    chat: String,
    response: Option<String>,
    pub usage: Option<TokenUsage>,
}
impl Decoder {
    pub fn new(chat: String) -> Self {
        Self {
            buffer: BytesMut::new(),
            data: String::new(),
            total: 0,
            frames: 0,
            text: 0,
            done: false,
            pending_done: false,
            answered: false,
            chat,
            response: None,
            usage: None,
        }
    }
    pub fn feed(&mut self, bytes: Bytes) -> Result<(), GatewayError> {
        self.total = self.total.saturating_add(bytes.len());
        if self.done || self.total > 8 * 1024 * 1024 {
            return Err(limit());
        }
        self.buffer.extend_from_slice(&bytes);
        Ok(())
    }
    pub fn end(&mut self) {
        self.buffer.extend_from_slice(b"\n\n");
    }
    pub fn next(&mut self) -> Result<Option<Action>, GatewayError> {
        if self.pending_done {
            self.pending_done = false;
            return self.complete().map(Some);
        }
        while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            if pos > 1024 * 1024 {
                return Err(limit());
            }
            let bytes = self.buffer.split_to(pos + 1);
            let line = std::str::from_utf8(&bytes[..pos])
                .map_err(|_| protocol())?
                .trim_end_matches('\r');
            if line.is_empty() {
                if !self.data.is_empty() {
                    let data = std::mem::take(&mut self.data);
                    if let Some(action) = self.frame(&data)? {
                        return Ok(Some(action));
                    }
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(data.strip_prefix(' ').unwrap_or(data));
                if self.data.len() > 1024 * 1024 {
                    return Err(limit());
                }
            } else if !line.starts_with(':')
                && !["event:", "id:", "retry:"]
                    .iter()
                    .any(|p| line.starts_with(p))
            {
                return Err(protocol());
            }
        }
        if self.buffer.len() > 1024 * 1024 {
            return Err(limit());
        }
        Ok(None)
    }
    fn frame(&mut self, raw: &str) -> Result<Option<Action>, GatewayError> {
        self.frames += 1;
        if self.frames > 16384 {
            return Err(limit());
        }
        if raw.trim() == "[DONE]" {
            return self.complete().map(Some);
        }
        let value: Value = serde_json::from_str(raw).map_err(|_| protocol())?;
        if !value.is_object() {
            return Err(protocol());
        }
        if value.get("error").is_some_and(|v| !v.is_null()) || value["success"] == false {
            return Err(failure(
                "qwen_web_current_upstream_failed",
                ErrorKind::ServerError,
            ));
        }
        if value
            .get("response.stopped")
            .is_some_and(|v| !v.is_null() && v != false)
        {
            return Err(failure("qwen_web_current_stopped", ErrorKind::ServerError));
        }
        if let Some(created) = value.get("response.created") {
            if !created.is_object()
                || created
                    .get("chat_id")
                    .is_some_and(|v| v.as_str() != Some(&self.chat))
            {
                return Err(protocol());
            }
            self.bind(created.get("response_id"))?;
        }
        self.bind(value.get("response_id"))?;
        if let Some(usage) = value.get("usage").filter(|v| !v.is_null()) {
            self.usage = token_usage(usage)?;
        }
        let terminal = match value.get("done") {
            Some(Value::Bool(v)) => *v,
            None => false,
            _ => return Err(protocol()),
        };
        let mut action = None;
        if let Some(choices) = value.get("choices") {
            let choices = choices.as_array().ok_or_else(protocol)?;
            if choices.len() > 1 {
                return Err(protocol());
            }
            if let Some(choice) = choices.first() {
                if choice.get("index").is_some_and(|v| v.as_u64() != Some(0))
                    || choice
                        .get("finish_reason")
                        .filter(|v| !v.is_null())
                        .is_some_and(|v| !matches!(v.as_str(), Some("stop" | "length")))
                {
                    return Err(protocol());
                }
                let delta = choice
                    .get("delta")
                    .filter(|d| d.is_object())
                    .ok_or_else(protocol)?;
                if delta["status"] == "error" {
                    return Err(failure(
                        "qwen_web_current_upstream_failed",
                        ErrorKind::ServerError,
                    ));
                }
                if delta.get("function_call").is_some_and(|v| !v.is_null())
                    || delta.get("tool_calls").is_some_and(|v| !v.is_null())
                    || delta
                        .get("role")
                        .filter(|v| !v.is_null())
                        .is_some_and(|v| !matches!(v.as_str(), Some("" | "assistant")))
                    || delta
                        .get("extra")
                        .filter(|v| !v.is_null())
                        .is_some_and(|v| !v.as_object().is_some_and(|o| o.is_empty()))
                    || delta.as_object().unwrap().keys().any(|k| {
                        !matches!(
                            k.as_str(),
                            "content"
                                | "role"
                                | "phase"
                                | "status"
                                | "extra"
                                | "function_call"
                                | "tool_calls"
                        )
                    })
                {
                    return Err(failure(
                        "qwen_web_current_unsupported_output",
                        ErrorKind::ServerError,
                    ));
                }
                for field in ["phase", "status"] {
                    if delta
                        .get(field)
                        .is_some_and(|v| !v.is_null() && !v.is_string())
                    {
                        return Err(protocol());
                    }
                }
                let phase = delta["phase"].as_str().unwrap_or("");
                if !matches!(phase, "" | "answer" | "think" | "thinking_summary") {
                    return Err(protocol());
                }
                if matches!(phase, "" | "answer") {
                    if let Some(text) = delta.get("content").filter(|v| !v.is_null()) {
                        let text = text.as_str().ok_or_else(protocol)?;
                        self.text = self.text.saturating_add(text.len());
                        self.answered |= !text.trim().is_empty();
                        if self.text > 2 * 1024 * 1024 {
                            return Err(limit());
                        }
                        if !text.is_empty() {
                            action = Some(Action::Text(text.into()));
                        }
                    }
                }
            }
        } else if !terminal
            && value.get("response.created").is_none()
            && value.get("usage").is_none()
            && value.get("done").is_none()
        {
            return Err(protocol());
        }
        // Preserve a final answer delta before exposing the explicit terminal on the next pull.
        if terminal {
            if action.is_some() {
                self.pending_done = true;
            } else {
                return self.complete().map(Some);
            }
        }
        Ok(action)
    }
    fn complete(&mut self) -> Result<Action, GatewayError> {
        if !self.answered {
            return Err(failure(
                "qwen_web_current_empty_response",
                ErrorKind::ServerError,
            ));
        }
        self.done = true;
        Ok(Action::Done)
    }
    fn bind(&mut self, value: Option<&Value>) -> Result<(), GatewayError> {
        if let Some(value) = value {
            let id = value
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 256)
                .ok_or_else(protocol)?;
            if self.response.as_deref().is_some_and(|v| v != id) {
                return Err(protocol());
            }
            self.response.get_or_insert_with(|| id.into());
        }
        Ok(())
    }
}
fn token_usage(value: &Value) -> Result<Option<TokenUsage>, GatewayError> {
    if !value.is_object() {
        return Err(protocol());
    }
    let read = |key| {
        value
            .get(key)
            .map(|v| v.as_u64().ok_or_else(protocol))
            .transpose()
    };
    let (input, prompt, output, completion) = (
        read("input_tokens")?,
        read("prompt_tokens")?,
        read("output_tokens")?,
        read("completion_tokens")?,
    );
    if matches!((input, prompt), (Some(a), Some(b)) if a != b)
        || matches!((output, completion), (Some(a), Some(b)) if a != b)
    {
        return Err(protocol());
    }
    let input = input.or(prompt);
    let output = output.or(completion);
    let (Some(input), Some(output)) = (input, output) else {
        return Ok(None);
    };
    let total = match read("total_tokens")? {
        Some(total) => total,
        None => input.checked_add(output).ok_or_else(protocol)?,
    };
    Ok(Some(TokenUsage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: total,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    }))
}
pub(super) fn protocol() -> GatewayError {
    failure("qwen_web_current_protocol_invalid", ErrorKind::ServerError)
}
fn limit() -> GatewayError {
    failure("qwen_web_current_response_limit", ErrorKind::ServerError)
}
