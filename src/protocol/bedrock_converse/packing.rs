//! Canonical request packing for the Bedrock Converse wire format.

use crate::protocol::canonical::CanonicalMessage;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::CanonicalTool;
use crate::protocol::canonical::ContentPart;
use crate::protocol::canonical::MessageRole;
use crate::protocol::tool_choice;
use serde_json::json;
use serde_json::Value;

pub fn pack_bedrock_converse(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = json!({
        "modelId": model,
        "messages": build_messages(req),
    });

    if let Some(system) = req.system_message().filter(|value| !value.is_empty()) {
        body["system"] = json!([{ "text": system }]);
    }

    if !req.tools.is_empty() || req.tool_choice.is_some() {
        let mut tool_config = json!({});
        if !req.tools.is_empty() {
            tool_config["tools"] = json!(req.tools.iter().map(pack_tool).collect::<Vec<_>>());
        }
        if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
            tool_config["toolChoice"] = tool_choice;
        }
        body["toolConfig"] = tool_config;
    }

    if stream {
        body["stream"] = json!(true);
    }

    for (key, value) in &req.extra {
        body[key] = value.clone();
    }

    body
}

fn build_messages(req: &CanonicalRelayRequest) -> Vec<Value> {
    let mut messages = Vec::new();
    for (index, msg) in req.messages.iter().enumerate() {
        if msg.role == MessageRole::System {
            continue;
        }
        if let Some(value) = pack_message(req, index, msg) {
            messages.push(value);
        }
    }
    messages
}

fn pack_message(
    req: &CanonicalRelayRequest,
    index: usize,
    msg: &CanonicalMessage,
) -> Option<Value> {
    match msg.role {
        MessageRole::System => None,
        MessageRole::User => Some(json!({
            "role": "user",
            "content": pack_content_parts(&msg.content),
        })),
        MessageRole::Assistant => {
            let mut content = pack_content_parts(&msg.content);
            for tool_call in &msg.tool_calls {
                content.push(json!({
                    "toolUse": {
                        "toolUseId": tool_call.id,
                        "name": tool_call.name,
                        "input": tool_call
                            .arguments
                            .as_deref()
                            .and_then(|value| serde_json::from_str::<Value>(value).ok())
                            .unwrap_or_else(|| json!({})),
                    }
                }));
            }
            Some(json!({
                "role": "assistant",
                "content": content,
            }))
        }
        MessageRole::Tool => {
            let tool_name = msg
                .name
                .clone()
                .or_else(|| find_tool_name(&req.messages[..index], msg.tool_call_id.as_deref()));
            let result_value = if msg.content.len() == 1 {
                match &msg.content[0] {
                    ContentPart::Json { value } => value.clone(),
                    _ => serde_json::from_str::<Value>(&msg.text_content())
                        .unwrap_or_else(|_| json!({ "content": msg.text_content() })),
                }
            } else {
                json!({ "content": msg.text_content() })
            };

            Some(json!({
                "role": "user",
                "content": [{
                    "toolResult": {
                        "toolUseId": msg.tool_call_id,
                        "status": "success",
                        "content": [{
                            "json": result_value
                        }],
                        "name": tool_name,
                    }
                }]
            }))
        }
    }
}

fn pack_content_parts(parts: &[ContentPart]) -> Vec<Value> {
    if parts.is_empty() {
        return vec![json!({"text": ""})];
    }

    parts
        .iter()
        .map(|part| match part {
            ContentPart::Text { text } => json!({"text": text}),
            ContentPart::Json { value } => json!({"json": value}),
            ContentPart::ImageUrl { image_url, .. } => json!({
                "image": {
                    "format": guess_image_format(image_url),
                    "source": { "url": image_url }
                }
            }),
            ContentPart::Raw { value } => value.clone(),
        })
        .collect()
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "toolSpec": {
            "name": tool.name,
            "description": tool.description,
            "inputSchema": {
                "json": tool
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }
        }
    })
}

fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_bedrock_tool_choice(tool_choice)
}

fn find_tool_name(history: &[CanonicalMessage], tool_call_id: Option<&str>) -> Option<String> {
    let tool_call_id = tool_call_id?;
    history.iter().rev().find_map(|message| {
        message.tool_calls.iter().find_map(|tool_call| {
            if tool_call.id.as_deref() == Some(tool_call_id) {
                tool_call.name.clone()
            } else {
                None
            }
        })
    })
}

fn guess_image_format(image_url: &str) -> &'static str {
    let lower = image_url.to_ascii_lowercase();
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
