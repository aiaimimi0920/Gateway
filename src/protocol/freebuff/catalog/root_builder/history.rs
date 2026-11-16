//! Repair only the consumed bare snapshot. The canonical host history remains host-owned.
use super::{merge, BusinessError};
use serde_json::Value;
use std::{
    collections::HashSet,
    io::{self, Write},
};

pub(in crate::protocol::freebuff::catalog) fn prepare(
    mut messages: Vec<Value>,
    limit: usize,
) -> Result<Vec<Value>, BusinessError> {
    if messages.is_empty() {
        return Err(BusinessError::InvalidInput);
    }
    // Count the RAW snapshot without retaining a second byte buffer, before new strings/vectors.
    // The original completion serializer still bounds, freezes and signs the final envelope once.
    check_size(&messages, limit)?;
    for message in &messages {
        validate(message)?;
    }
    let mut answered = HashSet::new();
    let mut keep = vec![Vec::new(); messages.len()];
    // Reverse scanning answers only calls BEFORE the result and in the same real user segment.
    // No generated instructions/step reminder exists yet; assistant messages are not boundaries.
    for (index, message) in messages.iter().enumerate().rev() {
        match message["role"].as_str().unwrap() {
            "user" => answered.clear(),
            "tool" => {
                answered.insert(message["tool_call_id"].as_str().unwrap());
            }
            "assistant" => {
                if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                    // Native providerExecuted is untrusted JSON, not the SDK's internal capability.
                    keep[index] = calls
                        .iter()
                        .map(|call| answered.contains(call["id"].as_str().unwrap()))
                        .collect();
                }
            }
            _ => unreachable!("validated native role"),
        }
    }
    drop(answered);
    let mut output = Vec::with_capacity(messages.len());
    for (index, mut message) in messages.drain(..).enumerate() {
        let flags = &keep[index];
        if flags.contains(&false) {
            let calls = message["tool_calls"].as_array_mut().unwrap();
            let mut position = 0;
            calls.retain(|_| {
                let retained = flags[position];
                position += 1;
                retained
            });
            if calls.is_empty() {
                message.as_object_mut().unwrap().remove("tool_calls");
                if !has_payload(&message) {
                    continue;
                }
            }
        }
        // Tool messages, including freestanding/duplicate results, remain verbatim boundaries.
        merge::push(&mut output, message)?;
    }
    if output.is_empty() {
        return Err(BusinessError::InvalidInput);
    }
    Ok(output)
}

pub(in crate::protocol::freebuff::catalog) fn check_size(
    messages: &[Value],
    limit: usize,
) -> Result<(), BusinessError> {
    if limit == 0 || limit > 50 * 1024 * 1024 || messages.len() > 4096 {
        return Err(BusinessError::InvalidInput);
    }
    serde_json::to_writer(&mut Counter { used: 0, limit }, messages)
        .map_err(|_| BusinessError::RequestTooLarge)
}

fn validate(message: &Value) -> Result<(), BusinessError> {
    let invalid = || BusinessError::InvalidInput;
    match message.get("role").and_then(Value::as_str) {
        Some("user") => Ok(()),
        Some("tool")
            if message
                .get("tool_call_id")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty()) =>
        {
            Ok(())
        }
        Some("assistant") => {
            for key in [
                "content",
                "reasoning",
                "reasoning_content",
                "reasoning_details",
                "tool_calls",
            ] {
                if let Some(value) = message.get(key).filter(|v| !v.is_null()) {
                    let valid = match key {
                        "content" => value.is_string() || value.is_array(),
                        "reasoning" | "reasoning_content" => value.is_string(),
                        _ => value.is_array(),
                    };
                    if !valid {
                        return Err(invalid());
                    }
                }
            }
            if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
                if calls.iter().any(|call| {
                    !call
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                }) {
                    return Err(invalid());
                }
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}
fn has_payload(message: &Value) -> bool {
    message
        .as_object()
        .unwrap()
        .iter()
        .any(|(key, value)| match key.as_str() {
            "role" | "tool_calls" => false,
            "content" => match value {
                Value::Null => false,
                Value::String(text) => !text.is_empty(),
                Value::Array(parts) => !parts.is_empty(),
                _ => unreachable!("validated content"),
            },
            // An already-native extension might be refusal/audio/opaque provider state. Do not erase it.
            _ => true,
        })
}
struct Counter {
    used: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.used) {
            return Err(io::Error::other("FreeBuff history limit reached"));
        }
        self.used += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
