use serde_json::{json, Value};

use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole, ProtocolFamily};
use crate::protocol::tool_choice::{self, CanonicalToolChoice};

fn role_name_for_responses(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn pack_responses_content_parts_with_text_kind(
    parts: &[ContentPart],
    text_kind: &str,
) -> Vec<Value> {
    parts
        .iter()
        .map(|part| pack_responses_content_part(part, text_kind))
        .collect()
}

fn pack_responses_content_part(part: &ContentPart, text_kind: &str) -> Value {
    match part {
        ContentPart::Text { text } => json!({
            "type": text_kind,
            "text": text,
        }),
        ContentPart::ImageUrl { image_url, detail } => {
            let mut value = json!({
                "type": "input_image",
                "image_url": image_url,
            });
            if let Some(detail) = detail {
                value["detail"] = Value::String(detail.clone());
            }
            value
        }
        ContentPart::Json { value } | ContentPart::Raw { value } => {
            pack_responses_raw_content_part(value, text_kind)
        }
    }
}

fn pack_responses_raw_content_part(value: &Value, text_kind: &str) -> Value {
    match value {
        Value::Object(map) => match map.get("type").and_then(|entry| entry.as_str()) {
            Some("input_text") | Some("output_text") | Some("text") => json!({
                "type": text_kind,
                "text": map
                    .get("text")
                    .and_then(|entry| entry.as_str())
                    .unwrap_or_default(),
            }),
            Some(_) => value.clone(),
            None => json!({
                "type": text_kind,
                "text": value.to_string(),
            }),
        },
        Value::String(text) => json!({
            "type": text_kind,
            "text": text,
        }),
        _ => json!({
            "type": text_kind,
            "text": value.to_string(),
        }),
    }
}

fn pack_responses_content_parts(parts: &[ContentPart]) -> Vec<Value> {
    pack_responses_content_parts_with_text_kind(parts, "input_text")
}

fn pack_responses_message_content_parts(role: MessageRole, parts: &[ContentPart]) -> Vec<Value> {
    if role == MessageRole::Assistant && assistant_parts_are_empty_text(parts) {
        return Vec::new();
    }

    match role {
        MessageRole::Assistant => pack_responses_content_parts_with_text_kind(parts, "output_text"),
        _ => pack_responses_content_parts(parts),
    }
}

fn pack_responses_tool_output(parts: &[ContentPart]) -> Value {
    if parts.is_empty() {
        return Value::String(String::new());
    }

    let mut serialized_parts = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            ContentPart::Text { text } => serialized_parts.push(text.clone()),
            ContentPart::Json { value } => serialized_parts.push(value.to_string()),
            ContentPart::Raw { value } => {
                if value.get("type").and_then(|entry| entry.as_str()).is_some() {
                    return Value::Array(pack_responses_content_parts(parts));
                }
                serialized_parts.push(value.to_string());
            }
            ContentPart::ImageUrl { .. } => {
                return Value::Array(pack_responses_content_parts(parts))
            }
        }
    }

    if serialized_parts.len() == 1 {
        Value::String(serialized_parts.into_iter().next().unwrap_or_default())
    } else {
        Value::String(serialized_parts.join("\n"))
    }
}

fn assistant_parts_are_empty_text(parts: &[ContentPart]) -> bool {
    !parts.is_empty()
        && parts.iter().all(|part| {
            matches!(
                part,
                ContentPart::Text { text } if text.trim().is_empty()
            )
        })
}

fn infer_responses_instructions(req: &CanonicalRelayRequest) -> Option<String> {
    if let Some(instructions) = req
        .system_message()
        .filter(|value| !value.trim().is_empty())
    {
        return Some(instructions.to_string());
    }

    if req.messages.iter().any(|message| {
        message.role != MessageRole::System
            && message
                .content
                .iter()
                .any(|part| matches!(part, ContentPart::Text { text } if !text.trim().is_empty()))
    }) {
        return Some(
            "You are a helpful assistant. Follow the conversation in `input` and respond directly."
                .to_string(),
        );
    }

    None
}

/// Pack a [`CanonicalRelayRequest`] into an OpenAI Responses API request body.
pub fn pack_responses(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("model".to_string(), Value::String(model.to_string()));
    body.insert("stream".to_string(), Value::Bool(stream));

    if let Some(previous_response_id) = req.previous_response_id.as_deref() {
        body.insert(
            "previous_response_id".to_string(),
            Value::String(previous_response_id.to_string()),
        );
    }
    if let Some(instructions) = infer_responses_instructions(req) {
        body.insert("instructions".to_string(), Value::String(instructions));
    }

    let mut input_items = Vec::new();
    for message in &req.messages {
        if message.role == MessageRole::System {
            continue;
        }
        if message.role == MessageRole::Tool {
            input_items.push(json!({
                "type": "function_call_output",
                "call_id": message.tool_call_id.clone().unwrap_or_default(),
                "output": pack_responses_tool_output(&message.content),
            }));
            continue;
        }

        let content = pack_responses_message_content_parts(message.role, &message.content);
        if !content.is_empty() {
            input_items.push(json!({
                "role": role_name_for_responses(message.role),
                "content": content,
            }));
        }
        for tool_call in &message.tool_calls {
            input_items.push(json!({
                "type": "function_call",
                "call_id": tool_call.id.clone().unwrap_or_else(|| "call_auto".to_string()),
                "name": tool_call.name.clone().unwrap_or_else(|| "tool".to_string()),
                "arguments": tool_call.arguments.clone().unwrap_or_else(|| "{}".to_string()),
            }));
        }
    }

    if input_items.is_empty() {
        input_items.push(json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": req.messages_text(),
            }],
        }));
    }
    body.insert("input".to_string(), Value::Array(input_items));

    if !req.tools.is_empty() {
        body.insert(
            "tools".to_string(),
            Value::Array(
                req.tools
                    .iter()
                    .map(|tool| {
                        let mut item = serde_json::Map::new();
                        item.insert("type".to_string(), Value::String(tool.tool_type.clone()));
                        if let Some(name) = &tool.name {
                            item.insert("name".to_string(), Value::String(name.clone()));
                        }
                        if let Some(description) = &tool.description {
                            item.insert(
                                "description".to_string(),
                                Value::String(description.clone()),
                            );
                        }
                        if let Some(input_schema) = &tool.input_schema {
                            item.insert("parameters".to_string(), input_schema.clone());
                        }
                        for (key, value) in &tool.raw {
                            item.entry(key.clone()).or_insert_with(|| value.clone());
                        }
                        Value::Object(item)
                    })
                    .collect(),
            ),
        );
    }
    if let Some(tool_choice) =
        pack_responses_tool_choice(req.protocol_family, req.tool_choice.as_ref())
    {
        body.insert("tool_choice".to_string(), tool_choice);
    }

    if let Some(value) = req
        .extra
        .get("max_output_tokens")
        .or_else(|| req.extra.get("max_completion_tokens"))
        .or_else(|| req.extra.get("max_tokens"))
    {
        body.insert("max_output_tokens".to_string(), value.clone());
    }
    for passthrough_key in ["temperature", "top_p", "store"] {
        if let Some(value) = req.extra.get(passthrough_key) {
            body.insert(passthrough_key.to_string(), value.clone());
        }
    }
    for (key, value) in &req.extra {
        if key == "max_tokens" || key == "max_completion_tokens" || body.contains_key(key) {
            continue;
        }
        body.insert(key.clone(), value.clone());
    }

    Value::Object(body)
}

pub fn pack_responses_bridge(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = pack_responses(req, model, stream);
    if let Some(map) = body.as_object_mut() {
        map.retain(|key, _| {
            matches!(
                key.as_str(),
                "model"
                    | "stream"
                    | "previous_response_id"
                    | "instructions"
                    | "input"
                    | "tools"
                    | "tool_choice"
                    | "store"
            )
        });
    }
    body
}

fn pack_responses_tool_choice(
    source: ProtocolFamily,
    tool_choice: Option<&Value>,
) -> Option<Value> {
    let tool_choice = tool_choice?;
    let canonical = tool_choice::parse_tool_choice(Some(tool_choice));
    match canonical {
        Some(CanonicalToolChoice::Auto) => Some(Value::String("auto".to_string())),
        Some(CanonicalToolChoice::None) => Some(Value::String("none".to_string())),
        Some(CanonicalToolChoice::Required) => Some(Value::String("required".to_string())),
        Some(CanonicalToolChoice::Specific(name)) => Some(json!({
            "type": "function",
            "name": name,
        })),
        Some(CanonicalToolChoice::PromptOnly) => Some(Value::String("none".to_string())),
        None if source == ProtocolFamily::Anthropic => Some(Value::String("auto".to_string())),
        None => Some(tool_choice.clone()),
    }
}
