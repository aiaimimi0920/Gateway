//! Stable tool identity and bounded terminal/delta reconciliation for Chat SSE.
use super::super::to_openai_chat_tools::{
    build_openai_chat_tool_call_arguments_chunk, build_openai_chat_tool_call_start_chunk,
};
use super::ResponsesToOpenAiChatState;
use crate::protocol::{
    canonical::CanonicalToolCall, sse_parse::format_sse_event, stream_error::ProtocolStreamError,
};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct ToolInputProgress {
    values: BTreeMap<String, String>,
    bytes: usize,
}

impl ToolInputProgress {
    fn record(&mut self, id: &str, delta: &str) -> Result<(), ProtocolStreamError> {
        if id.len() > 512
            || (!self.values.contains_key(id) && self.values.len() >= 128)
            || delta.len() > (8 * 1024 * 1024usize).saturating_sub(self.bytes)
        {
            return Err(ProtocolStreamError::invalid_data(
                "Chat tool stream retention limit exceeded",
            ));
        }
        self.values
            .entry(id.to_owned())
            .or_default()
            .push_str(delta);
        self.bytes += delta.len();
        Ok(())
    }

    fn complete(&mut self, id: &str, full: &str) -> Result<String, ProtocolStreamError> {
        let previous = self.values.get(id).map(String::as_str).unwrap_or_default();
        if full.is_empty() {
            return Ok(String::new());
        }
        let tail = full
            .strip_prefix(previous)
            .ok_or_else(|| {
                ProtocolStreamError::invalid_data(
                    "Terminal tool input conflicts with streamed deltas",
                )
            })?
            .to_owned();
        self.record(id, &tail)?;
        Ok(tail)
    }
}

impl ResponsesToOpenAiChatState {
    pub(super) fn handle_tool_added(&mut self, payload: &Value) -> Result<(), ProtocolStreamError> {
        let Some(item) = payload.get("item") else {
            return Ok(());
        };
        let kind = item.get("type").and_then(Value::as_str);
        if !matches!(kind, Some("function_call" | "custom_tool_call")) {
            return Ok(());
        }
        let custom = kind == Some("custom_tool_call");
        let item_id = item.get("id").and_then(Value::as_str).unwrap_or_default();
        if item_id.is_empty() {
            return Err(ProtocolStreamError::invalid_data(
                "Tool item missing identity",
            ));
        }
        self.tool_inputs.record(item_id, "")?;
        let call_id = item
            .get("call_id")
            .and_then(Value::as_str)
            .unwrap_or(item_id);
        let name = item.get("name").and_then(Value::as_str).unwrap_or("tool");
        if call_id.len() > 512 || name.len() > 512 {
            return Err(ProtocolStreamError::invalid_data(
                "Tool identity limit exceeded",
            ));
        }
        let index = self.tool_call_index_for(item_id);
        if self.announced_tool_calls.insert(item_id.to_owned()) {
            let chunk = build_openai_chat_tool_call_start_chunk(
                &self.response_id,
                self.created_at,
                &self.model,
                index,
                call_id,
                name,
                custom,
            );
            self.outputs
                .push_back(format_sse_event(None, &chunk.to_string()).into_bytes());
            if let Some(initial) = item
                .get(if custom { "input" } else { "arguments" })
                .and_then(Value::as_str)
                .filter(|input| !input.is_empty())
            {
                self.tool_inputs.record(item_id, initial)?;
                self.emit_tool_input(index, initial, custom);
            }
        }
        Ok(())
    }

    pub(super) fn handle_tool_delta(
        &mut self,
        payload: &Value,
        custom: bool,
    ) -> Result<(), ProtocolStreamError> {
        let id = payload
            .get("item_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let delta = payload
            .get("delta")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id.is_empty() || delta.is_empty() {
            return Ok(());
        }
        self.tool_inputs.record(id, delta)?;
        let index = self.tool_call_index_for(id);
        self.emit_tool_input(index, delta, custom);
        Ok(())
    }

    pub(super) fn handle_completed_tools(
        &mut self,
        calls: Vec<CanonicalToolCall>,
    ) -> Result<(), ProtocolStreamError> {
        for call in calls {
            if call.name.as_deref().is_some_and(|name| name.len() > 512)
                || call.id.as_deref().is_some_and(|id| id.len() > 512)
            {
                return Err(ProtocolStreamError::invalid_data(
                    "Tool identity limit exceeded",
                ));
            }
            // Item id and call id have different roles. Using call_id as the item
            // key creates a second tool when a terminal snapshot repeats it.
            let item_id = call
                .raw
                .get("id")
                .and_then(Value::as_str)
                .or(call.id.as_deref())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("call_{}", self.next_tool_call_index));
            self.tool_inputs.record(&item_id, "")?;
            let index = self.tool_call_index_for(&item_id);
            let custom = crate::protocol::openai_tool_wire::is_custom(&call);
            if self.announced_tool_calls.insert(item_id.clone()) {
                let chunk = build_openai_chat_tool_call_start_chunk(
                    &self.response_id,
                    self.created_at,
                    &self.model,
                    index,
                    call.id.as_deref().unwrap_or(&item_id),
                    call.name.as_deref().unwrap_or("tool"),
                    custom,
                );
                self.outputs
                    .push_back(format_sse_event(None, &chunk.to_string()).into_bytes());
            }
            if let Some(arguments) = call.arguments.as_deref() {
                let tail = self.tool_inputs.complete(&item_id, arguments)?;
                if !tail.is_empty() {
                    self.emit_tool_input(index, &tail, custom);
                }
            }
        }
        Ok(())
    }

    fn emit_tool_input(&mut self, index: usize, input: &str, custom: bool) {
        let chunk = build_openai_chat_tool_call_arguments_chunk(
            &self.response_id,
            self.created_at,
            &self.model,
            index,
            input,
            custom,
        );
        self.outputs
            .push_back(format_sse_event(None, &chunk.to_string()).into_bytes());
    }

    pub(super) fn tool_call_index_for(&mut self, item_id: &str) -> usize {
        if let Some(index) = self.tool_call_indexes.get(item_id).copied() {
            return index;
        }
        let index = self.next_tool_call_index;
        self.next_tool_call_index += 1;
        self.tool_call_indexes.insert(item_id.to_owned(), index);
        index
    }
}
