//! Deterministic native turns, including Responses' split assistant items and parallel tool results.
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole};
use serde_json::{json, Value};

pub(super) fn build(req: &CanonicalRelayRequest) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();
    for message in &req.messages {
        let (role, mut content) = match message.role {
            MessageRole::System => continue,
            MessageRole::User => ("user", parts(&message.content)),
            MessageRole::Assistant => {
                let mut content = if message.content.is_empty() && !message.tool_calls.is_empty() {
                    Vec::new()
                } else {
                    parts(&message.content)
                };
                for call in &message.tool_calls {
                    content.push(json!({"toolUse": {
                        "toolUseId":call.id,"name":call.name,
                        "input":call.arguments.as_deref().and_then(|v| serde_json::from_str::<Value>(v).ok()).unwrap_or_else(||json!({}))
                    }}));
                }
                ("assistant", content)
            }
            MessageRole::Tool => {
                let content: Vec<_> = message
                    .content
                    .iter()
                    .map(|part| match part {
                        ContentPart::Json { value } | ContentPart::Raw { value } => {
                            json!({"json":value})
                        }
                        _ => pack_part(part),
                    })
                    .collect();
                // ToolResultBlock has no name. A string is text, even when it looks like JSON.
                (
                    "user",
                    vec![
                        json!({"toolResult":{"toolUseId":message.tool_call_id,"content":content}}),
                    ],
                )
            }
        };
        if let Some(previous) = messages.last_mut().filter(|v| v["role"] == role) {
            previous["content"]
                .as_array_mut()
                .expect("constructed native content")
                .append(&mut content);
        } else {
            messages.push(json!({"role":role,"content":content}));
        }
    }
    messages
}

fn parts(parts: &[ContentPart]) -> Vec<Value> {
    if parts.is_empty() {
        return vec![json!({"text":""})];
    }
    parts.iter().map(pack_part).collect()
}
fn pack_part(part: &ContentPart) -> Value {
    match part {
        ContentPart::Text { text } => json!({"text":text}),
        ContentPart::Json { value } => json!({"json":value}),
        ContentPart::Raw { value } => value.clone(),
        ContentPart::ImageUrl { image_url, .. } => json!({"image":{
            "format":image_format(image_url),"source":{"url":image_url}
        }}),
    }
}
fn image_format(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "jpeg"
    } else if lower.ends_with(".webp") {
        "webp"
    } else if lower.ends_with(".gif") {
        "gif"
    } else {
        "png"
    }
}
