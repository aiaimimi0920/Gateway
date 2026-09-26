use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
    EndpointKind, MessageRole, ProtocolFamily,
};

/// Normalize an Anthropic `POST /v1/messages` request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_messages(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // ── system ────────────────────────────────────────────────────────────
    // In Anthropic's API, `system` is a top-level string (or array of blocks).

    let mut messages: Vec<CanonicalMessage> = Vec::new();

    if let Some(sys) = body.get("system") {
        let sys_text = match sys {
            Value::String(s) => s.clone(),
            Value::Array(parts) => {
                // Handle array of content blocks (e.g., extended thinking format).
                parts
                    .iter()
                    .filter_map(|p| {
                        if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                            p.get("text").and_then(|t| t.as_str()).map(str::to_string)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => String::new(),
        };

        if !sys_text.is_empty() {
            messages.push(CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text { text: sys_text }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
    }

    // ── messages ──────────────────────────────────────────────────────────

    let raw_messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::bad_request("missing or invalid `messages` array"))?;

    for raw_msg in raw_messages {
        messages.extend(normalize_anthropic_message(raw_msg)?);
    }

    // ── tools ─────────────────────────────────────────────────────────────

    let tools = parse_anthropic_tools(body.get("tools"));
    let tool_choice = normalize_anthropic_tool_choice(body.get("tool_choice"));

    // ── extra ─────────────────────────────────────────────────────────────

    // Known top-level fields that are handled explicitly above.
    // NOTE: max_tokens is NOT listed here because pack_anthropic reads it from
    // extra.  Listing it would silently discard the parameter.
    const KNOWN_FIELDS: &[&str] = &[
        "model",
        "messages",
        "system",
        "stream",
        "tools",
        "tool_choice",
        "thinking",
    ];

    let mut extra = std::collections::HashMap::new();
    // Capture ALL unknown fields (including context_management, metadata, etc.)
    if let Value::Object(map) = &body {
        for (key, value) in map {
            if !KNOWN_FIELDS.contains(&key.as_str()) {
                extra.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::Anthropic,
        endpoint_kind: EndpointKind::Messages,
        requested_model: model,
        stream,
        messages,
        tools,
        tool_choice,
        reasoning: body.get("thinking").cloned(),
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra,
    })
}

fn normalize_anthropic_message(raw_msg: &Value) -> Result<Vec<CanonicalMessage>, GatewayError> {
    let role_str = raw_msg
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| GatewayError::bad_request("message missing `role` field"))?;

    match role_str {
        "user" => normalize_anthropic_user_message(raw_msg.get("content")),
        "assistant" => {
            normalize_anthropic_assistant_message(raw_msg.get("content")).map(|m| vec![m])
        }
        other => Err(GatewayError::bad_request(format!(
            "unexpected Anthropic message role: {other}"
        ))),
    }
}

fn normalize_anthropic_user_message(
    content: Option<&Value>,
) -> Result<Vec<CanonicalMessage>, GatewayError> {
    match content {
        None | Some(Value::Null) => Ok(vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }]),
        Some(Value::String(s)) => Ok(vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: s.clone() }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }]),
        Some(Value::Array(parts)) => {
            let mut out = Vec::new();
            let mut buffered = Vec::new();

            for part in parts {
                let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                if kind == "tool_result" {
                    if !buffered.is_empty() {
                        out.push(CanonicalMessage {
                            role: MessageRole::User,
                            content: std::mem::take(&mut buffered),
                            name: None,
                            tool_call_id: None,
                            tool_calls: vec![],
                        });
                    }
                    out.push(parse_anthropic_tool_result_message(part)?);
                    continue;
                }

                if let Some(content_part) = parse_anthropic_content_block(part)? {
                    buffered.push(content_part);
                }
            }

            if !buffered.is_empty() || out.is_empty() {
                out.push(CanonicalMessage {
                    role: MessageRole::User,
                    content: buffered,
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                });
            }

            Ok(out)
        }
        Some(other) => Ok(vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Raw {
                value: other.clone(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }]),
    }
}

fn normalize_anthropic_assistant_message(
    content: Option<&Value>,
) -> Result<CanonicalMessage, GatewayError> {
    match content {
        None | Some(Value::Null) => Ok(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }),
        Some(Value::String(s)) => Ok(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![ContentPart::Text { text: s.clone() }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }),
        Some(Value::Array(parts)) => {
            let mut content = Vec::new();
            let mut tool_calls = Vec::new();
            for part in parts {
                let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                if kind == "tool_use" {
                    tool_calls.push(parse_anthropic_tool_use_call(part));
                    continue;
                }
                if let Some(content_part) = parse_anthropic_content_block(part)? {
                    content.push(content_part);
                }
            }

            Ok(CanonicalMessage {
                role: MessageRole::Assistant,
                content,
                name: None,
                tool_call_id: None,
                tool_calls,
            })
        }
        Some(other) => Ok(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![ContentPart::Raw {
                value: other.clone(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }),
    }
}

fn parse_anthropic_tool_result_message(part: &Value) -> Result<CanonicalMessage, GatewayError> {
    let content = parse_anthropic_tool_result_content(part.get("content"))?;
    Ok(CanonicalMessage {
        role: MessageRole::Tool,
        content,
        name: None,
        tool_call_id: part
            .get("tool_use_id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        tool_calls: vec![],
    })
}

fn parse_anthropic_tool_result_content(
    content: Option<&Value>,
) -> Result<Vec<ContentPart>, GatewayError> {
    parse_anthropic_content(content)
}

fn parse_anthropic_tool_use_call(part: &Value) -> CanonicalToolCall {
    CanonicalToolCall {
        id: part.get("id").and_then(|v| v.as_str()).map(str::to_string),
        call_type: "function".to_string(),
        name: part
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        arguments: part.get("input").map(|v| v.to_string()),
        raw: std::collections::HashMap::new(),
    }
}

fn parse_anthropic_content_block(part: &Value) -> Result<Option<ContentPart>, GatewayError> {
    let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("text");
    Ok(Some(match kind {
        "text" => {
            let text = part
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if is_plain_text_block(part) {
                ContentPart::Text { text }
            } else {
                ContentPart::Raw {
                    value: part.clone(),
                }
            }
        }
        "image" => {
            let url = part
                .get("source")
                .and_then(|s| s.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ContentPart::ImageUrl {
                image_url: url,
                detail: None,
            }
        }
        "tool_use" | "tool_result" => return Ok(None),
        _ => ContentPart::Raw {
            value: part.clone(),
        },
    }))
}

fn parse_anthropic_content(content: Option<&Value>) -> Result<Vec<ContentPart>, GatewayError> {
    match content {
        None | Some(Value::Null) => Ok(vec![]),

        Some(Value::String(s)) => Ok(vec![ContentPart::Text { text: s.clone() }]),

        Some(Value::Array(parts)) => {
            let mut out = Vec::with_capacity(parts.len());
            for part in parts {
                if let Some(content_part) = parse_anthropic_content_block(part)? {
                    out.push(content_part);
                }
            }
            Ok(out)
        }

        Some(other) => Ok(vec![ContentPart::Raw {
            value: other.clone(),
        }]),
    }
}

fn parse_anthropic_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|t| CanonicalTool {
            tool_type: "function".to_string(),
            name: t.get("name").and_then(|v| v.as_str()).map(str::to_string),
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            input_schema: t.get("input_schema").cloned(),
            raw: t
                .as_object()
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

fn normalize_anthropic_tool_choice(raw: Option<&Value>) -> Option<Value> {
    let raw = raw?;

    if let Some(text) = raw.as_str() {
        return Some(Value::String(
            match text {
                "none" => "none",
                "required" | "any" => "required",
                _ => "auto",
            }
            .to_string(),
        ));
    }

    let Some(object) = raw.as_object() else {
        return Some(raw.clone());
    };

    match object
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("auto")
    {
        "any" => Some(Value::String("required".to_string())),
        "none" => Some(Value::String("none".to_string())),
        "tool" => object
            .get("name")
            .and_then(|value| value.as_str())
            .map(|name| {
                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                    }
                })
            }),
        _ => Some(Value::String("auto".to_string())),
    }
}

fn is_plain_text_block(part: &Value) -> bool {
    part.as_object().is_some_and(|map| {
        map.keys()
            .all(|key| matches!(key.as_str(), "type" | "text"))
    })
}
