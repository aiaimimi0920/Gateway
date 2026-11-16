//! Closed native Chat shapes; opaque reasoning/call extensions stay data, never executable authority.
use super::*;
fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.is_empty())
}
fn function_name(value: &Value) -> bool {
    value.as_str().is_some_and(|name| {
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    })
}
pub(super) fn message(value: &Value) -> Result<(), BusinessError> {
    let role = value["role"].as_str().ok_or(BusinessError::InvalidInput)?;
    let allowed = match role {
        "system" | "developer" | "user" => &["role", "content", "name"][..],
        "assistant" => &[
            "role",
            "content",
            "name",
            "tool_calls",
            "reasoning",
            "reasoning_content",
            "reasoning_details",
            "refusal",
        ][..],
        "tool" => &["role", "content", "name", "tool_call_id"][..],
        _ => return Err(BusinessError::InvalidInput),
    };
    fields(value, allowed)?;
    if value.get("name").is_some_and(|name| !function_name(name))
        || role == "tool" && !nonempty(&value["tool_call_id"])
    {
        return Err(BusinessError::InvalidInput);
    }
    match value.get("content") {
        Some(Value::String(_)) => {}
        None | Some(Value::Null) if role == "assistant" => {}
        Some(Value::Array(parts)) if !matches!(role, "system" | "developer") => {
            for part in parts {
                fields(part, &["type", "text"])?;
                if part["type"] != "text" || !part["text"].is_string() {
                    return Err(BusinessError::InvalidInput);
                }
            }
        }
        _ => return Err(BusinessError::InvalidInput),
    }
    for key in ["reasoning", "reasoning_content", "refusal"] {
        if value
            .get(key)
            .is_some_and(|v| !v.is_string() && !v.is_null())
        {
            return Err(BusinessError::InvalidInput);
        }
    }
    if value.get("reasoning_details").is_some_and(|v| {
        v.as_array()
            .is_none_or(|items| items.iter().any(|item| !item.is_object()))
    }) {
        return Err(BusinessError::InvalidInput);
    }
    if let Some(calls) = value.get("tool_calls") {
        let calls = calls
            .as_array()
            .filter(|calls| calls.len() <= 1024)
            .ok_or(BusinessError::InvalidInput)?;
        for call in calls {
            fields(call, &["id", "type", "function", "extra_content"])?;
            fields(&call["function"], &["name", "arguments"])?;
            if call["type"] != "function"
                || !nonempty(&call["id"])
                || !function_name(&call["function"]["name"])
                || call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .is_none_or(|text| serde_json::from_str::<Value>(text).is_err())
                || call.get("extra_content").is_some_and(|v| !v.is_object())
            {
                return Err(BusinessError::InvalidInput);
            }
        }
    }
    Ok(())
}
pub(super) fn tool(value: &Value) -> Result<&str, BusinessError> {
    fields(value, &["type", "function"])?;
    let function = &value["function"];
    fields(function, &["name", "description", "parameters", "strict"])?;
    if value["type"] != "function"
        || !function_name(&function["name"])
        || !function["parameters"].is_object()
        || function.get("description").is_some_and(|v| !v.is_string())
        || function.get("strict").is_some_and(|v| !v.is_boolean())
    {
        return Err(BusinessError::InvalidInput);
    }
    Ok(function["name"].as_str().unwrap())
}
