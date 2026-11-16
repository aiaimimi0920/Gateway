//! Anthropic text/function subset, preserving block order by native message segments, not flattening.
use super::*;
pub(super) fn convert(raw: &Value) -> Result<Value, BusinessError> {
    fields(
        raw,
        &[
            "model",
            "messages",
            "system",
            "max_tokens",
            "temperature",
            "top_p",
            "stop_sequences",
            "tools",
            "tool_choice",
            "stream",
        ],
    )?;
    if raw["max_tokens"].as_u64().is_none_or(|n| n == 0) {
        return Err(BusinessError::InvalidInput);
    }
    let mut messages = Vec::new();
    if let Some(system) = raw.get("system") {
        system_messages(system, &mut messages)?;
    }
    for message in raw["messages"]
        .as_array()
        .filter(|m| !m.is_empty() && m.len() <= 4096)
        .ok_or(BusinessError::InvalidInput)?
    {
        fields(message, &["role", "content"])?;
        let role = message["role"]
            .as_str()
            .ok_or(BusinessError::InvalidInput)?;
        if messages.len() >= 4096 {
            return Err(BusinessError::InvalidInput);
        }
        if role == "system" {
            system_messages(&message["content"], &mut messages)?;
            continue;
        }
        if !matches!(role, "user" | "assistant") {
            return Err(BusinessError::InvalidInput);
        }
        let content = &message["content"];
        if content.is_string() {
            messages.push(json!({"role":role,"content":content}));
            continue;
        }
        let parts = content
            .as_array()
            .filter(|p| p.len() <= 4096)
            .ok_or(BusinessError::InvalidInput)?;
        if parts.is_empty() {
            messages.push(json!({"role":role,"content":[]}));
        }
        for part in parts {
            if messages.len() >= 4096 {
                return Err(BusinessError::InvalidInput);
            }
            messages.push(match part["type"].as_str() {
                Some("text") => {
                    fields(part, &["type","text"])?;
                    json!({"role":role,"content":[part]})
                }
                Some("tool_use") if role == "assistant" => {
                    fields(part, &["type","id","name","input"])?;
                    if !part["input"].is_object() { return Err(BusinessError::InvalidInput); }
                    json!({"role":"assistant","tool_calls":[{"id":part["id"],"type":"function","function":{"name":part["name"],"arguments":part["input"].to_string()}}]})
                }
                Some("tool_result") if role == "user" => {
                    fields(part, &["type","tool_use_id","content"])?;
                    json!({"role":"tool","tool_call_id":part["tool_use_id"],"content":request::text_parts(&part["content"], "text")?})
                }
                _ => return Err(BusinessError::InvalidInput),
            });
        }
    }
    let mut body = json!({"model":raw["model"],"messages":messages});
    copy_fields(
        raw,
        &mut body,
        &["max_tokens", "temperature", "top_p", "stream"],
    );
    if let Some(stop) = raw.get("stop_sequences") {
        body["stop"] = stop.clone();
    }
    if let Some(tools) = raw.get("tools") {
        let mut mapped = Vec::new();
        for tool in tools
            .as_array()
            .filter(|t| t.len() <= 1024)
            .ok_or(BusinessError::InvalidInput)?
        {
            fields(tool, &["name", "description", "input_schema"])?;
            let mut function = json!({"name":tool["name"],"parameters":tool["input_schema"]});
            copy_fields(tool, &mut function, &["description"]);
            mapped.push(json!({"type":"function","function":function}));
        }
        body["tools"] = json!(mapped);
    }
    if let Some(choice) = raw.get("tool_choice") {
        let kind = choice["type"].as_str().ok_or(BusinessError::InvalidInput)?;
        fields(
            choice,
            if kind == "tool" {
                &["type", "name", "disable_parallel_tool_use"]
            } else {
                &["type", "disable_parallel_tool_use"]
            },
        )?;
        body["tool_choice"] = match kind {
            "auto" => json!("auto"),
            "any" => json!("required"),
            "none" => json!("none"),
            "tool" => json!({"type":"function","function":{"name":choice["name"]}}),
            _ => return Err(BusinessError::InvalidInput),
        };
        if let Some(disable) = choice.get("disable_parallel_tool_use") {
            body["parallel_tool_calls"] =
                json!(!disable.as_bool().ok_or(BusinessError::InvalidInput)?);
        }
    }
    Ok(body)
}
fn system_messages(value: &Value, messages: &mut Vec<Value>) -> Result<(), BusinessError> {
    if value.is_string() {
        messages.push(json!({"role":"system","content":value}));
        return Ok(());
    }
    let parts = request::text_parts(value, "text")?;
    for part in parts.as_array().expect("checked text array") {
        if messages.len() >= 4096 {
            return Err(BusinessError::InvalidInput);
        }
        messages.push(json!({"role":"system","content":part["text"]}));
    }
    Ok(())
}
