//! Validate the native response before projection; keep reasoning/details/extra tool data intact.
use super::{invalid, optional_string, terminal, Accounting, BusinessError};
use serde_json::Value;
pub(super) fn validate_usage(value: &Value) -> Result<(), BusinessError> {
    if value.is_null() {
        return Ok(());
    }
    let object = value.as_object().ok_or_else(invalid)?;
    for field in ["prompt_tokens", "completion_tokens", "total_tokens"] {
        if let Some(value) = object.get(field).filter(|value| !value.is_null()) {
            value.as_u64().ok_or_else(invalid)?;
        }
    }
    for field in ["prompt_tokens_details", "completion_tokens_details"] {
        if let Some(value) = object.get(field).filter(|value| !value.is_null()) {
            let details = value.as_object().ok_or_else(invalid)?;
            for name in [
                "cached_tokens",
                "reasoning_tokens",
                "accepted_prediction_tokens",
                "rejected_prediction_tokens",
            ] {
                if let Some(value) = details.get(name).filter(|value| !value.is_null()) {
                    value.as_u64().ok_or_else(invalid)?;
                }
            }
        }
    }
    Ok(())
}
pub(super) fn validate_message(message: &Value) -> Result<(), BusinessError> {
    let object = message.as_object().ok_or_else(invalid)?;
    if optional_string(object.get("role"))?.is_some_and(|role| role != "assistant") {
        return Err(invalid());
    }
    for key in ["content", "reasoning", "reasoning_content"] {
        optional_string(object.get(key))?;
    }
    if object
        .get("reasoning_details")
        .is_some_and(|v| !v.is_null() && !v.is_array())
    {
        return Err(invalid());
    }
    if let Some(tools) = object.get("tool_calls").filter(|v| !v.is_null()) {
        let tools = tools.as_array().ok_or_else(invalid)?;
        if tools.len() > 1024 {
            return Err(BusinessError::ResponseTooLarge);
        }
        for tool in tools {
            optional_string(tool.get("id"))?;
            if optional_string(tool.get("type"))?.is_some_and(|kind| kind != "function") {
                return Err(invalid());
            }
            let function = tool
                .get("function")
                .and_then(Value::as_object)
                .ok_or_else(invalid)?;
            if optional_string(function.get("name"))?.is_none_or(str::is_empty) {
                return Err(invalid());
            }
            let arguments = optional_string(function.get("arguments"))?.ok_or_else(invalid)?;
            serde_json::from_str::<Value>(arguments).map_err(|_| invalid())?;
        }
    }
    Ok(())
}
pub(crate) fn decode_json(
    body: Value,
    accounting: &mut Accounting,
) -> Result<Value, BusinessError> {
    let object = body.as_object().ok_or_else(invalid)?;
    let usage = object.get("usage").unwrap_or(&Value::Null);
    accounting.observe(usage)?;
    validate_usage(usage)?;
    if object.get("error").is_some_and(|v| !v.is_null()) {
        return Err(invalid());
    }
    for key in ["id", "model"] {
        optional_string(object.get(key))?;
    }
    let choices = object
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    if choices.is_empty() || choices.len() > 128 {
        return Err(invalid());
    }
    let mut indices = std::collections::BTreeSet::new();
    for (position, choice) in choices.iter().enumerate() {
        let index = match choice.get("index") {
            None => position as u64,
            Some(v) => v.as_u64().ok_or_else(invalid)?,
        };
        if !indices.insert(index) {
            return Err(invalid());
        }
        terminal(choice.get("finish_reason").unwrap_or(&Value::Null))?;
        validate_message(choice.get("message").ok_or_else(invalid)?)?;
    }
    Ok(body)
}
