use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

pub(super) fn prompt_from_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    if let Some(prompt) = read_optional_string(&req.raw_body, &["prompt", "input", "lyrics"]) {
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }

    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(missing_message).with_code(missing_code));
    }

    Ok(prompt)
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
