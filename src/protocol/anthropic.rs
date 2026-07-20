// ---------------------------------------------------------------------------
// Anthropic protocol adapter — pack / unpack / builder helpers
//
// Converts between CanonicalRelayRequest/Response and the Anthropic
// /v1/messages wire format.
//
// Also provides an OpenAI SSE → Anthropic SSE stream translator for cases
// where the client expects Anthropic event format but the upstream returns
// OpenAI-compatible chunks.
// ---------------------------------------------------------------------------

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

use crate::error::GatewayError;
use crate::protocol::accio::{
    normalize_tool_args, parse_sse_line as parse_anthropic_sse_line, ParsedEvent,
};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool,
    CanonicalToolCall, ContentPart, EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::sse_parse::{parse_sse_line as parse_sse_frame_line, SseFrame, SseParseState};

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
// normalize_messages
// ---------------------------------------------------------------------------

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
// unpack_anthropic_response
// ---------------------------------------------------------------------------

/// Unpack an Anthropic response JSON body into a [`CanonicalRelayResponse`].
pub fn unpack_anthropic_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Anthropic returns `{ "content": [ { "type": "text", "text": "..." } ] }`.
    let content_arr = body
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::server_error("Anthropic response missing `content` array"))?;

    let mut text_parts: Vec<&str> = Vec::new();
    let mut tool_calls: Vec<CanonicalToolCall> = Vec::new();

    for item in content_arr {
        let block_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(t);
                }
            }
            "tool_use" => {
                let id = item.get("id").and_then(|v| v.as_str()).map(str::to_string);
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let arguments = item.get("input").map(|v| v.to_string());
                tool_calls.push(CanonicalToolCall {
                    id,
                    call_type: "function".to_string(),
                    name,
                    arguments,
                    raw: std::collections::HashMap::new(),
                });
            }
            _ => {}
        }
    }
    let text = text_parts.join("");

    let finish_reason =
        map_anthropic_finish_reason(body.get("stop_reason").and_then(|v| v.as_str()));

    let usage = body.get("usage").map(|u| {
        let prompt = u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        let completion = u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0);
        TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            cache_creation_input_tokens: u
                .get("cache_creation_input_tokens")
                .and_then(|value| value.as_u64()),
            cache_read_input_tokens: u
                .get("cache_read_input_tokens")
                .and_then(|value| value.as_u64()),
        }
    });

    let upstream_status = body
        .get("_upstream_status")
        .and_then(|v| v.as_u64())
        .map(|s| s as u16);

    Ok(CanonicalRelayResponse {
        model,
        text,
        usage,
        tool_calls,
        upstream_status,
        finish_reason,
    })
}

// ---------------------------------------------------------------------------
// Response builders (for the relay to re-emit in Anthropic format)
// ---------------------------------------------------------------------------

/// Build a complete, non-streaming Anthropic messages success response.
pub fn build_messages_success(
    response_id: &str,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        content.push(json!({"type": "text", "text": text}));
    }
    for tool_call in tool_calls {
        let input = tool_call
            .arguments
            .as_deref()
            .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
            .unwrap_or_else(|| json!({}));
        content.push(json!({
            "type": "tool_use",
            "id": tool_call.id,
            "name": tool_call.name,
            "input": input,
        }));
    }

    let mut response = json!({
        "id": response_id,
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": map_anthropic_stop_reason(finish_reason, tool_calls),
        "stop_sequence": null,
    });

    if let Some(u) = usage {
        let mut usage_json = json!({
            "input_tokens": u.prompt_tokens,
            "output_tokens": u.completion_tokens,
        });
        if let Some(cache_creation_input_tokens) = u.cache_creation_input_tokens {
            usage_json["cache_creation_input_tokens"] = json!(cache_creation_input_tokens);
        }
        if let Some(cache_read_input_tokens) = u.cache_read_input_tokens {
            usage_json["cache_read_input_tokens"] = json!(cache_read_input_tokens);
        }
        response["usage"] = usage_json;
    }

    response
}

/// Build a streaming SSE content delta in Anthropic format.
pub fn build_messages_delta(delta_text: &str) -> Value {
    json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {
            "type": "text_delta",
            "text": delta_text,
        }
    })
}

/// Build a streaming stop event in Anthropic format.
pub fn build_messages_stop(usage: Option<&TokenUsage>) -> Value {
    let mut obj = json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": "end_turn",
            "stop_sequence": null,
        },
    });

    if let Some(u) = usage {
        let mut usage_json = json!({
            "input_tokens": u.prompt_tokens,
            "output_tokens": u.completion_tokens,
        });
        if let Some(cache_creation_input_tokens) = u.cache_creation_input_tokens {
            usage_json["cache_creation_input_tokens"] = json!(cache_creation_input_tokens);
        }
        if let Some(cache_read_input_tokens) = u.cache_read_input_tokens {
            usage_json["cache_read_input_tokens"] = json!(cache_read_input_tokens);
        }
        obj["usage"] = usage_json;
    }

    obj
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

fn map_anthropic_finish_reason(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "end_turn" | "stop_sequence" => "stop",
            "tool_use" => "tool_calls",
            "max_tokens" => "length",
            other => other,
        }
        .to_string(),
    )
}

fn map_anthropic_stop_reason(
    finish_reason: Option<&str>,
    tool_calls: &[CanonicalToolCall],
) -> &'static str {
    match finish_reason.unwrap_or(if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    }) {
        "tool_calls" | "function_call" => "tool_use",
        "length" => "max_tokens",
        _ => "end_turn",
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

fn is_plain_text_block(part: &Value) -> bool {
    part.as_object().is_some_and(|map| {
        map.keys()
            .all(|key| matches!(key.as_str(), "type" | "text"))
    })
}

// ---------------------------------------------------------------------------
// OpenAI SSE → Anthropic SSE stream translation
// ---------------------------------------------------------------------------

/// Parse a single OpenAI SSE line and convert to Anthropic SSE event bytes.
///
/// Returns `None` for non-content lines (empty, comments, event: lines).
pub async fn accumulate_anthropic_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = response.text().await.map_err(|e| {
        GatewayError::server_error(format!("failed to read Anthropic SSE body: {e}"))
    })?;
    let mut text = String::new();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut cache_creation_input_tokens = None;
    let mut cache_read_input_tokens = None;
    let mut finish_reason = None;
    let mut reported_model = model.to_string();
    let mut tool_calls = Vec::new();
    let mut pending_tools: BTreeMap<i64, AnthropicPendingToolCall> = BTreeMap::new();

    for line in body.lines() {
        let Some(events) = parse_anthropic_sse_line(line.as_bytes()) else {
            continue;
        };
        for event in events {
            match event {
                ParsedEvent::ProviderError { code, message } => {
                    return Err(GatewayError::server_error(format!(
                        "Anthropic upstream stream error {code}: {message}"
                    )));
                }
                ParsedEvent::Start { model, usage } => {
                    if let Some(model) = model {
                        reported_model = model;
                    }
                    if let Some(usage) = usage {
                        prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                        completion_tokens = completion_tokens.max(usage.completion_tokens);
                        cache_creation_input_tokens = usage
                            .cache_creation_input_tokens
                            .or(cache_creation_input_tokens);
                        cache_read_input_tokens =
                            usage.cache_read_input_tokens.or(cache_read_input_tokens);
                    }
                }
                ParsedEvent::Text(delta) => text.push_str(&delta),
                ParsedEvent::ToolStart { index, id, name } => {
                    pending_tools.insert(
                        index,
                        AnthropicPendingToolCall {
                            id,
                            name,
                            arguments: String::new(),
                        },
                    );
                }
                ParsedEvent::ToolDelta { index, partial } => {
                    if let Some(call) = pending_tools.get_mut(&index) {
                        call.arguments.push_str(&partial);
                    }
                }
                ParsedEvent::ToolEnd { index } => {
                    if let Some(call) = pending_tools.remove(&index) {
                        tool_calls.push(CanonicalToolCall {
                            id: Some(call.id),
                            call_type: "function".into(),
                            name: Some(call.name),
                            arguments: Some(normalize_tool_args(&call.arguments)),
                            raw: std::collections::HashMap::new(),
                        });
                    }
                }
                ParsedEvent::ToolCall {
                    id,
                    name,
                    arguments,
                } => tool_calls.push(CanonicalToolCall {
                    id: Some(id),
                    call_type: "function".into(),
                    name: Some(name),
                    arguments: Some(normalize_tool_args(&arguments)),
                    raw: std::collections::HashMap::new(),
                }),
                ParsedEvent::Finish { reason, usage } => {
                    if let Some(reason) = reason {
                        finish_reason = Some(reason);
                    }
                    if let Some(usage) = usage {
                        prompt_tokens = prompt_tokens.max(usage.prompt_tokens);
                        completion_tokens = completion_tokens.max(usage.completion_tokens);
                        cache_creation_input_tokens = usage
                            .cache_creation_input_tokens
                            .or(cache_creation_input_tokens);
                        cache_read_input_tokens =
                            usage.cache_read_input_tokens.or(cache_read_input_tokens);
                    }
                }
                ParsedEvent::Done => {}
            }
        }
    }

    for (_, call) in pending_tools {
        tool_calls.push(CanonicalToolCall {
            id: Some(call.id),
            call_type: "function".into(),
            name: Some(call.name),
            arguments: Some(normalize_tool_args(&call.arguments)),
            raw: std::collections::HashMap::new(),
        });
    }

    let total_tokens = prompt_tokens.saturating_add(completion_tokens);
    let usage = if total_tokens > 0
        || cache_creation_input_tokens.is_some()
        || cache_read_input_tokens.is_some()
    {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        })
    } else {
        None
    };

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(finish_reason.unwrap_or_else(|| "stop".into())),
    })
}

fn build_anthropic_message_start_event(
    model: &str,
    response_id: &str,
    usage: Option<&TokenUsage>,
) -> String {
    let prompt_tokens = usage.map(|entry| entry.prompt_tokens).unwrap_or(0);
    let data = json!({
        "type": "message_start",
        "message": {
            "id": response_id,
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": prompt_tokens,
                "output_tokens": 0
            }
        }
    });
    format!("event: message_start\ndata: {}\n\n", data)
}

fn build_anthropic_text_block_start_event(index: usize) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "text",
            "text": ""
        }
    });
    format!("event: content_block_start\ndata: {}\n\n", data)
}

fn build_anthropic_tool_block_start_event(index: usize, id: &str, name: &str) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": {}
        }
    });
    format!("event: content_block_start\ndata: {}\n\n", data)
}

fn build_anthropic_text_block_delta_event(index: usize, text: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {
            "type": "text_delta",
            "text": text
        }
    });
    format!("event: content_block_delta\ndata: {}\n\n", data)
}

fn build_anthropic_tool_block_delta_event(index: usize, partial_json: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {
            "type": "input_json_delta",
            "partial_json": partial_json
        }
    });
    format!("event: content_block_delta\ndata: {}\n\n", data)
}

fn build_anthropic_content_block_stop(index: usize) -> String {
    let data = json!({
        "type": "content_block_stop",
        "index": index
    });
    format!("event: content_block_stop\ndata: {}\n\n", data)
}

fn build_anthropic_message_delta(stop_reason: &str, usage: Option<&TokenUsage>) -> String {
    let mut data = json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": stop_reason,
            "stop_sequence": null
        }
    });

    if let Some(u) = usage {
        data["usage"] = json!({
            "input_tokens": u.prompt_tokens,
            "output_tokens": u.completion_tokens
        });
    } else {
        data["usage"] = json!({
            "input_tokens": 0,
            "output_tokens": 0
        });
    }

    format!("event: message_delta\ndata: {}\n\n", data)
}

fn build_anthropic_message_stop_event() -> String {
    let data = json!({
        "type": "message_stop"
    });
    format!("event: message_stop\ndata: {}\n\n", data)
}

/// Wrap an OpenAI-format SSE byte stream and translate each event to Anthropic
/// SSE event format on-the-fly.
///
/// The translator implements a state machine:
/// 1. On the first content chunk: emit `message_start` + `content_block_start`
/// 2. For each content delta: emit `content_block_delta`
/// 3. On `finish_reason` or `[DONE]`: emit `content_block_stop` +
///    `message_delta` + `message_stop`
pub fn translate_openai_sse_to_anthropic(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("msg_{}", uuid::Uuid::new_v4().as_simple());

    let state = OpenAiToAnthropicState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        model,
        response_id,
        outputs: VecDeque::new(),
        message_started: false,
        open_text_index: None,
        next_block_index: 0,
        tool_blocks: BTreeMap::new(),
        pending_finish_reason: None,
        latest_usage: None,
        final_emitted: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, mut done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(output) = st.outputs.pop_front() {
                    done = st.final_emitted && st.outputs.is_empty();
                    return Some((Ok(Bytes::from(output)), (stream, st, done)));
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.buffer.extend_from_slice(&chunk);
                        st.drain_frames();
                    }
                    Some(Err(e)) => {
                        return Some((Err(e), (stream, st, true)));
                    }
                    None => {
                        st.drain_frames();
                        st.flush_pending_frame();
                        st.emit_final_if_needed(None);
                        if let Some(output) = st.outputs.pop_front() {
                            done = st.final_emitted && st.outputs.is_empty();
                            return Some((Ok(Bytes::from(output)), (stream, st, done)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

/// Internal state for the OpenAI-to-Anthropic stream translator.
struct OpenAiToAnthropicState {
    buffer: Vec<u8>,
    parser: SseParseState,
    model: String,
    response_id: String,
    outputs: VecDeque<Vec<u8>>,
    message_started: bool,
    open_text_index: Option<usize>,
    next_block_index: usize,
    tool_blocks: BTreeMap<usize, PendingAnthropicToolBlock>,
    pending_finish_reason: Option<String>,
    latest_usage: Option<TokenUsage>,
    final_emitted: bool,
}

#[derive(Debug, Clone)]
struct AnthropicPendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Clone)]
struct PendingAnthropicToolBlock {
    block_index: usize,
    id: String,
    name: Option<String>,
    arguments: String,
    announced: bool,
    closed: bool,
}

impl OpenAiToAnthropicState {
    fn drain_frames(&mut self) {
        while let Some(pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=pos).collect::<Vec<_>>();
            if matches!(line.last(), Some(b'\n')) {
                line.pop();
            }
            if matches!(line.last(), Some(b'\r')) {
                line.pop();
            }
            let Ok(line) = std::str::from_utf8(&line) else {
                continue;
            };
            if let Some(frame) = parse_sse_frame_line(line, &mut self.parser) {
                self.handle_frame(frame);
            }
        }
    }

    fn flush_pending_frame(&mut self) {
        if let Some(frame) = parse_sse_frame_line("", &mut self.parser) {
            self.handle_frame(frame);
        }
    }

    fn handle_frame(&mut self, frame: SseFrame) {
        if frame.data.is_empty() {
            return;
        }
        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return;
        }

        let Ok(chunk) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };
        if let Some(usage) = extract_openai_stream_usage(&chunk) {
            self.latest_usage = Some(usage);
        }

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            self.handle_choice(choice);
        }

        if self.pending_finish_reason.is_some() && choice.is_none() && self.latest_usage.is_some() {
            self.emit_final_if_needed(None);
        }
    }

    fn handle_choice(&mut self, choice: &Value) {
        let delta = choice.get("delta");
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .and_then(map_openai_finish_reason);

        if let Some(tool_calls) = delta
            .and_then(|entry| entry.get("tool_calls"))
            .and_then(|entry| entry.as_array())
        {
            self.close_text_block();
            self.ensure_message_started();
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                self.handle_tool_call_delta(tool_call, fallback_index);
            }
        }

        if let Some(text) = delta
            .and_then(|entry| entry.get("content"))
            .and_then(|entry| entry.as_str())
        {
            if !text.is_empty() {
                self.close_tool_blocks();
                self.ensure_message_started();
                let index = self.open_or_create_text_block();
                self.outputs
                    .push_back(build_anthropic_text_block_delta_event(index, text).into_bytes());
            }
        }

        if let Some(reason) = finish_reason {
            self.pending_finish_reason = Some(reason);
            if self.latest_usage.is_some() {
                self.emit_final_if_needed(None);
            }
        }
    }

    fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        let openai_index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);
        let mut start_event = None;
        let mut delta_event = None;
        let next_block_index = &mut self.next_block_index;
        {
            let entry =
                self.tool_blocks
                    .entry(openai_index)
                    .or_insert_with(|| PendingAnthropicToolBlock {
                        block_index: {
                            let index = *next_block_index;
                            *next_block_index += 1;
                            index
                        },
                        id: format!("toolu_{}", uuid::Uuid::new_v4().as_simple()),
                        name: None,
                        arguments: String::new(),
                        announced: false,
                        closed: false,
                    });

            if let Some(id) = tool_call.get("id").and_then(|value| value.as_str()) {
                if !id.trim().is_empty() {
                    entry.id = id.to_string();
                }
            }
            if let Some(name) = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(|value| value.as_str())
            {
                if !name.trim().is_empty() {
                    entry.name = Some(name.to_string());
                }
            }
            if !entry.announced {
                let tool_name = entry
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("tool_{}", openai_index));
                start_event = Some(build_anthropic_tool_block_start_event(
                    entry.block_index,
                    &entry.id,
                    &tool_name,
                ));
                entry.name = Some(tool_name);
                entry.announced = true;
            }
            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(|value| value.as_str())
            {
                if !arguments.is_empty() {
                    entry.arguments.push_str(arguments);
                    delta_event = Some(build_anthropic_tool_block_delta_event(
                        entry.block_index,
                        arguments,
                    ));
                }
            }
        }
        if let Some(event) = start_event {
            self.outputs.push_back(event.into_bytes());
        }
        if let Some(event) = delta_event {
            self.outputs.push_back(event.into_bytes());
        }
    }

    fn ensure_message_started(&mut self) {
        if self.message_started {
            return;
        }
        self.message_started = true;
        self.outputs.push_back(
            build_anthropic_message_start_event(
                &self.model,
                &self.response_id,
                self.latest_usage.as_ref(),
            )
            .into_bytes(),
        );
    }

    fn open_or_create_text_block(&mut self) -> usize {
        if let Some(index) = self.open_text_index {
            return index;
        }
        let index = self.next_block_index;
        self.next_block_index += 1;
        self.open_text_index = Some(index);
        self.outputs
            .push_back(build_anthropic_text_block_start_event(index).into_bytes());
        index
    }

    fn close_text_block(&mut self) {
        if let Some(index) = self.open_text_index.take() {
            self.outputs
                .push_back(build_anthropic_content_block_stop(index).into_bytes());
        }
    }

    fn close_tool_blocks(&mut self) {
        let mut tool_indexes: Vec<usize> = self.tool_blocks.keys().copied().collect();
        tool_indexes.sort_unstable();
        for openai_index in tool_indexes {
            let mut start_event = None;
            let mut stop_event = None;
            if let Some(entry) = self.tool_blocks.get_mut(&openai_index) {
                if !entry.announced {
                    let tool_name = entry
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("tool_{}", openai_index));
                    start_event = Some(build_anthropic_tool_block_start_event(
                        entry.block_index,
                        &entry.id,
                        &tool_name,
                    ));
                    entry.name = Some(tool_name);
                    entry.announced = true;
                }
                if !entry.closed {
                    stop_event = Some(build_anthropic_content_block_stop(entry.block_index));
                    entry.closed = true;
                }
            }
            if let Some(event) = start_event {
                self.outputs.push_back(event.into_bytes());
            }
            if let Some(event) = stop_event {
                self.outputs.push_back(event.into_bytes());
            }
        }
    }

    fn emit_final_if_needed(&mut self, fallback_reason: Option<&str>) {
        if self.final_emitted {
            return;
        }
        self.ensure_message_started();
        self.close_text_block();
        self.close_tool_blocks();
        let reason = self
            .pending_finish_reason
            .clone()
            .or_else(|| fallback_reason.map(str::to_string))
            .unwrap_or_else(|| "end_turn".to_string());
        self.outputs.push_back(
            build_anthropic_message_delta(&reason, self.latest_usage.as_ref()).into_bytes(),
        );
        self.outputs
            .push_back(build_anthropic_message_stop_event().into_bytes());
        self.final_emitted = true;
    }
}

fn extract_openai_stream_usage(chunk: &Value) -> Option<TokenUsage> {
    let usage = chunk.get("usage")?;
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|value| value.as_u64())
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|value| value.as_u64()),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|value| value.as_u64())
            .or_else(|| {
                usage
                    .get("prompt_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            })
            .or_else(|| {
                usage
                    .get("input_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            }),
    })
}

fn map_openai_finish_reason(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" => "tool_use",
            other => other,
        }
        .to_string(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── normalize_messages ────────────────────────────────────────────────

    #[test]
    fn normalize_basic_anthropic_request() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "Be helpful.",
            "messages": [{"role": "user", "content": "Hello!"}],
            "max_tokens": 1024,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(
            req.requested_model.as_deref(),
            Some("claude-3-5-sonnet-20241022")
        );
        // First message should be the extracted system prompt.
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert_eq!(req.messages[1].role, MessageRole::User);
    }

    #[test]
    fn normalize_no_system_field() {
        let body = json!({
            "model": "claude-3-opus-20240229",
            "messages": [{"role": "user", "content": "Hi"}],
            "max_tokens": 512,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::User);
    }

    #[test]
    fn normalize_captures_max_tokens_in_extra() {
        let body = json!({
            "model": "claude-3-haiku-20240307",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 2048,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.extra.get("max_tokens"), Some(&json!(2048)));
    }

    // ── pack_anthropic ────────────────────────────────────────────────────

    #[test]
    fn pack_extracts_system_to_top_level() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "You are a bot.",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1024,
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(packed["system"], json!("You are a bot."));
        let msgs = packed["messages"].as_array().unwrap();
        assert!(msgs.iter().all(|m| m["role"] != "system"));
    }

    #[test]
    fn pack_defaults_max_tokens_to_4096() {
        let body = json!({
            "model": "claude-3-haiku-20240307",
            "messages": [{"role": "user", "content": "hi"}],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-haiku-20240307", false);
        assert_eq!(packed["max_tokens"], json!(4096));
    }

    #[test]
    fn pack_preserves_existing_system_cache_control_from_anthropic_raw_body() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": [{
                "type": "text",
                "text": "You are a bot.",
                "cache_control": {"type": "ephemeral"}
            }],
            "messages": [{"role": "user", "content": "hi"}],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(
            packed["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    #[test]
    fn pack_preserves_existing_tool_cache_control_from_anthropic_raw_body() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "name": "search",
                "description": "Search",
                "input_schema": {"type": "object"},
                "cache_control": {"type": "ephemeral"}
            }],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(
            packed["tools"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    #[test]
    fn pack_auto_applies_cache_control_for_claude_when_missing() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "You are a bot.",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "name": "search",
                "description": "Search",
                "input_schema": {"type": "object"}
            }],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(packed["cache_control"], json!({"type": "ephemeral"}));
    }

    #[test]
    fn pack_reports_client_supplied_cache_markers_in_telemetry() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": [{
                "type": "text",
                "text": "You are a bot.",
                "cache_control": {"type": "ephemeral"}
            }],
            "messages": [{"role": "user", "content": "hi"}],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
        assert!(packed.prompt_cache_telemetry.client_has_cache_control);
        assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
    }

    #[test]
    fn pack_reports_auto_applied_cache_markers_in_telemetry() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "system": "You are a bot.",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "name": "search",
                "description": "Search",
                "input_schema": {"type": "object"}
            }],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
        assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
        assert!(packed.prompt_cache_telemetry.auto_cache_applied);
    }

    #[test]
    fn pack_does_not_auto_apply_cache_control_for_non_claude_models() {
        let body = json!({
            "model": "gpt-4o",
            "system": "You are a bot.",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "name": "search",
                "description": "Search",
                "input_schema": {"type": "object"}
            }],
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "gpt-4o", false);
        assert!(packed["system"].is_string());
        assert!(packed.get("cache_control").is_none());
    }

    #[test]
    fn pack_preserves_top_level_cache_control_from_openai_passthrough_extra() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "cache_control": {"type": "ephemeral"}
        });
        let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(packed["cache_control"], json!({"type": "ephemeral"}));
    }

    #[test]
    fn pack_respects_gateway_auto_cache_opt_out() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "gateway_auto_cache": false
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic_with_telemetry(&req, "claude-3-5-sonnet-20241022", false);
        assert!(packed.body.get("cache_control").is_none());
        assert!(!packed.prompt_cache_telemetry.client_has_cache_control);
        assert!(!packed.prompt_cache_telemetry.auto_cache_applied);
    }

    #[test]
    fn normalize_and_repack_preserves_message_block_cache_control() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hi",
                    "cache_control": {"type": "ephemeral"}
                }]
            }]
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(
            packed["messages"][0]["content"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    #[test]
    fn openai_system_text_block_cache_control_survives_anthropic_pack() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [
                {
                    "role": "system",
                    "content": [
                        {
                            "type": "text",
                            "text": "system prefix",
                            "cache_control": { "type": "ephemeral" }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": "hi"
                }
            ]
        });
        let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(
            packed["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    // ── unpack_anthropic_response ─────────────────────────────────────────

    #[test]
    fn unpack_standard_anthropic_response() {
        let body = json!({
            "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{"type": "text", "text": "Hello! How can I help?"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 12,
                "output_tokens": 8,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 40
            }
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.text, "Hello! How can I help?");
        assert_eq!(resp.model, "claude-3-5-sonnet-20241022");
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 12);
        assert_eq!(usage.completion_tokens, 8);
        assert_eq!(usage.total_tokens, 20);
        assert_eq!(usage.cache_creation_input_tokens, Some(100));
        assert_eq!(usage.cache_read_input_tokens, Some(40));
    }

    #[test]
    fn unpack_missing_content_returns_error() {
        let body = json!({"model": "claude-3-haiku-20240307"});
        assert!(unpack_anthropic_response(&body).is_err());
    }

    // ── builders ──────────────────────────────────────────────────────────

    #[test]
    fn build_messages_success_structure() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cache_creation_input_tokens: Some(120),
            cache_read_input_tokens: Some(80),
        };
        let resp = build_messages_success(
            "msg_abc",
            "claude-3-5-sonnet-20241022",
            "Hi!",
            Some(&usage),
            &[],
            None,
        );
        assert_eq!(resp["type"], "message");
        assert_eq!(resp["content"][0]["text"], "Hi!");
        assert_eq!(resp["usage"]["input_tokens"], 10);
        assert_eq!(resp["usage"]["output_tokens"], 20);
        assert_eq!(resp["usage"]["cache_creation_input_tokens"], 120);
        assert_eq!(resp["usage"]["cache_read_input_tokens"], 80);
    }

    #[test]
    fn build_messages_delta_structure() {
        let delta = build_messages_delta("Hello");
        assert_eq!(delta["type"], "content_block_delta");
        assert_eq!(delta["delta"]["type"], "text_delta");
        assert_eq!(delta["delta"]["text"], "Hello");
    }

    #[test]
    fn build_messages_stop_structure() {
        let stop = build_messages_stop(None);
        assert_eq!(stop["type"], "message_delta");
        assert_eq!(stop["delta"]["stop_reason"], "end_turn");
    }

    // ── tool_use parsing ─────────────────────────────────────────────────

    #[test]
    fn unpack_response_parses_tool_use_blocks() {
        let body = json!({
            "id": "msg_tc",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [
                {"type": "text", "text": "Let me check the weather."},
                {
                    "type": "tool_use",
                    "id": "toolu_abc123",
                    "name": "get_weather",
                    "input": {"location": "NYC", "unit": "celsius"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 20}
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.text, "Let me check the weather.");
        assert_eq!(resp.tool_calls.len(), 1);
        assert_eq!(resp.tool_calls[0].id.as_deref(), Some("toolu_abc123"));
        assert_eq!(resp.tool_calls[0].name.as_deref(), Some("get_weather"));
        assert_eq!(resp.tool_calls[0].call_type, "function");
        // arguments should be JSON-encoded string of the input object
        let args: serde_json::Value =
            serde_json::from_str(resp.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
        assert_eq!(args["location"], "NYC");
        assert_eq!(resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn unpack_response_multiple_tool_use_blocks() {
        let body = json!({
            "id": "msg_multi",
            "model": "claude-3-5-sonnet-20241022",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "search",
                    "input": {"q": "rust"}
                },
                {
                    "type": "tool_use",
                    "id": "toolu_2",
                    "name": "calculator",
                    "input": {"expr": "1+1"}
                }
            ],
            "stop_reason": "tool_use"
        });
        let resp = unpack_anthropic_response(&body).unwrap();
        assert_eq!(resp.tool_calls.len(), 2);
        assert_eq!(resp.tool_calls[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(resp.tool_calls[1].id.as_deref(), Some("toolu_2"));
    }

    #[test]
    fn normalize_assistant_tool_use_into_canonical_tool_calls() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Checking"},
                    {"type": "tool_use", "id": "toolu_1", "name": "lookup", "input": {"city": "Tokyo"}}
                ]
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::Assistant);
        assert_eq!(req.messages[0].text_content(), "Checking");
        assert_eq!(req.messages[0].tool_calls.len(), 1);
        assert_eq!(req.messages[0].tool_calls[0].id.as_deref(), Some("toolu_1"));
    }

    #[test]
    fn normalize_user_tool_result_into_tool_message() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "Here is the tool result"},
                    {"type": "tool_result", "tool_use_id": "toolu_1", "content": [{"type": "text", "text": "{\"ok\":true}"}]}
                ]
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, MessageRole::User);
        assert_eq!(req.messages[1].role, MessageRole::Tool);
        assert_eq!(req.messages[1].tool_call_id.as_deref(), Some("toolu_1"));
        assert_eq!(req.messages[1].text_content(), "{\"ok\":true}");
    }

    #[test]
    fn pack_tool_result_preserves_json_content() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::GeminiGenerateContent,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("models/gemini-2.5-pro".to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Use the weather tool.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::Tool,
                    content: vec![ContentPart::Json {
                        value: json!({"city": "Hangzhou", "condition": "sunny"}),
                    }],
                    name: Some("weather".to_string()),
                    tool_call_id: Some("toolu_weather".to_string()),
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let packed = pack_anthropic(&req, "claude-3-5-sonnet", false);
        assert_eq!(
            packed["messages"][1]["content"][0]["type"],
            json!("tool_result")
        );
        assert_eq!(
            packed["messages"][1]["content"][0]["content"],
            json!([{
                "type": "text",
                "text": "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}"
            }])
        );
    }

    #[test]
    fn normalize_tool_choice_any_to_required() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "tool_choice": { "type": "any" },
            "messages": [{
                "role": "user",
                "content": "hi"
            }]
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(req.tool_choice, Some(json!("required")));
    }

    #[test]
    fn pack_openai_required_tool_choice_to_anthropic_any() {
        let body = json!({
            "model": "gpt-4o",
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "weather",
                    "parameters": {"type":"object"}
                }
            }],
            "tool_choice": "required"
        });
        let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(packed["tool_choice"], json!({"type": "any"}));
    }

    #[test]
    fn pack_openai_responses_specific_tool_choice_to_anthropic_tool() {
        let body = json!({
            "model": "gpt-4o",
            "input": "Use only the weather tool.",
            "tools": [{
                "type": "function",
                "name": "weather",
                "parameters": {"type":"object"}
            }],
            "tool_choice": {
                "type": "function",
                "name": "weather"
            }
        });
        let req = crate::protocol::responses::normalize_responses(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(
            packed["tool_choice"],
            json!({"type": "tool", "name": "weather"})
        );
    }

    // ── unknown field preservation ───────────────────────────────────────

    #[test]
    fn normalize_captures_unknown_fields_in_extra() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1024,
            "context_management": {"enabled": true},
            "metadata": {"user_id": "u-123"},
            "custom_vendor_field": 42,
        });
        let req = normalize_messages(body).unwrap();
        assert_eq!(
            req.extra.get("context_management"),
            Some(&json!({"enabled": true}))
        );
        assert_eq!(
            req.extra.get("metadata"),
            Some(&json!({"user_id": "u-123"}))
        );
        assert_eq!(req.extra.get("custom_vendor_field"), Some(&json!(42)));
        assert_eq!(req.extra.get("max_tokens"), Some(&json!(1024)));
    }

    #[test]
    fn pack_merges_extra_fields_back_into_body() {
        let body = json!({
            "model": "claude-3-5-sonnet-20241022",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1024,
            "temperature": 0.5,
            "context_management": {"enabled": true},
        });
        let req = normalize_messages(body).unwrap();
        let packed = pack_anthropic(&req, "claude-3-5-sonnet-20241022", false);
        assert_eq!(packed["temperature"], json!(0.5));
        assert_eq!(packed["context_management"], json!({"enabled": true}));
    }

    // ── OpenAI SSE → Anthropic SSE translation ─────────────────────────

    #[tokio::test]
    async fn translate_openai_stream_produces_anthropic_events() {
        use futures::StreamExt;

        let events = vec![
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> = events
            .into_iter()
            .map(|s| Ok(Bytes::from(s.to_string())))
            .collect();

        let inner_stream = futures::stream::iter(chunks);
        let mut translated = Box::pin(translate_openai_sse_to_anthropic(
            inner_stream,
            "gpt-4o".to_string(),
        ));

        let mut collected: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        // Verify we have output events
        assert!(!collected.is_empty(), "expected some output events");

        let full_output = collected.join("");

        // Must contain all Anthropic event types
        assert!(
            full_output.contains("event: message_start"),
            "missing message_start"
        );
        assert!(
            full_output.contains("event: content_block_delta"),
            "missing content_block_delta"
        );
        assert!(
            full_output.contains("event: content_block_stop"),
            "missing content_block_stop"
        );
        assert!(
            full_output.contains("event: message_delta"),
            "missing message_delta"
        );
        assert!(
            full_output.contains("event: message_stop"),
            "missing message_stop"
        );

        // Verify the delta contains the text
        assert!(
            full_output.contains("\"text\":\"Hi\""),
            "missing text delta 'Hi'"
        );
    }

    #[tokio::test]
    async fn translate_openai_tool_stream_produces_tool_use_events() {
        use futures::StreamExt;

        let events = vec![
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"weather\",\"arguments\":\"{\\\"city\\\":\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Hangzhou\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"created\":1700000000,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":8,\"completion_tokens\":5,\"total_tokens\":13}}\n\n",
            "data: [DONE]\n\n",
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> = events
            .into_iter()
            .map(|entry| Ok(Bytes::from(entry.to_string())))
            .collect();

        let inner_stream = futures::stream::iter(chunks);
        let mut translated = Box::pin(translate_openai_sse_to_anthropic(
            inner_stream,
            "gpt-4o".to_string(),
        ));

        let mut collected: Vec<String> = Vec::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        let full_output = collected.join("");
        assert!(full_output.contains("event: message_start"));
        assert!(full_output.contains("\"type\":\"tool_use\""));
        assert!(full_output.contains("\"name\":\"weather\""));
        assert!(full_output.contains("\"type\":\"input_json_delta\""));
        assert!(full_output.contains("{\\\"city\\\":"));
        assert!(full_output.contains("\\\"Hangzhou\\\"}"));
        assert!(full_output.contains("\"stop_reason\":\"tool_use\""));
        assert!(full_output.contains("\"output_tokens\":5"));
        assert!(full_output.contains("event: message_stop"));
    }
}
