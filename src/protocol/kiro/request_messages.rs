use std::collections::{HashMap, HashSet};

use base64::Engine;
use serde_json::{json, Map, Value};

use super::{restore_tool_name, sha256_hex};
use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, ContentPart, MessageRole,
};

const TOOL_NAME_MAX_LEN: usize = 63;

#[derive(Debug)]
struct ExtractedUserParts {
    text: String,
    images: Vec<Value>,
}

pub(super) fn pack_tool(
    tool: &CanonicalTool,
    short_tool_name_map: &HashMap<String, String>,
) -> Value {
    let original_name = tool.name.clone().unwrap_or_default();
    let name = short_tool_name_map
        .get(&original_name)
        .cloned()
        .unwrap_or(original_name);
    json!({
        "toolSpecification": {
            "name": name,
            "description": tool.description.clone().unwrap_or_default(),
            "inputSchema": {
                "json": tool.input_schema.clone().unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }
        }
    })
}

pub(super) fn build_history_messages(
    req: &CanonicalRelayRequest,
    messages: &[&CanonicalMessage],
    model: &str,
    short_tool_name_map: &HashMap<String, String>,
) -> Result<Vec<Value>, GatewayError> {
    let mut history = Vec::new();
    if let Some(system_message) = req.system_message().filter(|text| !text.trim().is_empty()) {
        history.push(json!({
            "userInputMessage": {
                "content": system_message,
                "modelId": model,
                "origin": "AI_EDITOR",
            }
        }));
        history.push(json!({
            "assistantResponseMessage": {
                "content": "I will follow these instructions."
            }
        }));
    }

    let mut index = 0usize;
    while index < messages.len() {
        match messages[index].role {
            MessageRole::User | MessageRole::Tool => {
                let start = index;
                while index < messages.len()
                    && matches!(messages[index].role, MessageRole::User | MessageRole::Tool)
                {
                    index += 1;
                }
                history.push(json!({ "userInputMessage": build_user_message(&messages[start..index], model)? }));
            }
            MessageRole::Assistant => {
                let start = index;
                while index < messages.len() && messages[index].role == MessageRole::Assistant {
                    index += 1;
                }
                history.push(json!({
                    "assistantResponseMessage": build_assistant_message(&messages[start..index], short_tool_name_map)
                }));
            }
            MessageRole::System => {
                index += 1;
            }
        }
    }
    Ok(history)
}

pub(super) fn build_user_message(
    messages: &[&CanonicalMessage],
    model: &str,
) -> Result<Value, GatewayError> {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();
    let mut tool_results = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::User => {
                let extracted = extract_text_and_images(&message.content)?;
                if !extracted.text.is_empty() {
                    text_parts.push(extracted.text);
                }
                images.extend(extracted.images);
            }
            MessageRole::Tool => {
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    continue;
                };
                tool_results.push(json!({
                    "toolUseId": tool_call_id,
                    "content": [{ "text": message_text(message) }],
                    "status": "success",
                }));
            }
            _ => {}
        }
    }

    let content = {
        let joined = text_parts.join("\n");
        if joined.trim().is_empty() && !tool_results.is_empty() {
            " ".to_string()
        } else {
            joined
        }
    };

    let mut object = Map::new();
    object.insert("content".to_string(), Value::String(content));
    object.insert("modelId".to_string(), Value::String(model.to_string()));
    object.insert("origin".to_string(), Value::String("AI_EDITOR".to_string()));
    if !images.is_empty() {
        object.insert("images".to_string(), Value::Array(images));
    }
    if !tool_results.is_empty() {
        object.insert(
            "userInputMessageContext".to_string(),
            json!({ "toolResults": tool_results }),
        );
    }
    Ok(Value::Object(object))
}

fn build_assistant_message(
    messages: &[&CanonicalMessage],
    short_tool_name_map: &HashMap<String, String>,
) -> Value {
    let mut content_parts = Vec::new();
    let mut tool_uses = Vec::new();

    for message in messages {
        let text = message_text(message);
        if !text.is_empty() {
            content_parts.push(text);
        }
        for tool_call in &message.tool_calls {
            let name = tool_call.name.clone().unwrap_or_default();
            let mapped_name = short_tool_name_map.get(&name).cloned().unwrap_or(name);
            let input = tool_call
                .arguments
                .as_deref()
                .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
                .unwrap_or_else(|| json!({}));
            tool_uses.push(json!({
                "toolUseId": tool_call.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                "name": mapped_name,
                "input": input,
            }));
        }
    }

    let content = {
        let joined = content_parts.join("\n\n");
        if joined.trim().is_empty() && !tool_uses.is_empty() {
            " ".to_string()
        } else {
            joined
        }
    };

    let mut object = Map::new();
    object.insert("content".to_string(), Value::String(content));
    if !tool_uses.is_empty() {
        object.insert("toolUses".to_string(), Value::Array(tool_uses));
    }
    Value::Object(object)
}

pub(super) fn ensure_history_tools_declared(
    history: &[Value],
    tools: &mut Vec<Value>,
    tool_name_map: &HashMap<String, String>,
) {
    let mut declared = tools
        .iter()
        .filter_map(|tool| {
            tool.get("toolSpecification")
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
                .map(str::to_ascii_lowercase)
        })
        .collect::<HashSet<_>>();

    for entry in history {
        let Some(tool_uses) = entry
            .get("assistantResponseMessage")
            .and_then(|value| value.get("toolUses"))
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for tool_use in tool_uses {
            let Some(name) = tool_use.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            if !declared.insert(name.to_ascii_lowercase()) {
                continue;
            }
            let description = restore_tool_name(tool_name_map, name);
            tools.push(json!({
                "toolSpecification": {
                    "name": name,
                    "description": format!("History tool placeholder for {description}"),
                    "inputSchema": { "json": {"type":"object","properties":{}} }
                }
            }));
        }
    }
}

pub(super) fn validate_tool_pairing(history: &mut [Value], current: &mut Value) {
    let all_tool_use_ids = history
        .iter()
        .filter_map(|entry| {
            entry
                .get("assistantResponseMessage")
                .and_then(|value| value.get("toolUses"))
                .and_then(|value| value.as_array())
        })
        .flat_map(|tool_uses| tool_uses.iter())
        .filter_map(|tool_use| {
            tool_use
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<HashSet<_>>();

    let resolved_tool_result_ids = history
        .iter()
        .filter_map(|entry| {
            entry
                .get("userInputMessage")
                .and_then(|value| value.get("userInputMessageContext"))
                .and_then(|value| value.get("toolResults"))
                .and_then(|value| value.as_array())
        })
        .flat_map(|tool_results| tool_results.iter())
        .filter_map(|tool_result| {
            tool_result
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<HashSet<_>>();

    if let Some(results) = current
        .get_mut("userInputMessageContext")
        .and_then(|value| value.get_mut("toolResults"))
        .and_then(|value| value.as_array_mut())
    {
        results.retain(|tool_result| {
            tool_result
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(|tool_use_id| all_tool_use_ids.contains(tool_use_id))
                .unwrap_or(false)
        });
    }

    let current_result_ids = current
        .get("userInputMessageContext")
        .and_then(|value| value.get("toolResults"))
        .and_then(|value| value.as_array())
        .map(|results| {
            results
                .iter()
                .filter_map(|tool_result| {
                    tool_result
                        .get("toolUseId")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    for entry in history.iter_mut() {
        let Some(tool_uses) = entry
            .get_mut("assistantResponseMessage")
            .and_then(|value| value.get_mut("toolUses"))
            .and_then(|value| value.as_array_mut())
        else {
            continue;
        };
        tool_uses.retain(|tool_use| {
            let Some(tool_use_id) = tool_use.get("toolUseId").and_then(|value| value.as_str())
            else {
                return false;
            };
            resolved_tool_result_ids.contains(tool_use_id)
                || current_result_ids.contains(tool_use_id)
        });
    }
}

pub(super) fn build_tool_name_map(req: &CanonicalRelayRequest) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for tool in &req.tools {
        let Some(name) = tool.name.as_deref() else {
            continue;
        };
        if let Some(short_name) = maybe_shortened_tool_name(name) {
            map.insert(short_name, name.to_string());
        }
    }
    map
}

pub(super) fn build_short_tool_name_map(
    tool_name_map: &HashMap<String, String>,
) -> HashMap<String, String> {
    tool_name_map
        .iter()
        .map(|(short, original)| (original.clone(), short.clone()))
        .collect()
}

fn maybe_shortened_tool_name(name: &str) -> Option<String> {
    if name.chars().count() <= TOOL_NAME_MAX_LEN {
        return None;
    }
    let hash = sha256_hex(name);
    let suffix = &hash[..8];
    let prefix_max = TOOL_NAME_MAX_LEN.saturating_sub(1 + suffix.len());
    let mut prefix = String::new();
    for ch in name.chars().take(prefix_max) {
        prefix.push(ch);
    }
    Some(format!("{prefix}_{suffix}"))
}

fn extract_text_and_images(parts: &[ContentPart]) -> Result<ExtractedUserParts, GatewayError> {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    for part in parts {
        match part {
            ContentPart::Text { text } => {
                if !text.is_empty() {
                    text_parts.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
                if !serialized.is_empty() {
                    text_parts.push(serialized);
                }
            }
            ContentPart::ImageUrl { image_url, .. } => {
                if let Some(image) = pack_image_url_as_kiro_image(image_url)? {
                    images.push(image);
                }
            }
        }
    }

    Ok(ExtractedUserParts {
        text: text_parts.join("\n"),
        images,
    })
}

fn pack_image_url_as_kiro_image(image_url: &str) -> Result<Option<Value>, GatewayError> {
    let Some(rest) = image_url.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((meta, bytes)) = rest.split_once(',') else {
        return Ok(None);
    };
    if !meta.to_ascii_lowercase().contains(";base64") {
        return Ok(None);
    }

    let media_type = meta.split(';').next().unwrap_or("image/png");
    let format = match media_type.to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpeg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => {
            return Err(GatewayError::bad_request(format!(
                "Unsupported Kiro image media type: {media_type}"
            ))
            .with_code("kiro_unsupported_image_media_type"))
        }
    };

    let decoded = percent_decode_data_uri(bytes);
    let data = base64::engine::general_purpose::STANDARD
        .decode(decoded)
        .map_err(|error| {
            GatewayError::bad_request(format!("Invalid Kiro image base64 payload: {error}"))
                .with_code("kiro_invalid_image_base64")
        })?;
    let base64_bytes = base64::engine::general_purpose::STANDARD.encode(data);

    Ok(Some(json!({
        "format": format,
        "source": {
            "bytes": base64_bytes,
        }
    })))
}

fn percent_decode_data_uri(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn message_text(message: &CanonicalMessage) -> String {
    let mut parts = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => parts.push(text.clone()),
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                parts.push(serde_json::to_string(value).unwrap_or_else(|_| value.to_string()));
            }
            ContentPart::ImageUrl { image_url, .. } => parts.push(image_url.clone()),
        }
    }
    parts.join("\n")
}

pub(super) fn resolve_conversation_id(req: &CanonicalRelayRequest) -> String {
    req.explicit_session_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            req.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user_id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
