//! Native Chat deltas stay native. Success is emitted only after EOF/projection/original FINISH.
use super::*;
use serde_json::{json, Value};
pub(super) struct Projection {
    observed: bool,
    remaining: usize,
}
impl Projection {
    pub fn new() -> Self {
        Self {
            observed: false,
            remaining: crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
        }
    }
    pub fn frame(&mut self, mut value: Value) -> Result<Bytes, BusinessError> {
        self.observed = true;
        for choice in value["choices"]
            .as_array_mut()
            .ok_or(BusinessError::InvalidResponse)?
        {
            choice["finish_reason"] = Value::Null;
        }
        value["object"] = json!("chat.completion.chunk");
        self.encode(value, false)
    }
    pub fn finish(&mut self, mut value: Value) -> Result<Bytes, BusinessError> {
        for choice in value["choices"]
            .as_array_mut()
            .ok_or(BusinessError::InvalidResponse)?
        {
            let message = choice
                .as_object_mut()
                .ok_or(BusinessError::InvalidResponse)?
                .remove("message")
                .ok_or(BusinessError::InvalidResponse)?;
            choice["delta"] = if self.observed { json!({}) } else { message };
            // A JSON fallback is one complete delta, not simulated token timing. Tool indices are
            // the native array ordinals; IDs, names, arguments and opaque extensions remain intact.
            if !self.observed {
                if let Some(tools) = choice["delta"]
                    .get_mut("tool_calls")
                    .and_then(Value::as_array_mut)
                {
                    for (index, tool) in tools.iter_mut().enumerate() {
                        tool["index"] = json!(index);
                    }
                }
            }
        }
        value["object"] = json!("chat.completion.chunk");
        self.encode(value, true)
    }
    fn encode(&mut self, value: Value, terminal: bool) -> Result<Bytes, BusinessError> {
        let framing = 8 + if terminal { 14 } else { 0 };
        if self.remaining < framing || !protocol::within_limit(&value, self.remaining - framing) {
            return Err(BusinessError::ResponseTooLarge);
        }
        let mut bytes = b"data: ".to_vec();
        serde_json::to_writer(&mut bytes, &value).map_err(|_| BusinessError::InvalidResponse)?;
        bytes.extend_from_slice(b"\n\n");
        if terminal {
            bytes.extend_from_slice(b"data: [DONE]\n\n");
        }
        self.remaining -= bytes.len();
        Ok(Bytes::from(bytes))
    }
}
#[cfg(test)]
mod tests;
