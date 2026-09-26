//! Shared request-local state for the two Kiro streaming wire encoders.
use std::collections::{HashMap, VecDeque};

use super::KiroEvent;
use super::{build_tool_name_map, estimate_request_prompt_tokens, map_model, now_unix_seconds};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

#[derive(Debug)]
pub(super) struct StreamLimits {
    pub(super) tools: usize,
    pub(super) metadata_bytes: usize,
    pub(super) blocks: usize,
}

impl Default for StreamLimits {
    fn default() -> Self {
        Self {
            tools: 4096,
            metadata_bytes: 64 * 1024 * 1024,
            blocks: 4096,
        }
    }
}

#[derive(Debug)]
pub(super) struct StreamToolCall {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) announced: bool,
    pub(super) index: usize,
}

#[derive(Debug)]
pub(super) struct TranslatorState {
    pub(super) response_id: String,
    pub(super) anthropic_message_id: String,
    pub(super) created: i64,
    pub(super) model: String,
    pub(super) prompt_tokens: u64,
    pub(super) completion_tokens: u64,
    pub(super) context_overflow: bool,
    pub(super) message_started: bool,
    pub(super) open_text_index: Option<usize>,
    pub(super) next_block_index: usize,
    pub(super) tool_name_map: HashMap<String, String>,
    pub(super) pending_tools: HashMap<String, StreamToolCall>,
    pub(super) tool_calls_seen: bool,
    pub(super) outputs: VecDeque<Vec<u8>>,
    pub(super) limits: StreamLimits,
    pub(super) retained_tool_bytes: usize,
}

impl TranslatorState {
    pub(super) fn admit(&mut self, event: &KiroEvent, anthropic: bool) -> Result<(), GatewayError> {
        match event {
            KiroEvent::ToolUse {
                name, tool_use_id, ..
            } if !self.pending_tools.contains_key(tool_use_id) => {
                if self.pending_tools.len() >= self.limits.tools {
                    return Err(state_limit_error("tool count"));
                }
                if anthropic {
                    self.check_new_block()?;
                }
                let restored_name = self
                    .tool_name_map
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                // The map key and entry ID are retained; argument deltas are not.
                let bytes = tool_use_id
                    .len()
                    .checked_mul(2)
                    .and_then(|bytes| bytes.checked_add(restored_name.len()))
                    .and_then(|bytes| bytes.checked_add(self.retained_tool_bytes))
                    .filter(|bytes| *bytes <= self.limits.metadata_bytes)
                    .ok_or_else(|| state_limit_error("tool metadata bytes"))?;
                self.pending_tools.try_reserve(1).map_err(|_| {
                    GatewayError::server_error("Unable to allocate Kiro streaming tool state")
                        .with_code("kiro_stream_state_allocation_failed")
                })?;
                self.retained_tool_bytes = bytes;
            }
            KiroEvent::AssistantResponse { content }
                if anthropic && !content.is_empty() && self.open_text_index.is_none() =>
            {
                self.check_new_block()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn check_new_block(&self) -> Result<(), GatewayError> {
        if self.next_block_index >= self.limits.blocks {
            return Err(state_limit_error("content block count"));
        }
        Ok(())
    }
}

fn state_limit_error(scope: &str) -> GatewayError {
    GatewayError::server_error(format!("Kiro streaming state exceeds {scope} limit"))
        .with_code("kiro_stream_state_too_large")
}

pub(super) fn build_translator_state(model: String, req: CanonicalRelayRequest) -> TranslatorState {
    TranslatorState {
        response_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        anthropic_message_id: format!("msg_{}", uuid::Uuid::new_v4().as_simple()),
        created: now_unix_seconds(),
        model: map_model(&model),
        prompt_tokens: estimate_request_prompt_tokens(&req, &model),
        completion_tokens: 0,
        context_overflow: false,
        message_started: false,
        open_text_index: None,
        next_block_index: 0,
        tool_name_map: build_tool_name_map(&req),
        pending_tools: HashMap::new(),
        tool_calls_seen: false,
        outputs: VecDeque::new(),
        limits: StreamLimits::default(),
        retained_tool_bytes: 0,
    }
}
