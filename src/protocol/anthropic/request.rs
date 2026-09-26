use serde_json::{json, Value};

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, ContentPart, MessageRole,
    ProtocolFamily,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PromptCacheTelemetry {
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
}

#[derive(Debug, Clone)]
pub struct PackedAnthropicRequest {
    pub body: Value,
    pub prompt_cache_telemetry: PromptCacheTelemetry,
}

// ---------------------------------------------------------------------------
// pack_anthropic
// ---------------------------------------------------------------------------

/// Pack a [`CanonicalRelayRequest`] into an Anthropic /v1/messages JSON body.
pub fn pack_anthropic(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    pack_anthropic_with_telemetry(req, model, stream).body
}

pub fn inspect_prompt_cache_telemetry(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> PromptCacheTelemetry {
    pack_anthropic_with_telemetry(req, model, stream).prompt_cache_telemetry
}

pub fn pack_anthropic_with_telemetry(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> PackedAnthropicRequest {
    let mut body = build_anthropic_request_body(req, model, stream);
    let client_has_cache_control = has_existing_cache_control(&body);
    let auto_cache_applied = apply_auto_cache_markers(&mut body, model);
    strip_gateway_auto_cache_controls(&mut body);

    PackedAnthropicRequest {
        body,
        prompt_cache_telemetry: PromptCacheTelemetry {
            client_has_cache_control,
            auto_cache_applied,
        },
    }
}

fn build_anthropic_request_body(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    // Separate system message from conversation history.
    let canonical_system = req.messages.iter().find(|m| m.role == MessageRole::System);

    let json_messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != MessageRole::System)
        .map(pack_anthropic_message)
        .collect();

    // max_tokens is REQUIRED by Anthropic — default to 4096.
    let max_tokens = req
        .extra
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(4096);

    let mut body = json!({
        "model": model,
        "messages": json_messages,
        "max_tokens": max_tokens,
        "stream": stream,
    });

    if req.protocol_family == ProtocolFamily::Anthropic {
        if let Some(raw_system) = req.raw_body.get("system") {
            body["system"] = raw_system.clone();
        } else if let Some(system) = canonical_system {
            body["system"] = pack_anthropic_system(system);
        }
    } else if let Some(system) = canonical_system {
        body["system"] = pack_anthropic_system(system);
    }

    // ── extra parameters (merge ALL back into body) ────────────────────────
    //
    // Keys already present in body (model, messages, max_tokens, stream,
    // system) will be overridden if they appear in extra, which is intentional
    // so that callers can fine-tune the final body.
    for (k, v) in &req.extra {
        match k.as_str() {
            // max_tokens is already handled above — skip to avoid double write
            "max_tokens" => {}
            _ => {
                body.as_object_mut()
                    .unwrap()
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
        }
    }

    // ── tools ─────────────────────────────────────────────────────────────

    if req.protocol_family == ProtocolFamily::Anthropic {
        if let Some(raw_tools) = req.raw_body.get("tools") {
            body["tools"] = raw_tools.clone();
        } else if !req.tools.is_empty() {
            body["tools"] = json!(req
                .tools
                .iter()
                .map(pack_anthropic_tool)
                .collect::<Vec<_>>());
        }
    } else if !req.tools.is_empty() {
        body["tools"] = json!(req
            .tools
            .iter()
            .map(pack_anthropic_tool)
            .collect::<Vec<_>>());
    }
    if let Some(tc) = pack_anthropic_tool_choice(req.protocol_family, req.tool_choice.as_ref()) {
        body["tool_choice"] = tc;
    }

    // ── extended thinking ─────────────────────────────────────────────────

    if let Some(thinking) = &req.reasoning {
        body["thinking"] = thinking.clone();
    }

    body
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn is_claude_model(model: &str) -> bool {
    model.to_ascii_lowercase().contains("claude")
}

fn has_existing_cache_control(body: &Value) -> bool {
    value_contains_cache_control(body)
}

fn value_contains_cache_control(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.contains_key("cache_control") || map.values().any(value_contains_cache_control)
        }
        Value::Array(values) => values.iter().any(value_contains_cache_control),
        _ => false,
    }
}

fn apply_auto_cache_markers(body: &mut Value, model: &str) -> bool {
    let client_has_cache_control = has_existing_cache_control(body);
    apply_auto_cache_markers_with_known_state(body, model, client_has_cache_control)
}

fn apply_auto_cache_markers_with_known_state(
    body: &mut Value,
    model: &str,
    client_has_cache_control: bool,
) -> bool {
    if !is_claude_model(model) || client_has_cache_control || auto_cache_disabled(body) {
        return false;
    }

    let Some(object) = body.as_object_mut() else {
        return false;
    };
    if object.contains_key("cache_control") {
        return false;
    }
    object.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
    true
}

fn auto_cache_disabled(body: &Value) -> bool {
    for key in ["gateway_auto_cache", "neuro_auto_cache"] {
        let Some(value) = body.get(key) else {
            continue;
        };
        match value {
            Value::Bool(flag) => {
                if !flag {
                    return true;
                }
            }
            Value::String(text) => {
                let normalized = text.trim().to_ascii_lowercase();
                if matches!(normalized.as_str(), "false" | "0" | "off" | "no") {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn is_gateway_auto_cache_control_key(key: &str) -> bool {
    matches!(key, "gateway_auto_cache" | "neuro_auto_cache")
}

fn strip_gateway_auto_cache_controls(body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    object.retain(|key, _| !is_gateway_auto_cache_control_key(key));
}

fn pack_anthropic_tool_choice(
    source: ProtocolFamily,
    tool_choice: Option<&Value>,
) -> Option<Value> {
    let tool_choice = tool_choice?;
    if source == ProtocolFamily::Anthropic {
        return Some(tool_choice.clone());
    }

    if let Some(text) = tool_choice.as_str() {
        return Some(match text {
            "auto" | "none" => json!({ "type": text }),
            "required" => json!({ "type": "any" }),
            other => json!({ "type": other }),
        });
    }

    let Some(object) = tool_choice.as_object() else {
        return Some(tool_choice.clone());
    };

    let choice_type = object.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match choice_type {
        "function" => object
            .get("function")
            .and_then(|value| value.get("name"))
            .and_then(|value| value.as_str())
            .or_else(|| object.get("name").and_then(|value| value.as_str()))
            .map(|name| json!({ "type": "tool", "name": name })),
        "required" => Some(json!({ "type": "any" })),
        "auto" | "none" => Some(json!({ "type": choice_type })),
        _ => Some(tool_choice.clone()),
    }
}

fn pack_anthropic_message(msg: &CanonicalMessage) -> Value {
    // System messages are extracted to the top-level `system` field by
    // pack_anthropic; they should never appear in the messages array.
    let role = match msg.role {
        MessageRole::User | MessageRole::Tool => "user",
        MessageRole::Assistant => "assistant",
        // System should not reach here, but map to user as a safe fallback.
        MessageRole::System => "user",
    };

    // For Tool role messages, build a tool_result content block.
    if msg.role == MessageRole::Tool {
        let mut content_block = json!({
            "type": "tool_result",
            "content": pack_anthropic_tool_result_content(&msg.content),
        });

        // Include tool_use_id if present.
        if let Some(tcid) = &msg.tool_call_id {
            content_block["tool_use_id"] = json!(tcid);
        }

        return json!({
            "role": role,
            "content": [content_block],
        });
    }

    // If the assistant message has tool_calls, emit them as tool_use content blocks.
    if msg.role == MessageRole::Assistant && !msg.tool_calls.is_empty() {
        let mut content_blocks: Vec<Value> = Vec::new();

        // Include any text content first.
        for part in &msg.content {
            if let Some(t) = part.as_text() {
                if !t.is_empty() {
                    content_blocks.push(json!({"type": "text", "text": t}));
                }
            }
        }

        // Append tool_use blocks.
        for tc in &msg.tool_calls {
            let input: Value = tc
                .arguments
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(json!({}));
            content_blocks.push(json!({
                "type": "tool_use",
                "id": tc.id,
                "name": tc.name,
                "input": input,
            }));
        }

        return json!({
            "role": role,
            "content": content_blocks,
        });
    }

    // Anthropic supports string or array content.
    let content: Value = if msg.content.len() == 1 {
        if matches!(msg.content[0], ContentPart::Text { .. }) {
            if let Some(text) = msg.content[0].as_text() {
                Value::String(text.to_string())
            } else {
                pack_anthropic_content_array(&msg.content)
            }
        } else {
            pack_anthropic_content_array(&msg.content)
        }
    } else if msg.content.is_empty() {
        Value::String(String::new())
    } else {
        pack_anthropic_content_array(&msg.content)
    };

    json!({
        "role": role,
        "content": content,
    })
}

fn pack_anthropic_tool_result_content(parts: &[ContentPart]) -> Value {
    if parts.is_empty() {
        return Value::String(String::new());
    }

    if parts.len() == 1 {
        if let Some(text) = parts[0].as_text() {
            return Value::String(text.to_string());
        }
    }

    pack_anthropic_content_array(parts)
}

fn pack_anthropic_content_array(parts: &[ContentPart]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|p| match p {
                ContentPart::Text { text } => json!({"type": "text", "text": text}),
                ContentPart::ImageUrl { image_url, .. } => json!({
                    "type": "image",
                    "source": {
                        "type": "url",
                        "url": image_url,
                    }
                }),
                ContentPart::Json { value } => json!({"type": "text", "text": value.to_string()}),
                ContentPart::Raw { value } => value.clone(),
            })
            .collect(),
    )
}

fn pack_anthropic_tool(tool: &CanonicalTool) -> Value {
    let mut obj: Value = tool
        .raw
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<String, Value>>()
        .into();

    obj["name"] = json!(tool.name);
    obj["description"] = json!(tool.description);

    if let Some(schema) = &tool.input_schema {
        obj["input_schema"] = schema.clone();
    } else {
        obj["input_schema"] = json!({"type": "object", "properties": {}});
    }

    obj
}

fn pack_anthropic_system(msg: &CanonicalMessage) -> Value {
    if msg.content.len() == 1 {
        if let ContentPart::Text { text } = &msg.content[0] {
            return json!(text);
        }
    }
    pack_anthropic_content_array(&msg.content)
}
