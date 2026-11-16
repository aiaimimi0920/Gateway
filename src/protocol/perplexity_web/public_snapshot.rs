//! Full replacement observations are provisional; only the ordinary terminal owns canonical text.
use super::{snapshot, Answer};
use crate::protocol::stream_error::ProtocolStreamError;
use serde_json::{json, Value};
pub(crate) const KEY: &str = "gateway_perplexity_snapshot";
pub(crate) const EVENT: &str = "gateway.perplexity.snapshot";

pub(crate) fn value(sequence: usize, answer: Option<&Answer>) -> Value {
    json!({"version":1,"sequence":sequence,"answer":answer})
}

#[derive(Default)]
pub(crate) struct Relay {
    sequence: u64,
}
impl Relay {
    pub(crate) fn require_terminal(
        &self,
        standard_finish: bool,
    ) -> Result<(), ProtocolStreamError> {
        if self.sequence > 0 && !standard_finish {
            return Err(ProtocolStreamError::invalid_data(
                "Perplexity snapshots have no final answer",
            ));
        }
        Ok(())
    }

    /// Revalidate the public carrier at the generic protocol boundary and omit all raw metadata.
    pub(crate) fn observe(&mut self, chunk: &Value) -> Result<Option<Value>, ProtocolStreamError> {
        let Some(raw) = chunk.get(KEY) else {
            return Ok(None);
        };
        let invalid = || ProtocolStreamError::invalid_data("Invalid Perplexity snapshot carrier");
        let raw = raw
            .as_object()
            .filter(|v| v.len() == 3)
            .ok_or_else(invalid)?;
        let sequence = raw
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or_else(invalid)?;
        if raw.get("version") != Some(&json!(1))
            || sequence != self.sequence + 1
            || sequence > 4096
            || !chunk["choices"].as_array().is_some_and(Vec::is_empty)
            || chunk.get("usage").is_some()
            || chunk.get("gateway_sources").is_some()
        {
            return Err(invalid());
        }
        let answer = raw.get("answer").ok_or_else(invalid)?;
        let answer = if answer.is_null() {
            None
        } else {
            Some(snapshot::public_answer(answer).map_err(|_| invalid())?)
        };
        self.sequence = sequence;
        Ok(Some(value(sequence as usize, answer.as_ref())))
    }
}
