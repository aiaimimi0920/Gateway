//! Responses text/function subset. No implicit server continuation, built-in tools or store promise.
use super::*;
pub(super) fn convert(raw: &Value) -> Result<Value, BusinessError> {
    fields(
        raw,
        &[
            "model",
            "input",
            "instructions",
            "max_output_tokens",
            "temperature",
            "top_p",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "reasoning",
            "user",
            "stream",
        ],
    )?;
    let mut messages = Vec::new();
    if let Some(instructions) = raw.get("instructions") {
        if !instructions.is_string() {
            return Err(BusinessError::InvalidInput);
        }
        messages.push(json!({"role":"system","content":instructions}));
    }
    if raw["input"].is_string() {
        messages.push(json!({"role":"user","content":raw["input"]}));
    } else {
        for item in raw["input"]
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= 4096)
            .ok_or(BusinessError::InvalidInput)?
        {
            if item.get("type").is_some_and(|v| !v.is_string()) {
                return Err(BusinessError::InvalidInput);
            }
            messages.push(match item.get("type").and_then(Value::as_str) {
                Some("function_call") => {
                    fields(item, &["type","call_id","name","arguments","id","status"])?;
                    if item.get("id").is_some_and(|id| id != &item["call_id"])
                        || item.get("status").is_some_and(|s| s != "completed") { return Err(BusinessError::InvalidInput); }
                    json!({"role":"assistant","tool_calls":[{"id":item["call_id"],"type":"function","function":{"name":item["name"],"arguments":item["arguments"]}}]})
                }
                Some("function_call_output") => {
                    fields(item, &["type","call_id","output"])?;
                    if !item["output"].is_string() { return Err(BusinessError::InvalidInput); }
                    json!({"role":"tool","tool_call_id":item["call_id"],"content":item["output"]})
                }
                None | Some("message") => {
                    fields(item, &["type","role","content"])?;
                    let role = item["role"].as_str().ok_or(BusinessError::InvalidInput)?;
                    if !matches!(role, "system" | "developer" | "user" | "assistant") { return Err(BusinessError::InvalidInput); }
                    let content = if matches!(role, "system" | "developer") {
                        if !item["content"].is_string() { return Err(BusinessError::InvalidInput); }
                        item["content"].clone()
                    } else { request::text_parts(&item["content"], if role == "assistant" { "output_text" } else { "input_text" })? };
                    json!({"role":role,"content":content})
                }
                _ => return Err(BusinessError::InvalidInput),
            });
        }
    }
    let mut body = json!({"model":raw["model"],"messages":messages});
    copy_fields(
        raw,
        &mut body,
        &[
            "temperature",
            "top_p",
            "parallel_tool_calls",
            "user",
            "stream",
        ],
    );
    if let Some(max) = raw.get("max_output_tokens") {
        body["max_completion_tokens"] = max.clone();
    }
    if let Some(reasoning) = raw.get("reasoning") {
        fields(reasoning, &["effort"])?;
        if reasoning.get("effort").is_none() {
            return Err(BusinessError::InvalidInput);
        }
        body["reasoning_effort"] = reasoning["effort"].clone();
    }
    if let Some(tools) = raw.get("tools") {
        let mut mapped = Vec::new();
        for tool in tools
            .as_array()
            .filter(|t| t.len() <= 1024)
            .ok_or(BusinessError::InvalidInput)?
        {
            fields(
                tool,
                &["type", "name", "description", "parameters", "strict"],
            )?;
            if tool["type"] != "function" {
                return Err(BusinessError::InvalidInput);
            }
            let mut function = tool.clone();
            function
                .as_object_mut()
                .expect("checked tool")
                .remove("type");
            mapped.push(json!({"type":"function","function":function}));
        }
        body["tools"] = json!(mapped);
    }
    if let Some(choice) = raw.get("tool_choice") {
        body["tool_choice"] = if choice.is_string() {
            choice.clone()
        } else {
            fields(choice, &["type", "name"])?;
            if choice["type"] != "function" {
                return Err(BusinessError::InvalidInput);
            }
            json!({"type":"function","function":{"name":choice["name"]}})
        };
    }
    Ok(body)
}
