use serde_json::{json, Value};

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalToolCall, ContentPart, MessageRole,
};

use super::{guess_image_mime, value_to_string};

pub(super) fn build_contents(req: &CanonicalRelayRequest) -> Vec<Value> {
    let mut contents = Vec::new();
    for (index, msg) in req.messages.iter().enumerate() {
        if msg.role == MessageRole::System {
            continue;
        }
        if let Some(value) = pack_message(req, index, msg) {
            contents.push(value);
        }
    }
    contents
}

fn pack_message(
    req: &CanonicalRelayRequest,
    index: usize,
    msg: &CanonicalMessage,
) -> Option<Value> {
    match msg.role {
        MessageRole::System => None,
        MessageRole::Tool => {
            let id = msg
                .tool_call_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let name = find_tool_name(&req.messages[..index], &id)
                .or_else(|| msg.name.clone())
                .unwrap_or_else(|| "unknown".into());
            Some(json!({
                "role": "tool",
                "parts": [{
                    "thought": false,
                    "functionResponse": {
                        "id": id,
                        "name": name,
                        "responseJson": serde_json::to_string(&json!({
                            "content": message_payload_text(msg),
                            "is_error": false
                        })).unwrap_or_else(|_| "{\"content\":\"\",\"is_error\":false}".into())
                    }
                }]
            }))
        }
        MessageRole::Assistant => {
            let mut parts = pack_parts(&msg.content);
            for tool_call in &msg.tool_calls {
                parts.push(pack_tool_call_part(tool_call));
            }
            if parts.is_empty() {
                parts.push(json!({"text": "", "thought": false}));
            }
            Some(json!({"role": "model", "parts": parts}))
        }
        MessageRole::User => {
            let mut parts = pack_parts(&msg.content);
            if parts.is_empty() {
                parts.push(json!({"text": "", "thought": false}));
            }
            Some(json!({"role": "user", "parts": parts}))
        }
    }
}

fn pack_parts(parts: &[ContentPart]) -> Vec<Value> {
    parts.iter().filter_map(pack_part).collect()
}

fn pack_part(part: &ContentPart) -> Option<Value> {
    match part {
        ContentPart::Text { text } => Some(json!({"text": text, "thought": false})),
        ContentPart::ImageUrl { image_url, .. } => Some(json!({
            "thought": false,
            "file_data": {
                "file_uri": image_url,
                "mime_type": guess_image_mime(image_url)
            }
        })),
        ContentPart::Json { value } => Some(json!({
            "text": serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
            "thought": false
        })),
        ContentPart::Raw { value } => pack_raw_part(value),
    }
}

fn pack_raw_part(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let block_type = object.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match block_type {
        "text" | "input_text" | "output_text" => Some(json!({
            "text": object.get("text").and_then(|v| v.as_str()).unwrap_or(""),
            "thought": false
        })),
        "thinking" => {
            let mut part = json!({
                "text": object
                    .get("thinking")
                    .or_else(|| object.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                "thought": true
            });
            if let Some(signature) = object.get("signature").and_then(|v| v.as_str()) {
                if !signature.is_empty() {
                    part["thoughtSignature"] = json!(signature);
                }
            }
            Some(part)
        }
        "image" => {
            let source = object.get("source").and_then(|v| v.as_object())?;
            if source.get("type").and_then(|v| v.as_str()) != Some("url") {
                return None;
            }
            let image_url = source.get("url").and_then(|v| v.as_str())?;
            Some(json!({
                "thought": false,
                "file_data": {
                    "file_uri": image_url,
                    "mime_type": source
                        .get("media_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| guess_image_mime(image_url))
                }
            }))
        }
        "image_url" => {
            let image = object.get("image_url").and_then(|v| v.as_object())?;
            let image_url = image.get("url").and_then(|v| v.as_str())?;
            Some(json!({
                "thought": false,
                "file_data": {
                    "file_uri": image_url,
                    "mime_type": guess_image_mime(image_url)
                }
            }))
        }
        "tool_use" | "tool_call" | "function_call" => {
            let function = object.get("function").and_then(|v| v.as_object());
            let id = object
                .get("id")
                .or_else(|| object.get("call_id"))
                .or_else(|| object.get("tool_call_id"))
                .or_else(|| function.and_then(|func| func.get("id")))
                .and_then(value_to_string)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let name = object
                .get("name")
                .or_else(|| function.and_then(|func| func.get("name")))
                .and_then(value_to_string)
                .unwrap_or_default();
            let arguments = object
                .get("input")
                .or_else(|| object.get("arguments"))
                .or_else(|| object.get("arguments_json"))
                .or_else(|| function.and_then(|func| func.get("arguments")))
                .or_else(|| function.and_then(|func| func.get("arguments_json")))
                .map(|value| {
                    if let Some(text) = value.as_str() {
                        text.to_string()
                    } else {
                        serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
                    }
                })
                .unwrap_or_else(|| "{}".into());
            Some(json!({
                "thought": false,
                "functionCall": {
                    "id": id,
                    "name": name,
                    "argsJson": arguments
                }
            }))
        }
        _ => Some(json!({
            "text": serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
            "thought": false
        })),
    }
}

fn pack_tool_call_part(tool_call: &CanonicalToolCall) -> Value {
    json!({
        "thought": false,
        "functionCall": {
            "id": tool_call.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "name": tool_call.name.clone().unwrap_or_default(),
            "argsJson": tool_call.arguments.clone().unwrap_or_else(|| "{}".into())
        }
    })
}

fn find_tool_name(messages: &[CanonicalMessage], tool_call_id: &str) -> Option<String> {
    for msg in messages.iter().rev() {
        if msg.role != MessageRole::Assistant {
            continue;
        }
        for tool_call in &msg.tool_calls {
            if tool_call.id.as_deref() == Some(tool_call_id) {
                return tool_call.name.clone();
            }
        }
    }
    None
}

fn message_payload_text(msg: &CanonicalMessage) -> String {
    let mut parts = Vec::new();
    for part in &msg.content {
        match part {
            ContentPart::Text { text } => parts.push(text.clone()),
            ContentPart::ImageUrl { image_url, .. } => parts.push(image_url.clone()),
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                parts.push(serde_json::to_string(value).unwrap_or_else(|_| value.to_string()));
            }
        }
    }
    parts.join("\n")
}

pub(super) fn ensure_alternating_roles(contents: Vec<Value>) -> Vec<Value> {
    if contents.len() <= 1 {
        return contents;
    }
    fn side(role: &str) -> &str {
        if role == "model" {
            "model"
        } else {
            "user"
        }
    }
    let mut result = vec![contents[0].clone()];
    for content in contents.iter().skip(1) {
        let prev = result
            .last()
            .and_then(|v| v.get("role"))
            .and_then(|v| v.as_str())
            .unwrap_or("user");
        let current = content
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user");
        if side(prev) == side(current) {
            result.push(json!({
                "role": if side(current) == "model" { "user" } else { "model" },
                "parts": [{"text": "", "thought": false}]
            }));
        }
        result.push(content.clone());
    }
    result
}
