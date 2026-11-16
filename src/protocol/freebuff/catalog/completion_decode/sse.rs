//! Strict bounded framing and native assembly. A finish marker is not success until actual EOF.
use super::{invalid, optional_string, terminal, Accounting, BusinessError, BODY_LIMIT};
use crate::protocol::stream_decode::{BoundedSseDecoder, DecodeStep};
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[cfg(test)]
#[path = "observed_tests.rs"]
mod observed_tests;

#[derive(Default)]
struct Choice {
    message: serde_json::Map<String, Value>,
    tools: super::tools::Tools,
    finish: Option<String>,
}
pub(crate) struct StreamDecoder {
    wire: BoundedSseDecoder,
    bytes: usize,
    done: bool,
    choices: BTreeMap<u64, Choice>,
    metadata: serde_json::Map<String, Value>,
    usage: Value,
    accounting: Accounting,
}
fn merge_usage(target: &mut Value, source: &Value) {
    if source.is_null() {
        return;
    }
    if let Some(object) = source.as_object() {
        if !target.is_object() {
            *target = json!({});
        }
        for (key, value) in object {
            if !value.is_null() {
                merge_usage(&mut target[key], value);
            }
        }
    } else {
        *target = source.clone();
    }
}
impl StreamDecoder {
    pub(crate) fn new() -> Self {
        Self {
            wire: BoundedSseDecoder::new_strict(BODY_LIMIT),
            bytes: 0,
            done: false,
            choices: BTreeMap::new(),
            metadata: Default::default(),
            usage: Value::Null,
            accounting: Default::default(),
        }
    }
    pub(crate) fn accounting(&self) -> &Accounting {
        &self.accounting
    }
    pub(crate) fn push(&mut self, bytes: Bytes) -> Result<(), BusinessError> {
        self.feed(bytes)?;
        while self.next_frame()?.is_some() {}
        Ok(())
    }
    /// Feed only after next_frame returns None. Keep the remaining chunk in the bounded framer,
    /// not an event queue; the owner drains it one validated frame and one awaited observer at a time.
    pub(crate) fn feed(&mut self, bytes: Bytes) -> Result<(), BusinessError> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= BODY_LIMIT)
            .ok_or(BusinessError::ResponseTooLarge)?;
        self.wire.push_chunk(bytes);
        Ok(())
    }
    /// A frame is provisional even when it contains finish_reason. Only finish at actual EOF
    /// validates the whole completion; accounting may advance even when this frame is rejected.
    pub(crate) fn next_frame(&mut self) -> Result<Option<Value>, BusinessError> {
        loop {
            match self.wire.decode_next() {
                DecodeStep::NeedInput => return Ok(None),
                DecodeStep::Error(_) => return Err(invalid()),
                DecodeStep::Frame(frame) => {
                    if frame.event_name.as_deref() == Some("error") {
                        return Err(invalid());
                    }
                    if frame.data.is_empty() {
                        continue;
                    }
                    if self.done {
                        return Err(invalid());
                    }
                    if frame.data.trim() == "[DONE]" {
                        self.done = true;
                        continue;
                    }
                    let value: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
                    self.observe(&value)?;
                    return Ok(Some(value));
                }
            }
        }
    }
    fn observe(&mut self, body: &Value) -> Result<(), BusinessError> {
        let body = body.as_object().ok_or_else(invalid)?;
        let usage = body.get("usage").unwrap_or(&Value::Null);
        self.accounting.observe(usage)?;
        super::json::validate_usage(usage)?;
        merge_usage(&mut self.usage, usage);
        if body.get("error").is_some_and(|v| !v.is_null()) {
            return Err(invalid());
        }
        for key in ["id", "model"] {
            if let Some(value) = optional_string(body.get(key))? {
                if self.metadata.get(key).is_some_and(|old| old != value) {
                    return Err(invalid());
                }
                self.metadata.insert(key.to_owned(), json!(value));
            }
        }
        if let Some(created) = body.get("created").filter(|v| !v.is_null()) {
            created.as_u64().ok_or_else(invalid)?;
            self.metadata.insert("created".into(), created.clone());
        }
        let choices = body
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let mut seen = std::collections::BTreeSet::new();
        for (position, value) in choices.iter().enumerate() {
            value.as_object().ok_or_else(invalid)?;
            let index = match value.get("index") {
                None => position as u64,
                Some(v) => v.as_u64().ok_or_else(invalid)?,
            };
            if index >= 128 || !seen.insert(index) {
                return Err(invalid());
            }
            let choice = self.choices.entry(index).or_default();
            if choice.finish.is_some() {
                return Err(invalid());
            }
            if let Some(delta) = value.get("delta").filter(|v| !v.is_null()) {
                let delta = delta.as_object().ok_or_else(invalid)?;
                if optional_string(delta.get("role"))?.is_some_and(|role| role != "assistant") {
                    return Err(invalid());
                }
                choice.message.insert("role".into(), json!("assistant"));
                for key in ["content", "reasoning", "reasoning_content"] {
                    if let Some(text) = optional_string(delta.get(key))? {
                        let value = choice.message.entry(key).or_insert_with(|| json!(""));
                        let Value::String(stored) = value else {
                            return Err(invalid());
                        };
                        stored.push_str(text);
                    }
                }
                if let Some(details) = delta.get("reasoning_details").filter(|v| !v.is_null()) {
                    let details = details.as_array().ok_or_else(invalid)?;
                    choice
                        .message
                        .entry("reasoning_details")
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .ok_or_else(invalid)?
                        .extend(details.iter().cloned());
                }
                if let Some(tools) = delta.get("tool_calls").filter(|v| !v.is_null()) {
                    choice.tools.push(tools)?;
                }
                // Unmodeled output modalities cannot be silently thrown away as successful text.
                if delta.keys().any(|key| {
                    !matches!(
                        key.as_str(),
                        "role"
                            | "content"
                            | "reasoning"
                            | "reasoning_content"
                            | "reasoning_details"
                            | "tool_calls"
                    )
                }) {
                    return Err(invalid());
                }
            }
            if let Some(reason) = value.get("finish_reason").filter(|v| !v.is_null()) {
                choice.finish = Some(terminal(reason)?.to_owned());
            }
        }
        Ok(())
    }
    pub(crate) fn finish(mut self) -> Result<Value, BusinessError> {
        self.wire.finish_strict().map_err(|_| invalid())?;
        if self.choices.is_empty() {
            return Err(invalid());
        }
        let mut choices = Vec::with_capacity(self.choices.len());
        for (index, mut choice) in self.choices {
            let reason = choice.finish.ok_or_else(invalid)?;
            if let Some(tools) = choice.tools.into_value() {
                choice.message.insert("tool_calls".into(), tools);
            }
            let message = Value::Object(choice.message);
            super::json::validate_message(&message)?;
            choices.push(json!({"index":index,"message":message,"finish_reason":reason}));
        }
        self.metadata
            .insert("choices".into(), Value::Array(choices));
        if !self.usage.is_null() {
            self.metadata.insert("usage".into(), self.usage);
        }
        Ok(Value::Object(self.metadata))
    }
}
