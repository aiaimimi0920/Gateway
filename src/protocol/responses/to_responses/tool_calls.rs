//! Tool-call identity and bounded argument accumulation for Responses SSE.
use super::OpenAiToResponsesState;
use crate::protocol::{sse_parse::format_sse_event, stream_error::ProtocolStreamError};
use serde_json::{json, Value};

const MAX_TOOL_CALLS: usize = 128;
const MAX_TOOL_INPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PENDING_DELTAS: usize = 8192;

#[derive(Debug, Clone)]
pub(super) struct PendingResponseToolCall {
    pub(super) item_id: String,
    pub(super) call_id: String,
    pub(super) output_index: usize,
    pub(super) name: Option<String>,
    pub(super) arguments: String,
    pending_argument_ends: Vec<usize>,
    pub(super) custom: bool,
    pub(super) announced: bool,
    pub(super) done: bool,
}

impl PendingResponseToolCall {
    pub(super) fn item(&self, status: &str) -> Value {
        let mut item = json!({"id":self.item_id,"call_id":self.call_id,
            "type":if self.custom {"custom_tool_call"} else {"function_call"},
            "status":status,"name":self.name.as_deref().unwrap_or("tool")});
        item[if self.custom { "input" } else { "arguments" }] = json!(if status == "in_progress" {
            ""
        } else {
            &self.arguments
        });
        item
    }

    fn event(&self, phase: &str, sequence_number: u64, delta: &str) -> String {
        let kind = if self.custom {
            "custom_tool_call_input"
        } else {
            "function_call_arguments"
        };
        let event_type = format!("response.{kind}.{phase}");
        let field = if phase == "delta" {
            "delta"
        } else if self.custom {
            "input"
        } else {
            "arguments"
        };
        let mut event = json!({"type":event_type,"sequence_number":sequence_number,
            "output_index":self.output_index,"item_id":self.item_id});
        event[field] = json!(delta);
        format_sse_event(Some(&event_type), &event.to_string())
    }

    fn item_event(&self, phase: &str, sequence_number: u64) -> String {
        let event_type = format!("response.output_item.{phase}");
        let payload = json!({"type":event_type,"sequence_number":sequence_number,
            "output_index":self.output_index,"item":self.item(if phase == "added" {"in_progress"} else {"completed"})});
        format_sse_event(Some(&event_type), &payload.to_string())
    }
}

impl OpenAiToResponsesState {
    pub(super) fn handle_tool_call_delta(
        &mut self,
        call: &Value,
        fallback_index: usize,
    ) -> Result<(), ProtocolStreamError> {
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(fallback_index);
        if !self.tool_calls.contains_key(&index) && self.tool_calls.len() >= MAX_TOOL_CALLS {
            return Err(ProtocolStreamError::invalid_data(
                "Responses tool-call count limit exceeded",
            ));
        }
        let custom = call.get("type").and_then(Value::as_str) == Some("custom")
            || call.get("custom").is_some();
        let function = call.get(if custom { "custom" } else { "function" });
        let delta = function
            .and_then(|value| value.get(if custom { "input" } else { "arguments" }))
            .and_then(Value::as_str)
            .unwrap_or_default();
        // Bound the aggregate retained input, not just each individual SSE frame.
        let retained = self
            .tool_calls
            .values()
            .map(|call| call.arguments.len())
            .sum::<usize>();
        if delta.len() > MAX_TOOL_INPUT_BYTES.saturating_sub(retained) {
            return Err(ProtocolStreamError::invalid_data(
                "Responses tool input limit exceeded",
            ));
        }
        if !delta.is_empty()
            && self
                .tool_calls
                .get(&index)
                .is_none_or(|call| !call.announced)
            && self
                .tool_calls
                .values()
                .map(|call| call.pending_argument_ends.len())
                .sum::<usize>()
                >= MAX_PENDING_DELTAS
        {
            return Err(ProtocolStreamError::invalid_data(
                "Responses pending tool delta limit exceeded",
            ));
        }
        let name = function
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty());
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty());
        if name.is_some_and(|name| name.len() > 512) || id.is_some_and(|id| id.len() > 512) {
            return Err(ProtocolStreamError::invalid_data(
                "Responses tool identity limit exceeded",
            ));
        }
        let entry = self.tool_calls.entry(index).or_insert_with(|| {
            let output_index = self.next_output_index;
            self.next_output_index += 1;
            let call_id = format!("call_{}", uuid::Uuid::new_v4().as_simple());
            PendingResponseToolCall {
                item_id: format!("fc_{call_id}"),
                call_id,
                output_index,
                name: None,
                arguments: String::new(),
                pending_argument_ends: Vec::new(),
                custom,
                announced: false,
                done: false,
            }
        });
        if entry.done {
            return Ok(());
        }
        if !entry.announced {
            entry.custom = custom;
            if let Some(id) = id {
                entry.call_id = id.to_owned();
                entry.item_id = format!("fc_{id}");
            }
            if let Some(name) = name {
                entry.name = Some(name.to_owned());
            }
        }
        entry.arguments.push_str(delta);
        if !entry.announced && !delta.is_empty() {
            entry.pending_argument_ends.push(entry.arguments.len());
        }
        if !entry.announced && entry.name.is_some() {
            self.sequence_number += 1;
            self.outputs
                .push_back(entry.item_event("added", self.sequence_number).into_bytes());
            let mut start = 0;
            // Preserve original chunk boundaries without storing a second input
            // copy. UTF-8 boundaries come from complete upstream string deltas.
            for end in std::mem::take(&mut entry.pending_argument_ends) {
                self.sequence_number += 1;
                self.outputs.push_back(
                    entry
                        .event("delta", self.sequence_number, &entry.arguments[start..end])
                        .into_bytes(),
                );
                start = end;
            }
            entry.announced = true;
        } else if entry.announced && !delta.is_empty() {
            self.sequence_number += 1;
            self.outputs.push_back(
                entry
                    .event("delta", self.sequence_number, delta)
                    .into_bytes(),
            );
        }
        Ok(())
    }

    pub(super) fn close_tool_item(&mut self, index: usize) {
        let Some(entry) = self.tool_calls.get_mut(&index).filter(|entry| !entry.done) else {
            return;
        };
        if entry.name.is_none() {
            entry.name = Some(format!("tool_{index}"));
        }
        if !entry.custom {
            entry.arguments = crate::protocol::accio::normalize_tool_args(&entry.arguments);
        }
        if !entry.announced {
            self.sequence_number += 1;
            self.outputs
                .push_back(entry.item_event("added", self.sequence_number).into_bytes());
        }
        self.sequence_number += 1;
        self.outputs.push_back(
            entry
                .event("done", self.sequence_number, &entry.arguments)
                .into_bytes(),
        );
        self.sequence_number += 1;
        self.outputs
            .push_back(entry.item_event("done", self.sequence_number).into_bytes());
        entry.announced = true;
        entry.done = true;
        entry.pending_argument_ends.clear();
    }
}
