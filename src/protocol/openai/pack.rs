use serde_json::{json, Value};

use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalTool, ContentPart, EndpointKind, MessageRole,
    ProtocolFamily,
};

/// Pack a [`CanonicalRelayRequest`] into an OpenAI chat/completions JSON body.
pub fn pack_openai(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let is_reasoning = model.starts_with("o1") || model.starts_with("o3");

    // Build messages array — extract system into first element if present.
    let mut json_messages: Vec<Value> = Vec::new();

    // If there is a system message, output it as the first message in the array.
    if let Some(sys_text) = req.system_message() {
        json_messages.push(json!({
            "role": "system",
            "content": sys_text,
        }));
    }

    for msg in &req.messages {
        if msg.role == MessageRole::System {
            // Already handled above.
            continue;
        }
        json_messages.push(pack_openai_message(msg));
    }

    let mut body = json!({
        "model": model,
        "messages": json_messages,
        "stream": stream,
    });

    // ── tools ──────────────────────────────────────────────────────────────

    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(pack_openai_tool).collect::<Vec<_>>());
    }
    if let Some(tc) = pack_openai_tool_choice(req) {
        body["tool_choice"] = tc;
    }

    // ── extra parameters ───────────────────────────────────────────────────

    for (k, v) in &req.extra {
        match k.as_str() {
            // max_tokens — routing depends on model family
            "max_tokens" if is_reasoning => {
                // For reasoning models, translate to max_completion_tokens
                body["max_completion_tokens"] = v.clone();
            }
            "max_tokens" => {
                body["max_tokens"] = v.clone();
            }
            "max_output_tokens" => {}
            // Reasoning models should not receive temperature/top_p.
            "temperature" | "top_p" if is_reasoning => {}
            _ => {
                body[k] = v.clone();
            }
        }
    }

    // Responses uses a different output-limit field than Chat Completions.
    if !req.extra.contains_key("max_tokens") {
        if let Some(limit) = req.extra.get("max_output_tokens") {
            let key = if is_reasoning {
                "max_completion_tokens"
            } else {
                "max_tokens"
            };
            body[key] = limit.clone();
        }
    }

    // If the request explicitly carried max_completion_tokens (from a
    // reasoning model request), always forward it verbatim.
    if let Some(mct) = req.extra.get("max_completion_tokens") {
        body["max_completion_tokens"] = mct.clone();
    }

    if stream {
        ensure_stream_usage_requested(&mut body);
    }

    body
}

fn pack_openai_tool_choice(req: &CanonicalRelayRequest) -> Option<Value> {
    let tool_choice = req.tool_choice.as_ref()?;
    if req.endpoint_kind == EndpointKind::Responses
        && tool_choice.get("type").and_then(Value::as_str) == Some("function")
    {
        if let Some(name) = tool_choice.get("name").and_then(Value::as_str) {
            return Some(json!({"type": "function", "function": {"name": name}}));
        }
    }
    if req.protocol_family == ProtocolFamily::OpenAi {
        return Some(tool_choice.clone());
    }

    let Some(object) = tool_choice.as_object() else {
        return Some(tool_choice.clone());
    };

    let choice_type = object.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match choice_type {
        "auto" => Some(json!("auto")),
        "none" => Some(json!("none")),
        "any" => Some(json!("required")),
        "tool" => object
            .get("name")
            .and_then(|v| v.as_str())
            .map(|name| json!({"type": "function", "function": {"name": name}})),
        _ => Some(tool_choice.clone()),
    }
}

fn ensure_stream_usage_requested(body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };

    match object.get_mut("stream_options") {
        Some(Value::Object(options)) => {
            options
                .entry("include_usage".to_string())
                .or_insert_with(|| json!(true));
        }
        Some(_) => {}
        None => {
            object.insert(
                "stream_options".to_string(),
                json!({ "include_usage": true }),
            );
        }
    }
}

pub(super) fn pack_openai_message(msg: &CanonicalMessage) -> Value {
    let role = match msg.role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    };

    let content: Value = if msg.role == MessageRole::Tool {
        Value::String(render_openai_tool_message_content(msg))
    } else if msg.content.len() == 1 {
        if let Some(text) = msg.content[0].as_text() {
            Value::String(text.to_string())
        } else {
            pack_content_parts_array(&msg.content)
        }
    } else if msg.content.is_empty() {
        Value::String(String::new())
    } else {
        pack_content_parts_array(&msg.content)
    };

    let mut obj = json!({
        "role": role,
        "content": content,
    });

    if let Some(name) = &msg.name {
        obj["name"] = json!(name);
    }

    if let Some(tcid) = &msg.tool_call_id {
        obj["tool_call_id"] = json!(tcid);
    }

    if !msg.tool_calls.is_empty() {
        obj["tool_calls"] = json!(msg
            .tool_calls
            .iter()
            .map(|tc| json!({
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments,
                }
            }))
            .collect::<Vec<_>>());
    }

    obj
}

fn render_openai_tool_message_content(msg: &CanonicalMessage) -> String {
    if msg.content.is_empty() {
        return String::new();
    }

    let mut fragments = Vec::new();
    for part in &msg.content {
        match part {
            ContentPart::Text { text } => fragments.push(text.clone()),
            ContentPart::Json { value } | ContentPart::Raw { value } => fragments.push(
                value
                    .get("text")
                    .and_then(|entry| entry.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string()),
            ),
            ContentPart::ImageUrl { image_url, .. } => {
                fragments.push(format!("[image omitted: {image_url}]"));
            }
        }
    }
    fragments.join("\n")
}

fn pack_content_parts_array(parts: &[ContentPart]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|p| match p {
                ContentPart::Text { text } => json!({"type": "text", "text": text}),
                ContentPart::ImageUrl { image_url, detail } => {
                    if let Some(d) = detail {
                        json!({"type": "image_url", "image_url": {"url": image_url, "detail": d}})
                    } else {
                        json!({"type": "image_url", "image_url": {"url": image_url}})
                    }
                }
                ContentPart::Json { value } => json!({"type": "json", "value": value}),
                ContentPart::Raw { value } => value.clone(),
            })
            .collect(),
    )
}

fn pack_openai_tool(tool: &CanonicalTool) -> Value {
    let mut fn_obj = json!({});
    if let Some(name) = &tool.name {
        fn_obj["name"] = json!(name);
    }
    if let Some(desc) = &tool.description {
        fn_obj["description"] = json!(desc);
    }
    if let Some(schema) = &tool.input_schema {
        fn_obj["parameters"] = schema.clone();
    }

    json!({
        "type": tool.tool_type,
        "function": fn_obj,
    })
}
