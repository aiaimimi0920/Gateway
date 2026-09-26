//! Cohere request and success-response serialization behind the public adapter.

use serde_json::{json, Value};

use super::map_cohere_finish_reason;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    MessageRole, TokenUsage,
};
use crate::protocol::tool_choice;

pub fn pack_cohere(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut body = json!({
        "model": model,
        "stream": stream,
        "messages": req
            .messages
            .iter()
            .map(pack_message)
            .collect::<Vec<_>>(),
    });
    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(pack_tool).collect::<Vec<_>>());
    }
    if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
        body["tool_choice"] = tool_choice;
    }
    if let Some(reasoning) = &req.reasoning {
        body["thinking"] = reasoning.clone();
    }
    for (key, value) in &req.extra {
        body[key] = value.clone();
    }
    body
}

pub fn build_chat_v2_success(
    response_id: &str,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        content.push(json!({"text": text}));
    }
    let mut body = json!({
        "id": response_id,
        "model": model,
        "message": {
            "role": "assistant",
            "content": content,
        },
        "finish_reason": map_cohere_finish_reason(finish_reason, tool_calls),
    });
    if !tool_calls.is_empty() {
        body["message"]["tool_calls"] =
            json!(tool_calls.iter().map(pack_tool_call).collect::<Vec<_>>());
    }
    if let Some(usage) = usage {
        body["usage"] = json!({
            "input_tokens": usage.prompt_tokens,
            "output_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        });
    }
    body
}

pub(super) fn pack_message(message: &CanonicalMessage) -> Value {
    let role = match message.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };

    let content = if message.content.len() == 1 {
        if let Some(text) = message.content[0].as_text() {
            json!(text)
        } else {
            json!(message
                .content
                .iter()
                .map(pack_content_part)
                .collect::<Vec<_>>())
        }
    } else if message.content.is_empty() {
        json!("")
    } else {
        json!(message
            .content
            .iter()
            .map(pack_content_part)
            .collect::<Vec<_>>())
    };

    let mut value = json!({ "role": role, "content": content });
    if let Some(tool_call_id) = &message.tool_call_id {
        value["tool_call_id"] = json!(tool_call_id);
    }
    if !message.tool_calls.is_empty() {
        value["tool_calls"] = json!(message
            .tool_calls
            .iter()
            .map(pack_tool_call)
            .collect::<Vec<_>>());
    }
    value
}

pub(super) fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema.clone().unwrap_or_else(|| json!({"type":"object","properties":{}})),
        }
    })
}

pub(super) fn pack_tool_call(tool_call: &CanonicalToolCall) -> Value {
    json!({
        "id": tool_call.id,
        "type": "function",
        "function": { "name": tool_call.name, "arguments": tool_call.arguments }
    })
}

fn pack_content_part(part: &ContentPart) -> Value {
    match part {
        ContentPart::Text { text } => json!({"type": "text", "text": text}),
        ContentPart::Json { value } => json!({"type": "json", "value": value}),
        ContentPart::ImageUrl { image_url, .. } => json!({"type": "image_url", "url": image_url}),
        ContentPart::Raw { value } => value.clone(),
    }
}

pub(super) fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_cohere_tool_choice(tool_choice)
}
