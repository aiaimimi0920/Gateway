//! Chat JSON 结构准入；产品负责传输、诊断与是否允许重放。
use crate::protocol::{canonical::CanonicalRelayResponse, openai};
use serde_json::Value;
pub(crate) fn unpack(body: &Value) -> Result<CanonicalRelayResponse, ()> {
    validate(body)?;
    openai::unpack_openai_response(body).map_err(|_| invalid())
}
pub(crate) fn validate(body: &Value) -> Result<(), ()> {
    if !body.is_object() || body.get("error").is_some_and(|v| !v.is_null()) {
        return Err(invalid());
    }
    let choices = body
        .get("choices")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(invalid)?;
    for choice in choices {
        let message = choice
            .get("message")
            .filter(|v| v.is_object())
            .ok_or_else(invalid)?;
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            return Err(invalid());
        }
        let finish = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        if !matches!(
            finish,
            "stop" | "length" | "content_filter" | "tool_calls" | "function_call"
        ) {
            return Err(invalid());
        }
        let tools = message
            .get("tool_calls")
            .filter(|v| !v.is_null())
            .map(|value| {
                value
                    .as_array()
                    .filter(|calls| calls.iter().all(valid_tool))
                    .ok_or_else(invalid)
            })
            .transpose()?;
        let legacy = message.get("function_call").filter(|v| !v.is_null());
        if legacy.is_some_and(|v| !valid_function(v)) {
            return Err(invalid());
        }
        if (finish == "tool_calls" && tools.is_none_or(Vec::is_empty))
            || (finish == "function_call" && legacy.is_none())
        {
            return Err(invalid());
        }
        let has_tools = tools.is_some_and(|v| !v.is_empty()) || legacy.is_some();
        match message.get("content") {
            Some(Value::String(_)) => {}
            Some(Value::Array(parts))
                if parts.iter().all(|part| {
                    part.get("type").and_then(Value::as_str) == Some("text")
                        && part.get("text").is_some_and(Value::is_string)
                }) => {}
            // Moderation/truncation can legitimately produce no text. Unlike a probe, tools are valid output.
            None | Some(Value::Null)
                if has_tools || matches!(finish, "length" | "content_filter") => {}
            _ => return Err(invalid()),
        }
    }
    validate_usage(body)?;
    Ok(())
}

fn valid_function(value: &Value) -> bool {
    value
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|v| !v.trim().is_empty())
        && value.get("arguments").is_some_and(Value::is_string)
}
fn valid_tool(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("function")
        && value
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty())
        && value.get("function").is_some_and(valid_function)
}
pub(crate) fn validate_usage(body: &Value) -> Result<(), ()> {
    let Some(usage) = body.get("usage").filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let prompt = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    let completion = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    let total = prompt.checked_add(completion).ok_or_else(invalid)?;
    if usage
        .get("total_tokens")
        .is_some_and(|v| v.as_u64() != Some(total))
    {
        return Err(invalid());
    }
    // Match the canonical parser's primary/alias fallback, including an empty primary details object.
    for name in ["prompt_tokens_details", "input_tokens_details"] {
        if let Some(details) = usage.get(name).filter(|v| !v.is_null()) {
            if !details.is_object() {
                return Err(invalid());
            }
            if let Some(cached) = details.get("cached_tokens") {
                if cached.as_u64().is_none_or(|n| n > prompt) {
                    return Err(invalid());
                }
                break;
            }
        }
    }
    Ok(())
}
fn invalid() {}
