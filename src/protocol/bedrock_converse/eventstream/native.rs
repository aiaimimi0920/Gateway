//! Native carrier bypasses the legacy dense-tool remapper; hold success events until true EOF.
use super::{frame::Output, invalid};
use crate::protocol::{
    bedrock_converse::StreamRelay, canonical::TokenUsage, stream_error::ProtocolStreamError,
};
use serde_json::Value;

#[derive(Default)]
pub(super) struct Native {
    relay: StreamRelay,
    terminal: Vec<(String, Value)>,
}
impl Native {
    pub fn active(&self) -> bool {
        self.relay.active()
    }
    pub fn observe(
        &mut self,
        raw: &Value,
        output: &mut Output,
    ) -> Result<bool, ProtocolStreamError> {
        if let Some((kind, body)) = self.relay.observe(raw)? {
            if matches!(kind.as_str(), "messageStop" | "metadata") {
                if self.terminal.len() >= 2 {
                    return Err(invalid("duplicate native terminal event"));
                }
                self.terminal.push((kind, body));
            } else {
                output.native(&kind, body)?;
            }
        }
        Ok(self.relay.active())
    }
    pub fn done(&mut self) -> Result<(), ProtocolStreamError> {
        self.relay.done()
    }
    pub fn finish(&mut self, output: &mut Output) -> Result<TokenUsage, ProtocolStreamError> {
        self.relay.finish()?;
        let usage = self
            .relay
            .usage()
            .ok_or_else(|| invalid("missing native validated usage"))?;
        let mut validated = super::usage::Usage::default();
        validated.observe(&serde_json::to_value(usage).map_err(ProtocolStreamError::from)?)?;
        let usage = validated.finish()?;
        for (kind, body) in std::mem::take(&mut self.terminal) {
            output.emit(&kind, body)?;
        }
        Ok(usage)
    }
}
