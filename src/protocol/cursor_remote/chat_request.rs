//! Strict Chat wire projection, deliberately independent of lossy generic history.
use super::{text_output, tool_schema::invalid};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind},
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub(super) fn project(req: &CanonicalRelayRequest) -> Result<CanonicalRelayRequest, GatewayError> {
    let body = req
        .raw_body
        .as_object()
        .ok_or_else(super::request::invalid)?;
    let mut projected = Map::new();
    for (key, value) in body {
        match key.as_str() {
            "model" | "stream" | "store" | "tool_choice" | "parallel_tool_calls" => {
                projected.insert(key.clone(), value.clone());
            }
            "messages" | "tools" => {}
            "response_format" if value == &json!({"type":"text"}) => {}
            "n" if value.as_u64() == Some(1) => {}
            "stream_options"
                if req.stream
                    && (value == &json!({}) || value == &json!({"include_usage":false})) => {}
            "user" if value.as_str().is_some_and(|s| s.len() <= 256) => {}
            "reasoning_effort" if value == "none" => {}
            "function_call" | "functions" => return Err(invalid()),
            // In particular max_tokens and max_completion_tokens have no native
            // mapping; rejecting them is safer than claiming an enforced bound.
            _ => return Err(super::request::invalid()),
        }
    }
    if let Some(tools) = body.get("tools") {
        let tools = tools
            .as_array()
            .filter(|v| v.len() <= 64)
            .ok_or_else(invalid)?;
        projected.insert(
            "tools".into(),
            Value::Array(tools.iter().map(definition).collect::<Result<_, _>>()?),
        );
    }
    let messages = body["messages"]
        .as_array()
        .filter(|v| v.len() <= 256)
        .ok_or_else(invalid)?;
    let mut history = Vec::new();
    let mut pending = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for message in messages {
        let fields = message.as_object().ok_or_else(invalid)?;
        let role = message["role"].as_str().ok_or_else(invalid)?;
        if role == "tool" {
            keys(fields, &["role", "content", "tool_call_id"])?;
            let id = message["tool_call_id"].as_str().ok_or_else(invalid)?;
            let custom = pending.remove(id).ok_or_else(invalid)?;
            history.push(json!({
                "type":if custom {"custom_tool_call_output"} else {"function_call_output"},
                "call_id":id, "output":text_output::parse(&message["content"], "text")?
            }));
            continue;
        }
        if !pending.is_empty() || !["system", "developer", "user", "assistant"].contains(&role) {
            return Err(invalid());
        }
        keys(
            fields,
            &["role", "content", "tool_calls", "reasoning_content"],
        )?;
        let calls = match message.get("tool_calls") {
            None => &[][..],
            Some(value) if role == "assistant" => value
                .as_array()
                .filter(|v| v.len() == 1)
                .ok_or_else(invalid)?
                .as_slice(),
            _ => return Err(invalid()),
        };
        let content = if !calls.is_empty() && message.get("content").is_none_or(Value::is_null) {
            json!("")
        } else {
            content(&message["content"], role)?
        };
        if let Some(thinking) = message.get("reasoning_content") {
            if role != "assistant" || calls.is_empty() {
                return Err(invalid());
            }
            history.push(super::thinking::chat_item(thinking)?);
        }
        // The live owner records prelude and call as separate Responses items.
        // A tool-only assistant's empty/null content is not an extra history item.
        if content != "" || calls.is_empty() {
            history.push(json!({"role":role,"content":content}));
        }
        for call in calls {
            let item = call_item(call)?;
            let id = item["call_id"].as_str().unwrap().to_owned();
            if !seen.insert(id.clone()) {
                return Err(invalid());
            }
            pending.insert(id, item["type"] == "custom_tool_call");
            history.push(item);
        }
    }
    if !pending.is_empty() || history.len() > 256 {
        return Err(invalid());
    }
    projected.insert("input".into(), Value::Array(history));
    let mut internal = req.clone();
    internal.endpoint_kind = EndpointKind::Responses;
    internal.raw_body = Value::Object(projected);
    Ok(internal)
}

fn content(value: &Value, role: &str) -> Result<Value, GatewayError> {
    let Some(parts) = value
        .as_array()
        .filter(|parts| parts.iter().any(|part| part["type"] == "image_url"))
    else {
        return text_output::parse(value, "text").map(Value::String);
    };
    if role != "user" || parts.len() > 128 {
        return Err(invalid());
    }
    parts.iter().map(|part| {
        let fields = part.as_object().ok_or_else(invalid)?;
        match part["type"].as_str() {
            Some("text") => {
                keys(fields, &["type", "text"])?;
                Ok(json!({"type":"input_text", "text":part["text"].as_str().ok_or_else(invalid)?}))
            }
            Some("image_url") => {
                keys(fields, &["type", "image_url"])?;
                let image = part["image_url"].as_object().ok_or_else(invalid)?;
                keys(image, &["url", "detail"])?;
                let mut projected = json!({"type":"input_image",
                    "image_url":image.get("url").ok_or_else(invalid)?});
                if let Some(detail) = image.get("detail") {
                    projected["detail"] = detail.clone();
                }
                Ok(projected)
            }
            _ => Err(invalid()),
        }
    }).collect::<Result<Vec<_>, _>>().map(Value::Array)
}

fn keys(fields: &Map<String, Value>, allowed: &[&str]) -> Result<(), GatewayError> {
    if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid());
    }
    Ok(())
}

fn definition(tool: &Value) -> Result<Value, GatewayError> {
    let fields = tool.as_object().ok_or_else(invalid)?;
    let kind = tool["type"].as_str().ok_or_else(invalid)?;
    if !["function", "custom"].contains(&kind) {
        return Err(invalid());
    }
    keys(fields, &["type", kind])?;
    let mut definition = tool[kind].as_object().cloned().ok_or_else(invalid)?;
    // Do not overwrite a caller's nested discriminator or silently remove fields.
    if definition.contains_key("type") {
        return Err(invalid());
    }
    definition.insert("type".into(), json!(kind));
    Ok(Value::Object(definition))
}

fn call_item(call: &Value) -> Result<Value, GatewayError> {
    let fields = call.as_object().ok_or_else(invalid)?;
    let kind = call["type"].as_str().ok_or_else(invalid)?;
    if !["function", "custom"].contains(&kind) {
        return Err(invalid());
    }
    keys(fields, &["id", "type", kind])?;
    let payload = call[kind].as_object().ok_or_else(invalid)?;
    let input = if kind == "custom" {
        "input"
    } else {
        "arguments"
    };
    keys(payload, &["name", input])?;
    let id = call["id"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 256)
        .ok_or_else(invalid)?;
    let mut item = json!({
        "type":if kind == "custom" {"custom_tool_call"} else {"function_call"},
        "call_id":id, "name":payload.get("name").ok_or_else(invalid)?
    });
    item[input] = payload.get(input).ok_or_else(invalid)?.clone();
    Ok(item)
}

#[cfg(test)]
#[path = "chat_request_tests.rs"]
mod tests;
