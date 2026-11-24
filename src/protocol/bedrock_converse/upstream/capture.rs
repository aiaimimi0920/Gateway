//! Shared bounded native observation; normalized content and terminal witness have one owner.
use super::{
    headers::Event,
    invalid,
    state::{State, Update},
    turn::Snapshot,
};
use crate::protocol::{
    bedrock_turn::{self, BedrockAssistantTurn},
    canonical::TokenUsage,
    stream_error::ProtocolStreamError,
};
use serde_json::{json, Value};

#[derive(Default)]
pub(super) struct Capture {
    state: State,
    snapshot: Snapshot,
    pub count: u64,
}
pub(super) struct Completion {
    pub reason: &'static str,
    pub usage: TokenUsage,
    pub turn: Option<BedrockAssistantTurn>,
    pub count: u64,
}
impl Capture {
    pub fn public(tools: super::super::public_tools::PublicTools) -> Self {
        Self {
            state: State::public(tools),
            ..Self::default()
        }
    }
    pub fn observe(&mut self, event: &Event) -> Result<Value, ProtocolStreamError> {
        let update = self.state.observe(event)?;
        if let Some(update) = update.as_ref().filter(|_| !self.state.is_public()) {
            self.snapshot.accept(event, update)?;
        }
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| invalid("event count exceeded"))?;
        Ok(match update {
            Some(Update::Start) => json!({"role":"assistant"}),
            Some(Update::Text(text)) => json!({"content":text}),
            Some(Update::ToolStart { index, id, name }) => json!({"tool_calls":[{
                "index":index,"id":id,"type":"function","function":{"name":name,"arguments":""}
            }]}),
            Some(Update::ToolDelta { index, input }) => {
                json!({"tool_calls":[{"index":index,"function":{"arguments":input}}]})
            }
            Some(Update::Reasoning(_)) | None => json!({}),
        })
    }
    pub fn finish(self) -> Result<Completion, ProtocolStreamError> {
        let (reason, usage) = self.state.finish()?;
        Ok(Completion {
            reason,
            usage,
            turn: self.snapshot.finish()?,
            count: self.count,
        })
    }
}
impl Completion {
    pub fn usage_json(&self) -> Value {
        json!({"prompt_tokens":self.usage.prompt_tokens,"completion_tokens":self.usage.completion_tokens,
            "total_tokens":self.usage.total_tokens,"cache_read_input_tokens":self.usage.cache_read_input_tokens,
            "cache_creation_input_tokens":self.usage.cache_creation_input_tokens})
    }
    pub fn project(&self, body: &mut Value) {
        body[super::relay::TERMINAL_FIELD] = json!({"events":self.count});
        bedrock_turn::project(body, self.turn.as_ref());
    }
}
