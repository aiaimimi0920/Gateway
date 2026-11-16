//! Legacy buffered replies still need a terminal; partial text and late errors are not success.
use crate::error::GatewayError;
use serde_json::Value;

fn invalid(code: &'static str) -> GatewayError {
    GatewayError::server_error("Qwen Web returned an incomplete or invalid response.")
        .with_code(code)
        .with_provider("qwen_web_compatible")
}

pub(super) fn validate(body: &str) -> Result<(), GatewayError> {
    if body.trim().is_empty() {
        return Ok(()); // Preserve the existing empty-response classification.
    }
    let mut terminal = false;
    if body.trim_start().starts_with('{') {
        inspect(body.trim(), &mut terminal)?;
    } else {
        let mut pending = String::new();
        for line in body.lines().chain(std::iter::once("")) {
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.strip_prefix(' ').unwrap_or(data);
                if pending.is_empty() && data.trim() == "[DONE]" {
                    terminal = true;
                    break;
                }
                if pending.is_empty() && serde_json::from_str::<Value>(data).is_ok() {
                    inspect(data, &mut terminal)?;
                } else {
                    if !pending.is_empty() {
                        pending.push('\n');
                    }
                    pending.push_str(data);
                }
            } else if line.is_empty() && !pending.is_empty() {
                inspect(&pending, &mut terminal)?;
                pending.clear();
            }
        }
    }
    if !terminal {
        return Err(invalid("qwen_web_incomplete_response"));
    }
    Ok(())
}

fn inspect(raw: &str, terminal: &mut bool) -> Result<(), GatewayError> {
    let value: Value =
        serde_json::from_str(raw).map_err(|_| invalid("qwen_web_invalid_response"))?;
    if !value.is_object()
        || value.get("error").is_some_and(|v| !v.is_null())
        || value.get("success") == Some(&Value::Bool(false))
        || value
            .get("response.stopped")
            .is_some_and(|v| !v.is_null() && v != false)
    {
        return Err(invalid("qwen_web_failed_response"));
    }
    if value.get("done") == Some(&Value::Bool(true)) {
        *terminal = true;
    }
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        if choices.len() > 1 {
            return Err(invalid("qwen_web_multiple_choices_unsupported"));
        }
        for choice in choices {
            if choice["delta"]["status"] == "error" {
                return Err(invalid("qwen_web_failed_response"));
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                if !matches!(reason, "stop" | "length") {
                    return Err(invalid("qwen_web_failed_response"));
                }
                *terminal = true;
            }
        }
    }
    Ok(())
}
