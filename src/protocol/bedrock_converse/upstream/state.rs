//! Validate response/block order before exposing deltas; completion includes post-stop usage.
use std::collections::BTreeMap;

use super::super::public_tools::PublicTools;
use super::{headers::Event, invalid, upstream_failure};
use crate::protocol::{canonical::TokenUsage, stream_error::ProtocolStreamError};
use serde_json::Value;

const MAX_BLOCKS: usize = 4096;
const MAX_TOOL_BYTES: usize = 4 * 1024 * 1024;

pub(super) enum Update {
    Start,
    Text(String),
    Reasoning(super::reasoning::Delta),
    ToolStart {
        index: u32,
        id: String,
        name: String,
    },
    ToolDelta {
        index: u32,
        input: String,
    },
}

enum Block {
    Text,
    ReasoningText { has_text: bool },
    Redacted,
    Tool { index: u32, arguments: String },
    Closed,
}

#[derive(Default)]
pub(super) struct State {
    started: bool,
    public_tools: Option<PublicTools>,
    blocks: BTreeMap<u32, Block>,
    tool_bytes: usize,
    tool_count: u32,
    stop: Option<&'static str>,
    usage: Option<TokenUsage>,
}

impl State {
    pub fn public(tools: PublicTools) -> Self {
        Self {
            public_tools: Some(tools),
            ..Self::default()
        }
    }

    pub fn is_public(&self) -> bool {
        self.public_tools.is_some()
    }
    pub fn observe(&mut self, event: &Event) -> Result<Option<Update>, ProtocolStreamError> {
        let body = &event.body;
        if event.kind == "messageStart" {
            if self.started || body.get("role").and_then(Value::as_str) != Some("assistant") {
                return Err(invalid("invalid message start"));
            }
            self.started = true;
            return Ok(Some(Update::Start));
        }
        if !self.started || self.usage.is_some() {
            return Err(invalid("event outside active response"));
        }
        if event.kind == "metadata" {
            if self.stop.is_none() {
                return Err(invalid("metadata before message stop"));
            }
            self.usage = Some(usage(
                body.get("usage").ok_or_else(|| invalid("missing usage"))?,
            )?);
            return Ok(None);
        }
        if self.stop.is_some() {
            return Err(invalid("content after message stop"));
        }
        match event.kind.as_str() {
            "contentBlockStart" => self.start(body).map(Some),
            "contentBlockDelta" => self.delta(body).map(Some),
            "contentBlockStop" => self.close(index(body)?),
            "messageStop" => {
                if self
                    .blocks
                    .values()
                    .any(|block| !matches!(block, Block::Closed))
                {
                    return Err(invalid("message stopped with open content blocks"));
                }
                let reason = stop_reason(body.get("stopReason").and_then(Value::as_str))?;
                if reason == "tool_calls" && self.tool_count == 0 {
                    return Err(invalid("tool stop reason has no tool identity"));
                }
                if self
                    .public_tools
                    .as_ref()
                    .is_some_and(|tools| !tools.complete(reason))
                {
                    return Err(invalid("tool identity contradicts finish reason"));
                }
                self.stop = Some(reason);
                Ok(None)
            }
            _ => Err(invalid("unsupported event type")),
        }
    }

    fn admit(&self) -> Result<(), ProtocolStreamError> {
        if self.blocks.len() >= MAX_BLOCKS {
            return Err(invalid("content block limit exceeded"));
        }
        Ok(())
    }

    fn start(&mut self, body: &Value) -> Result<Update, ProtocolStreamError> {
        self.admit()?;
        let index = index(body)?;
        if self.blocks.contains_key(&index) {
            return Err(invalid("duplicate content block"));
        }
        let start = body
            .get("start")
            .and_then(Value::as_object)
            .filter(|map| map.len() == 1)
            .and_then(|map| map.get("toolUse"))
            .ok_or_else(|| invalid("unsupported content start"))?;
        let id = identity(start, "toolUseId")?;
        let name = identity(start, "name")?;
        if let Some(tools) = &mut self.public_tools {
            if start.as_object().is_none_or(|map| map.len() != 2) || !tools.accept(&id, &name) {
                return Err(invalid("unsupported or duplicate client tool identity"));
            }
        }
        // AWS indices include text blocks. OpenAI tool indices are dense and tool-only.
        let tool_index = self.tool_count;
        self.tool_count += 1;
        self.blocks.insert(
            index,
            Block::Tool {
                index: tool_index,
                arguments: String::new(),
            },
        );
        Ok(Update::ToolStart {
            index: tool_index,
            id,
            name,
        })
    }

    fn delta(&mut self, body: &Value) -> Result<Update, ProtocolStreamError> {
        let index = index(body)?;
        let delta = body
            .get("delta")
            .and_then(Value::as_object)
            .filter(|map| map.len() == 1)
            .ok_or_else(|| invalid("invalid content delta"))?;
        if let Some(text) = delta.get("text").and_then(Value::as_str) {
            if !self.blocks.contains_key(&index) {
                self.admit()?;
                self.blocks.insert(index, Block::Text);
            }
            if !matches!(self.blocks.get(&index), Some(Block::Text)) {
                return Err(invalid("text delta conflicts with block identity"));
            }
            return Ok(Update::Text(text.into()));
        }
        if let Some(reasoning) = delta.get("reasoningContent") {
            if self.is_public() {
                return Err(invalid("reasoning block outside public projection"));
            }
            let delta = super::reasoning::parse(reasoning)?;
            if !self.blocks.contains_key(&index) {
                self.admit()?;
                self.blocks.insert(
                    index,
                    match &delta {
                        super::reasoning::Delta::Redacted(_) => Block::Redacted,
                        _ => Block::ReasoningText { has_text: false },
                    },
                );
            }
            match (self.blocks.get_mut(&index), &delta) {
                (Some(Block::ReasoningText { has_text }), super::reasoning::Delta::Text(_)) => {
                    *has_text = true
                }
                (Some(Block::ReasoningText { .. }), super::reasoning::Delta::Signature(_))
                | (Some(Block::Redacted), super::reasoning::Delta::Redacted(_)) => {}
                _ => return Err(invalid("reasoning delta conflicts with block identity")),
            }
            return Ok(Update::Reasoning(delta));
        }
        if let Some(input) = delta
            .get("toolUse")
            .filter(|v| !self.is_public() || v.as_object().is_some_and(|map| map.len() == 1))
            .and_then(|v| v.get("input"))
            .and_then(Value::as_str)
        {
            let Some(Block::Tool {
                index: tool_index,
                arguments,
            }) = self.blocks.get_mut(&index)
            else {
                return Err(invalid("tool delta has no active tool start"));
            };
            self.tool_bytes = self
                .tool_bytes
                .checked_add(input.len())
                .filter(|n| *n <= MAX_TOOL_BYTES)
                .ok_or_else(|| invalid("tool arguments exceed retained-byte limit"))?;
            arguments
                .try_reserve(input.len())
                .map_err(|_| invalid("tool argument allocation failed"))?;
            arguments.push_str(input);
            return Ok(Update::ToolDelta {
                index: *tool_index,
                input: input.into(),
            });
        }
        Err(invalid("unsupported content delta"))
    }

    fn close(&mut self, index: u32) -> Result<Option<Update>, ProtocolStreamError> {
        let block = self
            .blocks
            .get_mut(&index)
            .ok_or_else(|| invalid("stop has no content block"))?;
        let mut update = None;
        if let Block::Tool { index, arguments } = block {
            if self.public_tools.is_some()
                && super::super::public_tools::arguments(arguments).is_none()
            {
                return Err(invalid("tool arguments are not an unambiguous JSON object"));
            }
            if self.public_tools.is_none()
                && !arguments.trim().is_empty()
                && serde_json::from_str::<Value>(arguments).is_err()
            {
                return Err(invalid("tool arguments are not complete JSON"));
            }
            self.tool_bytes -= arguments.len();
            // Match buffered no-argument normalization without re-emitting prior fragments.
            if arguments.trim().is_empty() {
                update = Some(Update::ToolDelta {
                    index: *index,
                    input: "{}".into(),
                });
            }
        } else if matches!(block, Block::ReasoningText { has_text: false }) {
            return Err(invalid("reasoning text block has no text member"));
        } else if matches!(block, Block::Closed) {
            return Err(invalid("duplicate content block stop"));
        }
        *block = Block::Closed;
        Ok(update)
    }

    pub fn finish(self) -> Result<(&'static str, TokenUsage), ProtocolStreamError> {
        let reason = self
            .stop
            .ok_or_else(|| invalid("EOF before message stop"))?;
        let usage = self
            .usage
            .ok_or_else(|| invalid("EOF before usage metadata"))?;
        Ok((reason, usage))
    }
}

pub(super) fn index(body: &Value) -> Result<u32, ProtocolStreamError> {
    body.get("contentBlockIndex")
        .and_then(Value::as_u64)
        .and_then(|n| i32::try_from(n).ok())
        .map(|n| n as u32)
        .ok_or_else(|| invalid("invalid content block index"))
}

fn identity(body: &Value, key: &str) -> Result<String, ProtocolStreamError> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= 1024)
        .map(str::to_owned)
        .ok_or_else(|| invalid("missing or oversized tool identity"))
}

pub(super) fn usage(body: &Value) -> Result<TokenUsage, ProtocolStreamError> {
    let required = |key| {
        body.get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("invalid usage counter"))
    };
    let optional = |key| {
        body.get(key)
            .map(|v| v.as_u64().ok_or_else(|| invalid("invalid cache counter")))
            .transpose()
    };
    Ok(TokenUsage {
        prompt_tokens: required("inputTokens")?,
        completion_tokens: required("outputTokens")?,
        total_tokens: required("totalTokens")?,
        cache_creation_input_tokens: optional("cacheWriteInputTokens")?,
        cache_read_input_tokens: optional("cacheReadInputTokens")?,
    })
}

pub(super) fn stop_reason(reason: Option<&str>) -> Result<&'static str, ProtocolStreamError> {
    match reason {
        Some("end_turn" | "stop_sequence") => Ok("stop"),
        Some("tool_use") => Ok("tool_calls"),
        Some("max_tokens" | "model_context_window_exceeded") => Ok("length"),
        Some("guardrail_intervened" | "content_filtered") => Ok("content_filter"),
        Some(reason @ ("malformed_model_output" | "malformed_tool_use")) => {
            Err(upstream_failure(reason))
        }
        _ => Err(invalid("missing or unsupported stop reason")),
    }
}
