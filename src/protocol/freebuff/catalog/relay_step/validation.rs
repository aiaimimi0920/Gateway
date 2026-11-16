//! Native text/function relay subset. Reject unsupported input before quoting rather than dropping it.
use super::*;
mod native;
mod parameters;
pub(super) fn validate(body: &Value) -> Result<(), BusinessError> {
    let invalid = || BusinessError::InvalidInput;
    let object = body.as_object().ok_or_else(invalid)?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "model"
                | "messages"
                | "stream"
                | "n"
                | "tools"
                | "tool_choice"
                | "parallel_tool_calls"
                | "temperature"
                | "top_p"
                | "max_tokens"
                | "max_completion_tokens"
                | "stop"
                | "seed"
                | "frequency_penalty"
                | "presence_penalty"
                | "reasoning_effort"
                | "user"
        )
    }) || body.get("stream").is_some_and(|v| !v.is_boolean())
        || body.get("n").is_some_and(|v| v.as_u64() != Some(1))
        || body
            .get("model")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(invalid());
    }
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .filter(|m| !m.is_empty() && m.len() <= 4096)
        .ok_or_else(invalid)?;
    for message in messages {
        native::message(message)?;
    }
    if let Some(tools) = body.get("tools") {
        let tools = tools
            .as_array()
            .filter(|t| t.len() <= 1024)
            .ok_or_else(invalid)?;
        let mut names = std::collections::HashSet::new();
        for tool in tools {
            let name = native::tool(tool)?;
            if !names.insert(name) {
                return Err(invalid());
            }
        }
    }
    parameters::validate(body)
}

fn fields(value: &Value, allowed: &[&str]) -> Result<(), BusinessError> {
    if value
        .as_object()
        .is_none_or(|object| object.keys().any(|key| !allowed.contains(&key.as_str())))
    {
        return Err(BusinessError::InvalidInput);
    }
    Ok(())
}
