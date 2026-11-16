//! Chat admission and terminal separation; no converter-produced completion is trusted.
use super::chat_stream::invalid;
use crate::protocol::{sse_parse::SseFrame, stream_error::ProtocolStreamError};
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Choice {
    finished: bool,
    tools: BTreeMap<u64, ToolFields>,
    legacy: Option<ToolFields>,
}
#[derive(Default)]
struct ToolFields {
    id: bool,
    name: bool,
    arguments: bool,
}
impl ToolFields {
    fn observe(&mut self, function: &Value) {
        self.name |= function["name"]
            .as_str()
            .is_some_and(|v| !v.trim().is_empty());
        self.arguments |= function.get("arguments").is_some_and(Value::is_string);
    }
    fn complete(&self) -> bool {
        self.name && self.arguments
    }
}
#[derive(Default)]
pub(super) struct Admission {
    choices: BTreeMap<u64, Choice>,
    done: bool,
    usage_seen: bool,
    either_terminal: bool,
}
impl Admission {
    pub(super) fn new(either_terminal: bool) -> Self {
        Self {
            either_terminal,
            ..Self::default()
        }
    }
    pub(super) fn eof_terminal(&self) -> Option<Bytes> {
        (self.either_terminal && !self.done).then(|| Bytes::from_static(b"data: [DONE]\n\n"))
    }

    pub(super) fn complete(&self) -> bool {
        (self.done || self.either_terminal)
            && !self.choices.is_empty()
            && self.choices.values().all(|c| c.finished)
    }
    pub(super) fn accept(
        &mut self,
        frame: SseFrame,
    ) -> Result<(Option<Bytes>, Option<Bytes>), ProtocolStreamError> {
        if self.done
            || frame
                .event_name
                .as_deref()
                .is_some_and(|name| name != "message")
        {
            return Err(invalid());
        }
        if frame.data == "[DONE]" {
            let mut terminal = Vec::new();
            if self.either_terminal {
                for (index, choice) in &mut self.choices {
                    if choice.finished {
                        continue;
                    }
                    if choice.tools.values().any(|t| !t.id || !t.complete())
                        || choice.legacy.as_ref().is_some_and(|t| !t.complete())
                    {
                        return Err(invalid());
                    }
                    choice.finished = true;
                    // DONE is authoritative completion; this reason is a public-protocol projection, not an observed upstream reason.
                    let finish = if !choice.tools.is_empty() {
                        "tool_calls"
                    } else if choice.legacy.is_some() {
                        "function_call"
                    } else {
                        "stop"
                    };
                    terminal.push(json!({"index":index,"delta":{},"finish_reason":finish}));
                }
            }
            self.done = true;
            if !self.complete() {
                return Err(invalid());
            }
            let mut bytes = if terminal.is_empty() {
                Vec::new()
            } else {
                encode(&json!({"choices":terminal})).to_vec()
            };
            bytes.extend_from_slice(b"data: [DONE]\n\n");
            return Ok((None, Some(Bytes::from(bytes))));
        }
        let mut value: Value = serde_json::from_str(&frame.data).map_err(|_| invalid())?;
        if !value.is_object()
            || value.get("error").is_some_and(|v| !v.is_null())
            // Usage taps also recognize Responses envelopes; ordinary Chat must not carry one.
            || value.get("response").is_some()
            || value.get("type").is_some()
            || value
                .get("object")
                .is_some_and(|v| v.as_str() != Some("chat.completion.chunk"))
        {
            return Err(invalid());
        }
        super::chat_admission::validate_usage(&value).map_err(|_| invalid())?;
        validate_usage_aliases(&value)?;
        let has_usage = value.get("usage").is_some_and(|v| !v.is_null());
        if let Some(usage) = value.get_mut("usage").and_then(Value::as_object_mut) {
            // All downstream observers must choose Chat counters, not the optional Responses alias branch.
            usage.remove("input_tokens");
            usage.remove("output_tokens");
        }
        let mut terminal = value.clone();
        let choices = value
            .get_mut("choices")
            .and_then(Value::as_array_mut)
            .ok_or_else(invalid)?;
        if self.usage_seen || (choices.is_empty() && !has_usage) {
            return Err(invalid());
        }
        let mut seen = BTreeSet::new();
        let mut endings = Vec::new();
        for choice in choices.iter_mut() {
            let index = choice
                .get("index")
                .and_then(Value::as_u64)
                .ok_or_else(invalid)?;
            if !seen.insert(index)
                || (!self.choices.contains_key(&index) && self.choices.len() >= 128)
            {
                return Err(invalid());
            }
            let state = self.choices.entry(index).or_default();
            if state.finished {
                return Err(invalid());
            }
            let delta = choice
                .get("delta")
                .and_then(Value::as_object)
                .ok_or_else(invalid)?;
            if delta
                .get("role")
                .is_some_and(|v| v.as_str() != Some("assistant"))
                || delta
                    .get("content")
                    .is_some_and(|v| !v.is_null() && !v.is_string())
            {
                return Err(invalid());
            }
            if let Some(calls) = delta.get("tool_calls").filter(|v| !v.is_null()) {
                let calls = calls
                    .as_array()
                    .filter(|v| v.len() <= 128)
                    .ok_or_else(invalid)?;
                let mut tool_indices = BTreeSet::new();
                for call in calls {
                    let index = call
                        .get("index")
                        .and_then(Value::as_u64)
                        .ok_or_else(invalid)?;
                    if !tool_indices.insert(index)
                        || (!state.tools.contains_key(&index) && state.tools.len() >= 128)
                        || call.get("id").is_some_and(|v| !v.is_string())
                        || call
                            .get("type")
                            .is_some_and(|v| v.as_str() != Some("function"))
                    {
                        return Err(invalid());
                    }
                    // Retain presence bits only; argument content remains in the bounded downstream parser.
                    let fields = state.tools.entry(index).or_default();
                    fields.id |= call["id"].as_str().is_some_and(|v| !v.is_empty());
                    if let Some(function) = call.get("function") {
                        validate_function_delta(function)?;
                        fields.observe(function);
                    }
                }
            }
            if let Some(function) = delta.get("function_call").filter(|v| !v.is_null()) {
                validate_function_delta(function)?;
                state
                    .legacy
                    .get_or_insert_with(ToolFields::default)
                    .observe(function);
            }
            if let Some(finish) = choice.get("finish_reason").filter(|v| !v.is_null()) {
                let finish = finish.as_str().ok_or_else(invalid)?;
                if !matches!(
                    finish,
                    "stop" | "length" | "content_filter" | "tool_calls" | "function_call"
                ) || (finish == "tool_calls" && state.tools.is_empty())
                    || (finish == "function_call" && state.legacy.is_none())
                    || state.tools.values().any(|t| !t.id || !t.complete())
                    || state.legacy.as_ref().is_some_and(|t| !t.complete())
                {
                    return Err(invalid());
                }
                state.finished = true;
                let mut ending = choice.clone();
                ending["delta"] = json!({});
                endings.push(ending);
                choice["finish_reason"] = Value::Null;
            }
        }
        if has_usage {
            if self.choices.is_empty()
                || (!self.either_terminal && self.choices.values().any(|c| !c.finished))
            {
                return Err(invalid());
            }
            self.usage_seen = true;
        }
        // Every real delta remains live, even when a different choice finishes in the same frame.
        let live_choices: Vec<_> = choices
            .iter()
            .filter(|c| c["delta"].as_object().is_some_and(|d| !d.is_empty()))
            .cloned()
            .collect();
        value["choices"] = json!(live_choices);
        value.as_object_mut().unwrap().remove("usage");
        terminal["choices"] = json!(endings);
        if !has_usage {
            terminal.as_object_mut().unwrap().remove("usage");
        }
        let live = (!live_choices.is_empty()).then(|| encode(&value));
        let held = (has_usage
            || terminal["choices"]
                .as_array()
                .is_some_and(|v| !v.is_empty()))
        .then(|| encode(&terminal));
        Ok((live, held))
    }
}
fn validate_function_delta(value: &Value) -> Result<(), ProtocolStreamError> {
    if !value.is_object()
        || ["name", "arguments"]
            .iter()
            .any(|key| value.get(*key).is_some_and(|v| !v.is_string()))
    {
        return Err(invalid());
    }
    Ok(())
}
fn validate_usage_aliases(value: &Value) -> Result<(), ProtocolStreamError> {
    let Some(usage) = value.get("usage").filter(|v| !v.is_null()) else {
        return Ok(());
    };
    if usage.get("input_tokens").is_some() != usage.get("output_tokens").is_some() {
        return Err(invalid());
    }
    for (alias, primary) in [
        ("input_tokens", "prompt_tokens"),
        ("output_tokens", "completion_tokens"),
    ] {
        if usage.get(alias).is_some_and(|v| v != &usage[primary]) {
            return Err(invalid());
        }
    }
    let cache = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .or_else(|| usage.pointer("/input_tokens_details/cached_tokens"));
    // Existing usage taps prefer this nonstandard alias. It must not contradict the validated wire count.
    if usage
        .get("cache_read_input_tokens")
        .is_some_and(|v| Some(v) != cache)
        || usage
            .get("cache_creation_input_tokens")
            .is_some_and(|v| v.as_u64() != Some(0))
    {
        return Err(invalid());
    }
    Ok(())
}
fn encode(value: &Value) -> Bytes {
    // JSON reserialization makes multi-line data safe without copying raw SSE field syntax.
    Bytes::from(format!("data: {value}\n\n"))
}

#[cfg(test)]
#[path = "../qwen/workspace_stream_state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "chat_stream_tool_tests.rs"]
mod tool_tests;
