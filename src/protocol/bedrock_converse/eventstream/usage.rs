//! Merge sparse usage without inventing zero counters or discarding prior cache evidence.
use super::invalid;
use crate::protocol::{canonical::TokenUsage, stream_error::ProtocolStreamError};
use serde_json::Value;

#[derive(Default)]
pub(super) struct Usage {
    input: Option<u64>,
    output: Option<u64>,
    total: Option<u64>,
    read: Option<u64>,
    write: Option<u64>,
}

impl Usage {
    pub fn observe(&mut self, body: &Value) -> Result<(), ProtocolStreamError> {
        if !body.is_object() {
            return Err(invalid("Bedrock usage is not an object"));
        }
        merge(&mut self.input, body.get("prompt_tokens"))?;
        merge(&mut self.output, body.get("completion_tokens"))?;
        merge(&mut self.total, body.get("total_tokens"))?;
        merge(
            &mut self.read,
            body.get("cache_read_input_tokens").filter(|v| !v.is_null()),
        )?;
        merge(
            &mut self.write,
            body.get("cache_creation_input_tokens")
                .filter(|v| !v.is_null()),
        )?;
        if let Some(details) = body.get("prompt_tokens_details").filter(|v| !v.is_null()) {
            if !details.is_object() {
                return Err(invalid("Bedrock cache usage is not an object"));
            }
            if !body
                .get("cache_read_input_tokens")
                .is_some_and(|v| !v.is_null())
            {
                merge(&mut self.read, details.get("cached_tokens"))?;
            }
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<TokenUsage, ProtocolStreamError> {
        let (Some(input), Some(output), Some(total)) = (self.input, self.output, self.total) else {
            return Err(invalid("Bedrock response is missing complete token usage"));
        };
        Ok(TokenUsage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens: total,
            cache_read_input_tokens: self.read,
            cache_creation_input_tokens: self.write,
        })
    }
}

fn merge(target: &mut Option<u64>, value: Option<&Value>) -> Result<(), ProtocolStreamError> {
    if let Some(value) = value {
        let value = value
            .as_u64()
            .filter(|n| *n <= i32::MAX as u64)
            .ok_or_else(|| invalid("Bedrock usage counter does not fit a non-negative integer"))?;
        *target = Some(value);
    }
    Ok(())
}
