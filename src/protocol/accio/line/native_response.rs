//! Fold the same admitted events used by streaming; do not reconstruct or discard reported usage.
use super::{
    normalize_tool_args, stream_translate::merge_usage, to_tool_call, ParsedEvent, PendingToolCall,
};
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall};
use std::collections::HashMap;

pub(super) struct Collector {
    response: CanonicalRelayResponse,
    pending: HashMap<i64, PendingToolCall>,
}
impl Collector {
    pub(super) fn new(model: &str) -> Self {
        Self {
            response: CanonicalRelayResponse {
                sources: Vec::new(),
                bedrock_turn: None,
                vertex_turn: None,
                model_declaration: None,
                model: model.into(),
                text: String::new(),
                usage: None,
                tool_calls: Vec::new(),
                upstream_status: Some(200),
                finish_reason: None,
            },
            pending: HashMap::new(),
        }
    }
    pub(super) fn accept(&mut self, events: Vec<ParsedEvent>) {
        for event in events {
            match event {
                ParsedEvent::Start { model, usage } => {
                    if let Some(model) = model {
                        self.response.model = model;
                    }
                    self.response.usage = merge_usage(self.response.usage.take(), usage);
                }
                ParsedEvent::Text(text) => self.response.text.push_str(&text),
                ParsedEvent::ToolStart { index, id, name } => {
                    self.pending.insert(
                        index,
                        PendingToolCall {
                            id,
                            name,
                            arguments: String::new(),
                            announced: false,
                        },
                    );
                }
                ParsedEvent::ToolDelta { index, partial } => {
                    if let Some(call) = self.pending.get_mut(&index) {
                        call.arguments.push_str(&partial);
                    }
                }
                ParsedEvent::ToolEnd { index } => {
                    if let Some(call) = self.pending.remove(&index) {
                        self.response.tool_calls.push(to_tool_call(call));
                    }
                }
                ParsedEvent::ToolCall {
                    id,
                    name,
                    arguments,
                } => self.response.tool_calls.push(CanonicalToolCall {
                    id: Some(id),
                    name: Some(name),
                    call_type: "function".into(),
                    arguments: Some(normalize_tool_args(&arguments)),
                    raw: HashMap::new(),
                }),
                ParsedEvent::Finish { reason, usage } => {
                    self.response.finish_reason = reason;
                    self.response.usage = merge_usage(self.response.usage.take(), usage);
                }
                ParsedEvent::Done => {}
                ParsedEvent::ProviderError { .. } => {
                    unreachable!("Accio admission rejects business errors before collection")
                }
            }
        }
    }
    pub(super) fn finish(mut self) -> CanonicalRelayResponse {
        let mut pending: Vec<_> = self.pending.into_iter().collect();
        pending.sort_unstable_by_key(|(index, _)| *index);
        for (_, call) in pending {
            self.response.tool_calls.push(to_tool_call(call));
        }
        self.response
    }
}
