//! SAP v2 tools live in the prompt module; function execution remains client-side.
use super::super::invalid;
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, ContentPart, MessageRole},
        openai_tool_wire,
    },
};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

fn function_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn keys(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|map| map.keys().all(|key| allowed.contains(&key.as_str())))
}

pub(super) fn definitions(req: &CanonicalRelayRequest) -> Result<Vec<Value>, GatewayError> {
    if let Some(raw) = req.raw_body.get("tools") {
        let raw = raw
            .as_array()
            .filter(|tools| !tools.is_empty() && tools.len() == req.tools.len())
            .ok_or_else(invalid)?;
        if raw.iter().any(|tool| {
            tool["type"] != "function"
                || !keys(tool, &["type", "function"])
                || !keys(
                    &tool["function"],
                    &["name", "description", "parameters", "strict"],
                )
        }) {
            return Err(invalid());
        }
    } else if !req.tools.is_empty() {
        return Err(invalid());
    }
    let mut names = HashSet::new();
    req.tools
        .iter()
        .map(|tool| {
            if tool.tool_type != "function" {
                return Err(invalid());
            }
            let packed = openai_tool_wire::chat_tool(tool, true);
            let function = &packed["function"];
            let name = tool
                .name
                .as_deref()
                .filter(|name| function_name(name))
                .ok_or_else(invalid)?;
            if !names.insert(name)
                || !keys(function, &["name", "description", "parameters", "strict"])
                || function.get("description").is_some_and(|v| !v.is_string())
                || function.get("parameters").is_some_and(|v| !v.is_object())
                || function
                    .get("strict")
                    .is_some_and(|v| !v.is_boolean() && !v.is_null())
            {
                return Err(invalid());
            }
            Ok(packed)
        })
        .collect()
}

fn full_call(value: &Value) -> Option<(&str, &str, &str)> {
    let id = value["id"].as_str().filter(|id| !id.trim().is_empty())?;
    let function = &value["function"];
    let name = function["name"]
        .as_str()
        .filter(|name| function_name(name))?;
    let arguments = function["arguments"].as_str()?;
    (value["type"] == "function"
        && keys(value, &["id", "type", "function"])
        && keys(function, &["name", "arguments"]))
    .then_some((id, name, arguments))
}

fn user_content(value: &Value, images: bool) -> bool {
    value.is_string()
        || (images
            && value.as_array().is_some_and(|items| {
                !items.is_empty()
                    && items.iter().all(|item| match item["type"].as_str() {
                        Some("text") => keys(item, &["type", "text"]) && item["text"].is_string(),
                        Some("image_url") => keys(item, &["type", "image_url"]),
                        _ => false,
                    })
            }))
}

fn multipart(parts: &[ContentPart]) -> Result<Value, GatewayError> {
    let blocks = parts
        .iter()
        .map(|part| match part {
            ContentPart::Text { text } => Ok(json!({"type":"text", "text":text})),
            ContentPart::ImageUrl { image_url, detail } => {
                let mut image = json!({"url":image_url});
                if let Some(detail) = detail {
                    image["detail"] = json!(detail);
                }
                Ok(json!({"type":"image_url", "image_url":image}))
            }
            _ => Err(invalid()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::Array(blocks))
}

fn assistant_keys(message: &Value, reasoning: bool) -> bool {
    keys(
        message,
        &["role", "content", "tool_calls", "reasoning_content"],
    ) && message
        .get("reasoning_content")
        .is_none_or(|value| reasoning && super::reasoning::valid_blocks(value))
}

pub(super) fn history(
    req: &CanonicalRelayRequest,
    images: bool,
    reasoning: bool,
) -> Result<Vec<Value>, GatewayError> {
    let raw = req.raw_body["messages"].as_array().ok_or_else(invalid)?;
    let mut originals = HashMap::new();
    for message in raw {
        let role = message["role"].as_str().ok_or_else(invalid)?;
        let valid = match role {
            "user" => {
                keys(message, &["role", "content"]) && user_content(&message["content"], images)
            }
            "system" => keys(message, &["role", "content"]) && message["content"].is_string(),
            "tool" => {
                keys(message, &["role", "content", "tool_call_id"])
                    && message["content"].is_string()
                    && message["tool_call_id"].is_string()
            }
            "assistant" if message.get("tool_calls").is_some() => {
                let calls = message["tool_calls"]
                    .as_array()
                    .filter(|calls| !calls.is_empty())
                    .ok_or_else(invalid)?;
                for call in calls {
                    let (id, name, args) = full_call(call).ok_or_else(invalid)?;
                    if originals.insert(id, (name, args)).is_some() {
                        return Err(invalid());
                    }
                }
                assistant_keys(message, reasoning)
                    && (message["content"].is_string() || message["content"].is_null())
            }
            "assistant" => assistant_keys(message, reasoning) && message["content"].is_string(),
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
    }

    let mut pending = HashSet::new();
    let mut history = Vec::with_capacity(req.messages.len());
    for msg in &req.messages {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        if msg.name.is_some()
            || (role != "tool" && (!pending.is_empty() || msg.tool_call_id.is_some()))
        {
            return Err(invalid());
        }
        let mut out = json!({"role":role});
        // Respect canonical text transforms while leaving opaque tool arguments byte-for-byte intact.
        match msg.content.as_slice() {
            [ContentPart::Text { text }] => out["content"] = json!(text),
            [] if role == "assistant" && !msg.tool_calls.is_empty() => {}
            parts if images && role == "user" && !parts.is_empty() => {
                out["content"] = multipart(parts)?
            }
            _ => return Err(invalid()),
        }
        if role == "tool" {
            let id = msg
                .tool_call_id
                .as_deref()
                .filter(|id| pending.remove(id))
                .ok_or_else(invalid)?;
            out["tool_call_id"] = json!(id);
        }
        if !msg.tool_calls.is_empty() {
            if role != "assistant" {
                return Err(invalid());
            }
            let calls = msg
                .tool_calls
                .iter()
                .map(|call| {
                    let id = call.id.as_deref().ok_or_else(invalid)?;
                    let (name, args) = originals.remove(id).ok_or_else(invalid)?;
                    if call.call_type != "function"
                        || call.name.as_deref() != Some(name)
                        || !pending.insert(id)
                    {
                        return Err(invalid());
                    }
                    Ok(json!({"id":id,"type":"function","function":{"name":name,"arguments":args}}))
                })
                .collect::<Result<Vec<_>, _>>()?;
            out["tool_calls"] = json!(calls);
        }
        history.push(out);
    }
    if !pending.is_empty()
        || !originals.is_empty()
        || !history.iter().any(|msg| msg["role"] == "user")
    {
        return Err(invalid());
    }
    Ok(history)
}

pub(super) fn request_body(
    selection: Value,
    mut history: Vec<Value>,
    definitions: Vec<Value>,
    stream: bool,
) -> Result<Value, GatewayError> {
    let mut prompt_templating = json!({"model":selection});
    let mut body = Map::new();
    if !definitions.is_empty() {
        // SAP appends the template after history and requires at least one template message.
        // Only a Gateway-owned placeholder is templated; caller text is passed as a value.
        let mut template = history.pop().ok_or_else(invalid)?;
        if template["role"] != "user" && template["role"] != "tool" {
            return Err(invalid());
        }
        let mut values = Map::new();
        if let Some(parts) = template["content"].as_array_mut() {
            // Only scalar values are substituted; the template owns every block and field.
            for (index, part) in parts.iter_mut().enumerate() {
                let key = format!("gateway_part_{index}");
                let value = if part["type"] == "text" {
                    &mut part["text"]
                } else {
                    &mut part["image_url"]["url"]
                };
                values.insert(key.clone(), value.take());
                *value = json!(format!("{{{{?{key}}}}}"));
            }
        } else {
            values.insert("gateway_content".into(), template["content"].take());
            template["content"] = json!("{{?gateway_content}}");
        }
        prompt_templating["prompt"] = json!({"template":[template],"tools":definitions});
        body.insert("placeholder_values".into(), Value::Object(values));
    }
    let mut config = json!({"modules":{"prompt_templating":prompt_templating}});
    if stream {
        config["stream"] = json!({"enabled":true});
    }
    body.insert("config".into(), config);
    body.insert("messages_history".into(), json!(history));
    Ok(Value::Object(body))
}

pub(crate) fn valid_final_calls(message: &Value, finish: &Value, allow_tools: bool) -> bool {
    if !message["function_call"].is_null() || finish == "function_call" {
        return false;
    }
    if message["tool_calls"].is_null() {
        return finish != "tool_calls";
    }
    let Some(calls) = message["tool_calls"].as_array() else {
        return false;
    };
    if calls.is_empty() {
        return finish != "tool_calls";
    }
    let mut ids = HashSet::new();
    allow_tools
        && finish == "tool_calls"
        && calls
            .iter()
            .all(|call| full_call(call).is_some_and(|(id, _, _)| ids.insert(id)))
}

pub(crate) fn valid_chunks(calls: &[Value]) -> bool {
    calls.iter().all(|call| {
        keys(call, &["index", "id", "type", "function"])
            && call
                .get("function")
                .is_none_or(|function| keys(function, &["name", "arguments"]))
    })
}
