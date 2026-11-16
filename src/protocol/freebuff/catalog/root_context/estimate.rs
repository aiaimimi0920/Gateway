//! Cheap native-context estimates, never exact tokenizer counts. Provider usage calibrates deltas.
use super::super::{completion_decode::MAX_SAFE_INTEGER, BusinessError};
use serde_json::Value;

fn add(left: u64, right: u64) -> Result<u64, BusinessError> {
    left.checked_add(right)
        .filter(|n| *n <= MAX_SAFE_INTEGER)
        .ok_or(BusinessError::RequestTooLarge)
}
fn weighted(text: &str) -> Result<u64, BusinessError> {
    // Match the fixed estimator's JS UTF-16 length and native UTF-8 expansion, without a buffer.
    let units = text.encode_utf16().count() as u64;
    let extra = (text.len() as u64 - units)
        .checked_mul(3)
        .ok_or(BusinessError::RequestTooLarge)?;
    add(units, extra)
}
pub(super) fn text(value: &str) -> Result<u64, BusinessError> {
    Ok(weighted(value)?.div_ceil(3))
}
struct Budget {
    remaining: usize,
}
impl Budget {
    fn visit(&mut self, depth: usize) -> Result<(), BusinessError> {
        if depth > 100 || self.remaining == 0 {
            return Err(BusinessError::RequestTooLarge);
        }
        self.remaining -= 1;
        Ok(())
    }
    fn json(&mut self, value: &Value, depth: usize) -> Result<u64, BusinessError> {
        self.visit(depth)?;
        match value {
            Value::Null => Ok(4),
            Value::Bool(value) => Ok(if *value { 4 } else { 5 }),
            Value::Number(value) => Ok(value.to_string().len() as u64),
            Value::String(value) => add(weighted(value)?, 2),
            Value::Array(values) => values
                .iter()
                .try_fold(2, |sum, v| add(add(sum, 1)?, self.json(v, depth + 1)?)),
            Value::Object(values) => values.iter().try_fold(2, |sum, (key, v)| {
                add(add(sum, add(weighted(key)?, 4)?)?, self.json(v, depth + 1)?)
            }),
        }
    }
    fn json_tokens(&mut self, value: &Value) -> Result<u64, BusinessError> {
        Ok(self.json(value, 0)?.div_ceil(3))
    }
    fn content(&mut self, value: &Value) -> Result<u64, BusinessError> {
        match value {
            Value::Null => Ok(0),
            Value::String(value) => text(value),
            Value::Array(parts) => parts.iter().try_fold(0, |sum, part| {
                self.visit(1)?;
                let tokens = match part.get("type").and_then(Value::as_str) {
                    Some(
                        "image_url" | "file" | "file_url" | "image" | "input_audio" | "video_url",
                    ) => 1600,
                    Some("text" | "input_text" | "output_text" | "reasoning")
                        if part.get("text").is_some_and(Value::is_string) =>
                    {
                        text(part["text"].as_str().unwrap())?
                    }
                    _ => self.json_tokens(part)?,
                };
                add(sum, tokens)
            }),
            _ => self.json_tokens(value),
        }
    }
    fn tool(&mut self, call: &Value) -> Result<u64, BusinessError> {
        self.visit(1)?;
        let name = call.pointer("/function/name").and_then(Value::as_str);
        let arguments = call.pointer("/function/arguments").and_then(Value::as_str);
        let (Some(name), Some(arguments)) = (name, arguments) else {
            return self.json_tokens(call);
        };
        // Already-native arguments are text. Do not allocate another parsed multi-MB tool payload.
        let mut total = add(text(name)?, text(arguments)?)?;
        for (key, value) in call.as_object().unwrap() {
            if !matches!(key.as_str(), "id" | "type" | "function") {
                total = add(total, self.json_tokens(value)?)?;
            }
        }
        for (key, value) in call["function"].as_object().unwrap() {
            if !matches!(key.as_str(), "name" | "arguments") {
                total = add(total, self.json_tokens(value)?)?;
            }
        }
        Ok(total)
    }
    fn message(&mut self, message: &Value) -> Result<u64, BusinessError> {
        self.visit(0)?;
        let mut total = 8;
        let fields = message.as_object().ok_or(BusinessError::InvalidInput)?;
        for (key, value) in fields {
            let tokens = match key.as_str() {
                "role" | "tool_call_id" | "name" => 0,
                "content" => self.content(value)?,
                "reasoning" | "reasoning_content" if value.is_string() => {
                    text(value.as_str().unwrap())?
                }
                "tool_calls" if value.is_array() => value
                    .as_array()
                    .unwrap()
                    .iter()
                    .try_fold(0, |sum, call| add(sum, self.tool(call)?))?,
                _ if value.is_null() => 0,
                _ => self.json_tokens(value)?,
            };
            total = add(total, tokens)?;
        }
        Ok(total)
    }
}
pub(super) fn request(body: &Value) -> Result<u64, BusinessError> {
    let mut budget = Budget { remaining: 100_000 };
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(BusinessError::InvalidInput)?;
    let mut total = 0;
    for message in messages {
        let count = if message["role"] == "system" {
            text(
                message["content"]
                    .as_str()
                    .ok_or(BusinessError::InvalidInput)?,
            )?
        } else {
            budget.message(message)?
        };
        total = add(total, count)?;
    }
    add(
        total,
        match body.get("tools") {
            Some(tools) => budget.json_tokens(tools)?,
            None => 0,
        },
    )
}
pub(in crate::protocol::freebuff::catalog) fn message(
    message: &Value,
) -> Result<u64, BusinessError> {
    Budget { remaining: 100_000 }.message(message)
}
#[cfg(test)]
pub(super) fn json(value: &Value) -> Result<u64, BusinessError> {
    Budget { remaining: 100_000 }.json_tokens(value)
}
