use super::normalization::trimmed_owned_ref;
use super::*;

pub(super) fn coerce_message_export(message: &Value) -> Option<GatewayAnalysisExportMessageView> {
    let record = message.as_object()?;
    let role = record
        .get("role")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if !matches!(role.as_str(), "system" | "user" | "assistant" | "tool") {
        return None;
    }
    let tool_calls = record
        .get("toolCalls")
        .and_then(Value::as_array)
        .or_else(|| record.get("tool_calls").and_then(Value::as_array))
        .map(|items| items.len())
        .unwrap_or_default();
    Some(GatewayAnalysisExportMessageView {
        role,
        name: record
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        tool_call_id: record
            .get("toolCallId")
            .and_then(Value::as_str)
            .or_else(|| record.get("tool_call_id").and_then(Value::as_str))
            .map(ToString::to_string),
        text: coerce_message_text(record),
        tool_call_count: tool_calls,
    })
}

fn coerce_message_text(record: &serde_json::Map<String, Value>) -> String {
    record
        .get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .map(coerce_text_from_part)
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn coerce_text_from_part(part: &Value) -> String {
    let Some(record) = part.as_object() else {
        return String::new();
    };
    if let Some(value) = record.get("text").and_then(Value::as_str) {
        return value.to_string();
    }
    if let Some(value) = record.get("value").and_then(Value::as_str) {
        return value.to_string();
    }
    if let Some(value) = record.get("imageUrl").and_then(Value::as_str) {
        return format!("[image] {value}");
    }
    if let Some(value) = record.get("image_url").and_then(Value::as_str) {
        return format!("[image] {value}");
    }
    if let Some(url) = record
        .get("image_url")
        .and_then(Value::as_object)
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
    {
        return format!("[image] {url}");
    }
    if record
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "json" || value == "raw")
    {
        if let Some(value) = record.get("value") {
            return serde_json::to_string(value).unwrap_or_default();
        }
        return serde_json::to_string(part).unwrap_or_default();
    }
    String::new()
}

pub(super) fn coerce_request_tool_names(request_artifact: Option<&Value>) -> Vec<String> {
    let tools = request_artifact
        .and_then(|artifact| artifact.get("canonicalRequest"))
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tool in tools {
        let Some(name) = tool
            .as_object()
            .and_then(|record| record.get("name"))
            .and_then(Value::as_str)
            .and_then(trimmed_owned_ref)
        else {
            continue;
        };
        if seen.insert(name.to_string()) {
            result.push(name.to_string());
        }
    }
    result
}

pub(super) fn coerce_response_tool_names(response_artifact: Option<&Value>) -> Vec<String> {
    let tool_calls = response_artifact
        .and_then(|artifact| artifact.get("result"))
        .and_then(|value| value.get("toolCalls"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tool_call in tool_calls {
        let Some(name) = tool_call
            .as_object()
            .and_then(|record| record.get("name"))
            .and_then(Value::as_str)
            .and_then(trimmed_owned_ref)
        else {
            continue;
        };
        if seen.insert(name.to_string()) {
            result.push(name.to_string());
        }
    }
    result
}
