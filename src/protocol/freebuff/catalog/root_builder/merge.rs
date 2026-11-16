//! Coalesce native assistant runs without flattening parts or inventing signature/model provenance.
use super::BusinessError;
use serde_json::{Map, Value};

pub(super) fn push(messages: &mut Vec<Value>, message: Value) -> Result<(), BusinessError> {
    if message["role"] != "assistant" || messages.last().is_none_or(|v| v["role"] != "assistant") {
        messages.push(message);
        return Ok(());
    }
    let Value::Object(next) = message else {
        unreachable!("validated message")
    };
    let previous = messages.last_mut().unwrap().as_object_mut().unwrap();
    for (key, value) in next {
        match key.as_str() {
            "role" => {}
            "content" | "reasoning" | "reasoning_content" | "reasoning_details" | "tool_calls" => {
                append(previous, key, value)?
            }
            // The fixed converter's metadata union is shallow, with later-wins precedence.
            _ => {
                previous.insert(key, value);
            }
        }
    }
    Ok(())
}

fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(parts) => parts.is_empty(),
        _ => false,
    }
}
fn append(
    previous: &mut Map<String, Value>,
    key: String,
    value: Value,
) -> Result<(), BusinessError> {
    let Some(current) = previous.get_mut(&key) else {
        previous.insert(key, value);
        return Ok(());
    };
    if empty(&value) {
        return Ok(());
    }
    if empty(current) {
        *current = value;
        return Ok(());
    }
    // The raw snapshot has already passed a byte budget. Retain capacity across each append so
    // a long assistant run remains amortized linear rather than cloning its entire prefix per part.
    match (current, value) {
        (Value::String(text), Value::String(next)) => {
            text.try_reserve(next.len())
                .map_err(|_| BusinessError::RequestTooLarge)?;
            text.push_str(&next);
        }
        (Value::Array(parts), Value::Array(mut next)) => {
            parts
                .try_reserve(next.len())
                .map_err(|_| BusinessError::RequestTooLarge)?;
            parts.append(&mut next);
        }
        // Native mixed string/part content has no lossless fixed-source merge representation.
        // Reject locally instead of flattening media or choosing an invented provider part schema.
        _ => return Err(BusinessError::InvalidInput),
    }
    Ok(())
}
