//! Exact bounded text parts: no separators, trimming, entity repair or media coercion.
use super::tool_schema::invalid;
use crate::error::GatewayError;
use serde_json::{json, Value};

pub(super) const MAX_RESULT_IPC: usize = 128 * 1024;
pub(super) fn check_ipc(call_id: &str, output: &str) -> Result<(), GatewayError> {
    // JSON escaping can expand an admitted 64 KiB string by up to six times.
    // Reject before a live continuation slot is claimed or a grant is issued.
    let wire = json!({"command":"continue","callId":call_id,"output":output});
    if serde_json::to_vec(&wire).map_err(|_| invalid())?.len() + 1 > MAX_RESULT_IPC {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn parse(value: &Value, part_type: &str) -> Result<String, GatewayError> {
    const MAX_BYTES: usize = 65536;
    if let Some(text) = value.as_str() {
        return if text.len() <= MAX_BYTES {
            Ok(text.into())
        } else {
            Err(invalid())
        };
    }
    let parts = value
        .as_array()
        .filter(|v| v.len() <= 128)
        .ok_or_else(invalid)?;
    let mut output = String::new();
    for part in parts {
        let fields = part.as_object().ok_or_else(invalid)?;
        if fields.len() != 2 || part["type"] != part_type {
            return Err(invalid());
        }
        let text = part["text"].as_str().ok_or_else(invalid)?;
        if text.len() > MAX_BYTES - output.len() {
            return Err(invalid());
        }
        output.push_str(text);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "text_output_tests.rs"]
mod tests;
