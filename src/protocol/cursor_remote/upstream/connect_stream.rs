//! Connect-RPC binary frame decoder, `EndStream` validator, and `TextDecoderState`.
use super::{
    json_wire::unique_json,
    protobuf::{fields, oneof, Field},
    text_session::{default_text_context, TextSession},
    ProbeError, WorkerOutput,
};
use serde_json::Value;
use std::collections::HashSet;

const MAX_CONNECT_FRAME: usize = 256 * 1024;
const MAX_CONNECT_FRAMES: usize = 4096;
const MAX_THINKING_PHASE: usize = 16 * 1024;
const MAX_THINKING_RUN: usize = 64 * 1024;
const MAX_TEXT_PHASE: usize = 64 * 1024;

const CONNECT_ERROR_CODES: [&str; 16] = [
    "canceled",
    "unknown",
    "invalid_argument",
    "deadline_exceeded",
    "not_found",
    "already_exists",
    "permission_denied",
    "resource_exhausted",
    "failed_precondition",
    "aborted",
    "out_of_range",
    "unimplemented",
    "internal",
    "unavailable",
    "data_loss",
    "unauthenticated",
];

pub(crate) struct PhaseSnapshot {
    pub(crate) text: String,
    pub(crate) thinking: Option<String>,
}

pub(crate) struct TextDecoderState {
    buffer: Vec<u8>,
    pub(crate) text: String,
    pub(crate) thinking: String,
    thinking_bytes: usize,
    thinking_total_bytes: usize,
    pub(crate) turn_ended: bool,
    pub(crate) end_stream: bool,
    remote_connect_code: Option<String>,
    frames: usize,
    pub(crate) session: Option<TextSession>,
    pub(crate) replies: Vec<Vec<u8>>,
    usage: super::usage::Usage,
}

impl TextDecoderState {
    pub(crate) fn new_text() -> Result<Self, ProbeError> {
        Ok(Self::with_session(Some(TextSession::new(
            default_text_context()?,
        ))))
    }

    pub(crate) fn with_session(session: Option<TextSession>) -> Self {
        Self {
            buffer: Vec::new(),
            text: String::new(),
            thinking: String::new(),
            thinking_bytes: 0,
            thinking_total_bytes: 0,
            turn_ended: false,
            end_stream: false,
            remote_connect_code: None,
            frames: 0,
            session,
            replies: Vec::new(),
            usage: Default::default(),
        }
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<(), ProbeError> {
        self.push_with(chunk, |state, bytes| state.decode_server_message(bytes))
    }

    pub(crate) fn push_with<F>(&mut self, chunk: &[u8], mut on_message: F) -> Result<(), ProbeError>
    where
        F: FnMut(&mut Self, &[u8]) -> Result<(), ProbeError>,
    {
        self.buffer.extend_from_slice(chunk);
        let mut cursor = 0usize;
        let result = (|| {
            while self.buffer.len() - cursor >= 5 {
                let flags = self.buffer[cursor];
                let size = u32::from_be_bytes(
                    self.buffer[cursor + 1..cursor + 5]
                        .try_into()
                        .expect("4-byte frame length"),
                ) as usize;
                if !matches!(flags, 0 | 2) {
                    return Err(ProbeError::new("unsupported_connect_flags"));
                }
                if size > MAX_CONNECT_FRAME {
                    return Err(ProbeError::new("connect_frame_too_large"));
                }
                if self.end_stream {
                    return Err(ProbeError::new("data_after_end_stream"));
                }
                if self.buffer.len() - cursor < 5 + size {
                    break;
                }
                let frame_bytes = self.buffer[cursor + 5..cursor + 5 + size].to_vec();
                cursor += 5 + size;
                self.frames += 1;
                if self.frames > MAX_CONNECT_FRAMES {
                    return Err(ProbeError::new("connect_frame_limit"));
                }
                if flags == 2 {
                    let ending = unique_json(&frame_bytes)
                        .map_err(|_| ProbeError::new("invalid_end_stream"))?;
                    self.remote_connect_code = validate_end_stream(&ending)?;
                    self.end_stream = true;
                } else {
                    on_message(self, &frame_bytes)?;
                }
            }
            Ok(())
        })();
        if cursor > 0 {
            self.buffer.drain(..cursor);
        }
        result
    }

    pub(crate) fn decode_server_message(&mut self, bytes: &[u8]) -> Result<(), ProbeError> {
        let Some(envelope) = oneof(bytes, &[(8, 2)])? else {
            return Ok(());
        };
        if matches!(envelope.tag, 2 | 4) {
            let Some(session) = self.session.as_mut() else {
                return Err(ProbeError::new("remote_client_action_required"));
            };
            if self.turn_ended {
                return Err(ProbeError::new("remote_client_action_required"));
            }
            let reply = session.reply(envelope)?;
            self.replies.push(reply);
            return Ok(());
        }
        if matches!(envelope.tag, 5 | 7) {
            return Err(ProbeError::new("remote_client_action_required"));
        }
        if matches!(envelope.tag, 3 | 8) {
            return Ok(());
        }
        if envelope.tag != 1 {
            return Err(ProbeError::new("unknown_server_event"));
        }
        let event = oneof(envelope.bytes, &[(25, 0)])?
            .ok_or_else(|| ProbeError::new("unexpected_protobuf_message"))?;
        self.decode_interaction_update(event)
    }

    pub(crate) fn decode_interaction_update(&mut self, event: Field<'_>) -> Result<(), ProbeError> {
        if matches!(event.tag, 2 | 3 | 7 | 12 | 15 | 19 | 21 | 22 | 27 | 30) {
            return Err(ProbeError::new("remote_tool_or_interaction_required"));
        }
        match event.tag {
            14 => {
                if self.turn_ended {
                    return Err(ProbeError::new("duplicate_turn_end"));
                }
                self.usage.turn_end(event.bytes)?;
                self.turn_ended = true;
            }
            4 => {
                if self.turn_ended {
                    return Err(ProbeError::new("thinking_after_turn_end"));
                }
                let parts = fields(event.bytes)?;
                let mut seen = HashSet::with_capacity(parts.len());
                for item in &parts {
                    if !matches!(item.tag, 1 | 2) || !seen.insert(item.tag) {
                        return Err(ProbeError::new("unexpected_thinking_delta"));
                    }
                }
                let text_field = parts.iter().find(|i| i.tag == 1);
                let style_field = parts.iter().find(|i| i.tag == 2);
                if text_field.is_some_and(|f| f.wire != 2)
                    || style_field.is_some_and(|f| f.wire != 0)
                {
                    return Err(ProbeError::new("unexpected_thinking_delta"));
                }
                let raw_delta = text_field.map_or(&[][..], |f| f.bytes);
                let delta = std::str::from_utf8(raw_delta)
                    .map_err(|_| ProbeError::new("invalid_thinking_utf8"))?;
                let size = delta.len();
                if self.thinking_bytes + size > MAX_THINKING_PHASE
                    || self.thinking_total_bytes + size > MAX_THINKING_RUN
                {
                    return Err(ProbeError::new("thinking_too_large"));
                }
                self.thinking_bytes += size;
                self.thinking_total_bytes += size;
                self.thinking.push_str(delta);
            }
            1 => {
                if self.turn_ended {
                    return Err(ProbeError::new("text_after_turn_end"));
                }
                let parts = fields(event.bytes)?;
                let mut seen = HashSet::with_capacity(parts.len());
                for item in &parts {
                    if !matches!(item.tag, 1 | 2) || !seen.insert(item.tag) {
                        return Err(ProbeError::new("unexpected_text_delta"));
                    }
                }
                let text_field = parts.iter().find(|i| i.tag == 1);
                let notice_field = parts.iter().find(|i| i.tag == 2);
                if text_field.is_some_and(|f| f.wire != 2)
                    || notice_field.is_some_and(|f| f.wire != 0 || f.varint > 1)
                {
                    return Err(ProbeError::new("unexpected_text_delta"));
                }
                if notice_field.is_some_and(|f| f.varint == 1) {
                    return Ok(());
                }
                let raw_delta = text_field.map_or(&[][..], |f| f.bytes);
                let delta = std::str::from_utf8(raw_delta)
                    .map_err(|_| ProbeError::new("invalid_text_utf8"))?;
                if self.text.len() + delta.len() > MAX_TEXT_PHASE {
                    return Err(ProbeError::new("text_too_large"));
                }
                self.text.push_str(delta);
            }
            31 => self.usage.model_call(event.bytes)?,
            5 | 6 | 8 | 9 | 10 | 11 | 13 | 16 | 17 | 18 | 20 | 23 | 24 | 26 | 28 => {}
            _ => return Err(ProbeError::new("unknown_interaction_event")),
        }
        Ok(())
    }

    pub(crate) fn take_phase(&mut self) -> PhaseSnapshot {
        self.usage.split_phase();
        let text = std::mem::take(&mut self.text);
        let raw_thinking = std::mem::take(&mut self.thinking);
        self.thinking_bytes = 0;
        PhaseSnapshot {
            text,
            thinking: (!raw_thinking.is_empty()).then_some(raw_thinking),
        }
    }

    pub(crate) fn take_replies(&mut self) -> Result<Vec<Vec<u8>>, ProbeError> {
        if self.end_stream && !self.replies.is_empty() {
            return Err(ProbeError::new("remote_client_action_required"));
        }
        Ok(std::mem::take(&mut self.replies))
    }

    pub(crate) fn finish(&mut self, requested_model: String) -> Result<WorkerOutput, ProbeError> {
        if !self.buffer.is_empty() {
            return Err(ProbeError::new("connect_truncated"));
        }
        if self.remote_connect_code.is_some() {
            return Err(ProbeError::new("connect_remote_error"));
        }
        if self.text.trim().is_empty() || !self.turn_ended || !self.end_stream {
            return Err(ProbeError::new("successful_terminal_not_confirmed"));
        }
        let thinking = (!self.thinking.is_empty()).then(|| std::mem::take(&mut self.thinking));
        Ok(WorkerOutput {
            text: std::mem::take(&mut self.text),
            thinking,
            requested_model,
            usage: self.usage.finish()?,
        })
    }

    pub(crate) fn close(&mut self) {
        if let Some(session) = self.session.as_mut() {
            session.close();
        }
        self.replies.clear();
    }
}

impl Drop for TextDecoderState {
    fn drop(&mut self) {
        self.close();
    }
}

fn validate_end_stream(ending: &Value) -> Result<Option<String>, ProbeError> {
    let invalid = || ProbeError::new("invalid_end_stream");
    let obj = ending.as_object().ok_or_else(invalid)?;
    if obj
        .keys()
        .any(|k| !matches!(k.as_str(), "metadata" | "error"))
    {
        return Err(invalid());
    }
    if let Some(meta) = obj.get("metadata") {
        let meta_obj = meta.as_object().ok_or_else(invalid)?;
        if meta_obj.values().any(|vals| {
            !vals
                .as_array()
                .is_some_and(|arr| arr.iter().all(Value::is_string))
        }) {
            return Err(invalid());
        }
    }
    let Some(err_val) = obj.get("error") else {
        return Ok(None);
    };
    let err_obj = err_val.as_object().ok_or_else(invalid)?;
    let code = err_obj
        .get("code")
        .and_then(Value::as_str)
        .filter(|c| CONNECT_ERROR_CODES.contains(c))
        .ok_or_else(invalid)?;
    if err_obj.get("message").is_some_and(|m| !m.is_string()) {
        return Err(invalid());
    }
    if let Some(details) = err_obj.get("details") {
        let arr = details.as_array().ok_or_else(invalid)?;
        let valid_details = arr.iter().all(|detail| {
            detail.as_object().is_some_and(|d| {
                d.get("type").is_some_and(Value::is_string)
                    && d.get("value").is_some_and(Value::is_string)
            })
        });
        if !valid_details {
            return Err(invalid());
        }
    }
    Ok(Some(code.to_owned()))
}
