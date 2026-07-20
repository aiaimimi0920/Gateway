// ---------------------------------------------------------------------------
// OpenAI Responses API adapter — pack / unpack / builder helpers
//
// Handles the simpler `POST /v1/responses` format.
// ---------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool,
    CanonicalToolCall, ContentPart, EndpointKind, MessageRole, ProtocolFamily, TokenUsage,
};
use crate::protocol::openai;
use crate::protocol::sse_parse::{
    format_sse_event, parse_sse_line as parse_sse_frame_line, SseFrame, SseParseState,
};
use crate::protocol::tool_choice::{self, CanonicalToolChoice};
use crate::protocol::tool_inject;

// ---------------------------------------------------------------------------
// normalize_responses
// ---------------------------------------------------------------------------

/// Normalize an OpenAI Responses API request body into a
/// [`CanonicalRelayRequest`].
pub fn normalize_responses(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let previous_response_id = body
        .get("previous_response_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // ── instructions → system message ────────────────────────────────────

    let mut messages: Vec<CanonicalMessage> = Vec::new();

    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            messages.push(CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: instructions.to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
    }

    // ── input — can be a string or a messages array ────────────────────────

    match body.get("input") {
        Some(Value::String(s)) => {
            messages.push(CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text { text: s.clone() }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
        Some(Value::Array(arr)) => {
            for item in arr {
                messages.extend(normalize_responses_input_item(item)?);
            }
        }
        Some(other) => {
            // Unknown input format — wrap as raw.
            messages.push(CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Raw {
                    value: other.clone(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            });
        }
        None => {
            return Err(GatewayError::bad_request(
                "Responses API request missing `input` field",
            ));
        }
    }

    // ── extra ─────────────────────────────────────────────────────────────

    let mut extra = std::collections::HashMap::new();
    for key in &["temperature", "max_output_tokens", "top_p"] {
        if let Some(v) = body.get(*key) {
            extra.insert(key.to_string(), v.clone());
        }
    }

    let explicit_session_key = body
        .get("user")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::Responses,
        requested_model: model,
        stream,
        messages,
        tools: parse_responses_tools(body.get("tools")),
        tool_choice: body.get("tool_choice").cloned(),
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id,
        explicit_session_key,
        extra,
    })
}

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

// ---------------------------------------------------------------------------
// Response builders
// ---------------------------------------------------------------------------

/// Build a complete Responses API success response.
pub fn build_responses_success(
    response_id: &str,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut output = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text,
            }],
        }));
    }

    for tool_call in tool_calls {
        output.push(json!({
            "type": "function_call",
            "id": tool_call.id,
            "call_id": tool_call.id,
            "name": tool_call.name,
            "arguments": tool_call.arguments,
        }));
    }

    let mut response = json!({
        "id": response_id,
        "object": "response",
        "model": model,
        "output": output,
        "status": finish_reason.unwrap_or(if tool_calls.is_empty() { "completed" } else { "tool_calls" }),
    });

    if let Some(usage) = usage {
        response["usage"] = json!({
            "input_tokens": usage.prompt_tokens,
            "output_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
            "input_tokens_details": {
                "cached_tokens": usage.cache_read_input_tokens.unwrap_or(0),
            },
        });
    }

    response
}

pub async fn accumulate_responses_stream(
    response: rquest::Response,
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let body = response.bytes().await.map_err(|error| {
        GatewayError::server_error(format!("failed to read responses SSE body: {error}"))
    })?;
    accumulate_responses_sse_bytes(&body, fallback_model)
}

pub async fn accumulate_responses_sse_stream(
    stream: std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    use futures::StreamExt;

    let chunks = stream.collect::<Vec<_>>().await;
    let mut body = Vec::new();
    for chunk in chunks {
        let chunk = chunk.map_err(|error| {
            GatewayError::server_error(format!(
                "failed to read translated responses SSE chunk: {error}"
            ))
        })?;
        body.extend_from_slice(&chunk);
    }
    accumulate_responses_sse_bytes(&body, fallback_model)
}

fn accumulate_responses_sse_bytes(
    body: &[u8],
    fallback_model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let text = String::from_utf8_lossy(body);
    let mut parser = SseParseState::new();
    let mut collected_text = String::new();
    let mut completed_response: Option<Value> = None;
    let mut streamed_tool_calls = BTreeMap::<usize, PendingAccumulatedToolCall>::new();

    let mut handle_frame = |frame: SseFrame| -> Result<(), GatewayError> {
        if frame.data.is_empty() || frame.data == "[DONE]" {
            return Ok(());
        }

        let payload: Value = serde_json::from_str(&frame.data).map_err(|error| {
            GatewayError::server_error(format!("failed to parse responses SSE frame: {error}"))
                .with_code("responses_stream_parse_failed")
        })?;
        let event_type = frame
            .event_name
            .as_deref()
            .or_else(|| payload.get("type").and_then(|value| value.as_str()))
            .unwrap_or_default();

        match event_type {
            "error" => {
                return Err(GatewayError::server_error(format!(
                    "responses upstream stream error: {}",
                    frame.data
                ))
                .with_code("responses_stream_upstream_error"));
            }
            "response.output_text.delta" => {
                if let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) {
                    collected_text.push_str(delta);
                }
            }
            "response.output_item.added" | "response.output_item.done" => {
                if let Some(item) = payload.get("item") {
                    collect_streamed_tool_call_item(
                        item,
                        payload.get("output_index").and_then(|value| value.as_u64()),
                        &mut streamed_tool_calls,
                    );
                }
            }
            "response.function_call_arguments.delta" => {
                if let Some(item_id) = payload.get("item_id").and_then(|value| value.as_str()) {
                    let output_index = payload
                        .get("output_index")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .or_else(|| {
                            streamed_tool_calls.iter().find_map(|(index, call)| {
                                if call.item_id == item_id {
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                        });
                    if let (Some(output_index), Some(delta)) = (
                        output_index,
                        payload.get("delta").and_then(|value| value.as_str()),
                    ) {
                        let entry = streamed_tool_calls.entry(output_index).or_insert_with(|| {
                            PendingAccumulatedToolCall {
                                item_id: item_id.to_string(),
                                call_id: item_id.to_string(),
                                name: None,
                                arguments: String::new(),
                            }
                        });
                        entry.arguments.push_str(delta);
                    }
                }
            }
            "response.function_call_arguments.done" => {
                if let Some(item_id) = payload.get("item_id").and_then(|value| value.as_str()) {
                    let output_index = payload
                        .get("output_index")
                        .and_then(|value| value.as_u64())
                        .map(|value| value as usize)
                        .or_else(|| {
                            streamed_tool_calls.iter().find_map(|(index, call)| {
                                if call.item_id == item_id {
                                    Some(*index)
                                } else {
                                    None
                                }
                            })
                        });
                    if let Some(output_index) = output_index {
                        let entry = streamed_tool_calls.entry(output_index).or_insert_with(|| {
                            PendingAccumulatedToolCall {
                                item_id: item_id.to_string(),
                                call_id: item_id.to_string(),
                                name: None,
                                arguments: String::new(),
                            }
                        });
                        if let Some(arguments) =
                            payload.get("arguments").and_then(|value| value.as_str())
                        {
                            entry.arguments = arguments.to_string();
                        }
                    }
                }
            }
            "response.completed" => {
                if let Some(response) = payload.get("response") {
                    completed_response = Some(response.clone());
                }
            }
            _ => {}
        }

        Ok(())
    };

    for line in text.split('\n') {
        if let Some(frame) = parse_sse_frame_line(line, &mut parser) {
            handle_frame(frame)?;
        }
    }
    if let Some(frame) = parse_sse_frame_line("", &mut parser) {
        handle_frame(frame)?;
    }

    if let Some(response_body) = completed_response {
        let mut canonical = unpack_responses_response(&response_body)?;
        if canonical.model == "unknown" {
            canonical.model = fallback_model.to_string();
        }
        if canonical.text.is_empty() && !collected_text.is_empty() {
            canonical.text = collected_text;
        }
        if canonical.tool_calls.is_empty() && !streamed_tool_calls.is_empty() {
            canonical.tool_calls = streamed_tool_calls
                .into_values()
                .map(|call| CanonicalToolCall {
                    id: Some(call.call_id),
                    call_type: "function".to_string(),
                    name: call.name,
                    arguments: Some(crate::protocol::accio::normalize_tool_args(&call.arguments)),
                    raw: std::collections::HashMap::new(),
                })
                .collect();
        }
        if !canonical.tool_calls.is_empty() {
            canonical.finish_reason = Some("tool_calls".to_string());
        }
        return Ok(canonical);
    }

    if !collected_text.is_empty() || !streamed_tool_calls.is_empty() {
        let has_text = !collected_text.is_empty();
        return Ok(CanonicalRelayResponse {
            model: fallback_model.to_string(),
            text: collected_text,
            usage: None,
            tool_calls: streamed_tool_calls
                .into_values()
                .map(|call| CanonicalToolCall {
                    id: Some(call.call_id),
                    call_type: "function".to_string(),
                    name: call.name,
                    arguments: Some(crate::protocol::accio::normalize_tool_args(&call.arguments)),
                    raw: std::collections::HashMap::new(),
                })
                .collect(),
            upstream_status: Some(200),
            finish_reason: Some(if has_text {
                "stop".to_string()
            } else {
                "tool_calls".to_string()
            }),
        });
    }

    Err(GatewayError::server_error(
        "Responses stream ended without a completed frame or text payload.",
    )
    .with_code("responses_stream_incomplete"))
}

#[derive(Debug, Clone)]
struct PendingAccumulatedToolCall {
    item_id: String,
    call_id: String,
    name: Option<String>,
    arguments: String,
}

fn collect_streamed_tool_call_item(
    item: &Value,
    output_index: Option<u64>,
    streamed_tool_calls: &mut BTreeMap<usize, PendingAccumulatedToolCall>,
) {
    let item_type = item.get("type").and_then(|value| value.as_str());
    if !matches!(item_type, Some("function_call") | Some("custom_tool_call")) {
        return;
    }

    let Some(output_index) = output_index.map(|value| value as usize) else {
        return;
    };

    let entry =
        streamed_tool_calls
            .entry(output_index)
            .or_insert_with(|| PendingAccumulatedToolCall {
                item_id: item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                call_id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: None,
                arguments: String::new(),
            });

    if let Some(item_id) = item.get("id").and_then(|value| value.as_str()) {
        if !item_id.trim().is_empty() {
            entry.item_id = item_id.to_string();
        }
    }
    if let Some(call_id) = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(|value| value.as_str())
    {
        if !call_id.trim().is_empty() {
            entry.call_id = call_id.to_string();
        }
    }
    if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
        if !name.trim().is_empty() {
            entry.name = Some(name.to_string());
        }
    }
    if let Some(arguments) = item.get("arguments").and_then(|value| value.as_str()) {
        if !arguments.is_empty() {
            entry.arguments = arguments.to_string();
        }
    }
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

// ---------------------------------------------------------------------------
// Streaming translators
// ---------------------------------------------------------------------------

/// Wrap an OpenAI chat-completions SSE byte stream and translate it to native
/// OpenAI Responses API SSE events on-the-fly.
///
/// If the upstream is already producing native Responses SSE, the stream is
/// passed through unchanged.
pub fn translate_openai_sse_to_responses(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let state = OpenAiToResponsesState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        response_id: format!("resp_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        native_passthrough: false,
        response_started: false,
        pending_finish_status: None,
        latest_usage: None,
        message: None,
        tool_calls: BTreeMap::new(),
        next_output_index: 0,
        final_emitted: false,
        sequence_number: 0,
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
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
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

pub fn translate_responses_sse_to_openai_chat(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let state = ResponsesToOpenAiChatState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        response_id: format!("chatcmpl_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        passthrough_openai: false,
        latest_usage: None,
        pending_finish_reason: None,
        emitted_text: false,
        tool_call_indexes: BTreeMap::new(),
        announced_tool_calls: BTreeSet::new(),
        next_tool_call_index: 0,
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
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
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

#[derive(Debug, Clone)]
struct PendingResponseMessage {
    item_id: String,
    output_index: usize,
    text: String,
    done: bool,
}

#[derive(Debug, Clone)]
struct PendingResponseToolCall {
    item_id: String,
    call_id: String,
    output_index: usize,
    name: Option<String>,
    arguments: String,
    pending_argument_deltas: Vec<String>,
    announced: bool,
    done: bool,
}

struct ResponsesToOpenAiChatState {
    buffer: Vec<u8>,
    parser: SseParseState,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    passthrough_openai: bool,
    latest_usage: Option<TokenUsage>,
    pending_finish_reason: Option<String>,
    emitted_text: bool,
    tool_call_indexes: BTreeMap<String, usize>,
    announced_tool_calls: BTreeSet<String>,
    next_tool_call_index: usize,
    final_emitted: bool,
}

impl ResponsesToOpenAiChatState {
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

        if self.passthrough_openai || frame_is_openai_chat(frame.data.as_str()) {
            self.passthrough_openai = true;
            self.outputs
                .push_back(format_sse_event(frame.event_name.as_deref(), &frame.data).into_bytes());
            if frame.data == "[DONE]" {
                self.final_emitted = true;
            }
            return;
        }

        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return;
        }

        let Ok(payload) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };
        let event_type = frame
            .event_name
            .as_deref()
            .or_else(|| payload.get("type").and_then(|value| value.as_str()))
            .unwrap_or_default();

        match event_type {
            "response.created" | "response.in_progress" => {
                if let Some(response) = payload.get("response") {
                    if let Some(id) = response.get("id").and_then(|value| value.as_str()) {
                        if !id.trim().is_empty() {
                            self.response_id = id.to_string();
                        }
                    }
                    if let Some(model) = response.get("model").and_then(|value| value.as_str()) {
                        if !model.trim().is_empty() {
                            self.model = model.to_string();
                        }
                    }
                    if let Some(created_at) =
                        response.get("created_at").and_then(|value| value.as_i64())
                    {
                        self.created_at = created_at;
                    }
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) {
                    if !delta.is_empty() {
                        self.emitted_text = true;
                        self.outputs.push_back(
                            format_sse_event(
                                None,
                                &openai::build_chat_completions_delta(
                                    &self.response_id,
                                    self.created_at,
                                    &self.model,
                                    delta,
                                )
                                .to_string(),
                            )
                            .into_bytes(),
                        );
                    }
                }
            }
            "response.output_item.added" => {
                let Some(item) = payload.get("item") else {
                    return;
                };
                if item.get("type").and_then(|value| value.as_str()) != Some("function_call") {
                    return;
                }
                let item_id = item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let call_id = item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or(item_id.as_str())
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let index = self.tool_call_index_for(&item_id);
                if self.announced_tool_calls.insert(item_id.clone()) {
                    self.outputs.push_back(
                        format_sse_event(
                            None,
                            &build_openai_chat_tool_call_start_chunk(
                                &self.response_id,
                                self.created_at,
                                &self.model,
                                index,
                                &call_id,
                                &name,
                            )
                            .to_string(),
                        )
                        .into_bytes(),
                    );
                }
            }
            "response.function_call_arguments.delta" => {
                let item_id = payload
                    .get("item_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let delta = payload
                    .get("delta")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if item_id.is_empty() || delta.is_empty() {
                    return;
                }
                let index = self.tool_call_index_for(&item_id);
                self.outputs.push_back(
                    format_sse_event(
                        None,
                        &build_openai_chat_tool_call_arguments_chunk(
                            &self.response_id,
                            self.created_at,
                            &self.model,
                            index,
                            delta,
                        )
                        .to_string(),
                    )
                    .into_bytes(),
                );
            }
            "response.completed" => {
                if let Some(response) = payload.get("response") {
                    if let Some(id) = response.get("id").and_then(|value| value.as_str()) {
                        if !id.trim().is_empty() {
                            self.response_id = id.to_string();
                        }
                    }
                    if let Some(model) = response.get("model").and_then(|value| value.as_str()) {
                        if !model.trim().is_empty() {
                            self.model = model.to_string();
                        }
                    }
                    if let Some(created_at) =
                        response.get("created_at").and_then(|value| value.as_i64())
                    {
                        self.created_at = created_at;
                    }
                    if let Ok(mut canonical) = unpack_responses_response(response) {
                        if canonical.tool_calls.is_empty() && !canonical.text.is_empty() {
                            let parse_result =
                                tool_inject::parse_tool_calls_from_text(&canonical.text);
                            if parse_result.had_tool_calls {
                                canonical.text = parse_result.clean_text;
                                canonical.tool_calls = parse_result.tool_calls;
                                canonical.finish_reason = Some("tool_calls".to_string());
                            }
                        }
                        self.latest_usage =
                            merge_stream_usage(self.latest_usage.take(), canonical.usage.clone());
                        self.pending_finish_reason = canonical.finish_reason.clone();
                        if !self.emitted_text && !canonical.text.is_empty() {
                            self.emitted_text = true;
                            self.outputs.push_back(
                                format_sse_event(
                                    None,
                                    &openai::build_chat_completions_delta(
                                        &self.response_id,
                                        self.created_at,
                                        &self.model,
                                        &canonical.text,
                                    )
                                    .to_string(),
                                )
                                .into_bytes(),
                            );
                        }
                        for tool_call in canonical.tool_calls {
                            let item_id = tool_call
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("call_{}", self.next_tool_call_index));
                            let index = self.tool_call_index_for(&item_id);
                            if self.announced_tool_calls.insert(item_id.clone()) {
                                self.outputs.push_back(
                                    format_sse_event(
                                        None,
                                        &build_openai_chat_tool_call_start_chunk(
                                            &self.response_id,
                                            self.created_at,
                                            &self.model,
                                            index,
                                            tool_call.id.as_deref().unwrap_or(item_id.as_str()),
                                            tool_call.name.as_deref().unwrap_or("tool"),
                                        )
                                        .to_string(),
                                    )
                                    .into_bytes(),
                                );
                            }
                            if let Some(arguments) = tool_call.arguments.as_deref() {
                                if !arguments.is_empty() {
                                    self.outputs.push_back(
                                        format_sse_event(
                                            None,
                                            &build_openai_chat_tool_call_arguments_chunk(
                                                &self.response_id,
                                                self.created_at,
                                                &self.model,
                                                index,
                                                arguments,
                                            )
                                            .to_string(),
                                        )
                                        .into_bytes(),
                                    );
                                }
                            }
                        }
                    }
                }
                self.emit_final_if_needed(None);
            }
            _ => {}
        }
    }

    fn tool_call_index_for(&mut self, item_id: &str) -> usize {
        if let Some(index) = self.tool_call_indexes.get(item_id).copied() {
            return index;
        }
        let index = self.next_tool_call_index;
        self.next_tool_call_index += 1;
        self.tool_call_indexes.insert(item_id.to_string(), index);
        index
    }

    fn emit_final_if_needed(&mut self, fallback_finish_reason: Option<&str>) {
        if self.final_emitted || self.passthrough_openai {
            return;
        }
        self.outputs.push_back(
            format_sse_event(
                None,
                &build_openai_chat_stop_chunk(
                    &self.response_id,
                    self.created_at,
                    &self.model,
                    self.latest_usage.as_ref(),
                    self.pending_finish_reason
                        .as_deref()
                        .or(fallback_finish_reason),
                )
                .to_string(),
            )
            .into_bytes(),
        );
        self.outputs
            .push_back(format_sse_event(None, "[DONE]").into_bytes());
        self.final_emitted = true;
    }
}

struct OpenAiToResponsesState {
    buffer: Vec<u8>,
    parser: SseParseState,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    native_passthrough: bool,
    response_started: bool,
    pending_finish_status: Option<String>,
    latest_usage: Option<TokenUsage>,
    message: Option<PendingResponseMessage>,
    tool_calls: BTreeMap<usize, PendingResponseToolCall>,
    next_output_index: usize,
    final_emitted: bool,
    sequence_number: u64,
}

impl OpenAiToResponsesState {
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

        if self.native_passthrough || frame_is_native_responses(&frame) {
            self.native_passthrough = true;
            self.outputs
                .push_back(format_sse_event(frame.event_name.as_deref(), &frame.data).into_bytes());
            if frame.data == "[DONE]" {
                self.final_emitted = true;
            }
            return;
        }

        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return;
        }

        let Ok(chunk) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };

        if let Some(model) = chunk.get("model").and_then(|value| value.as_str()) {
            if !model.trim().is_empty() {
                self.model = model.to_string();
            }
        }

        if let Some(usage) = extract_openai_stream_usage(&chunk) {
            self.latest_usage = merge_stream_usage(self.latest_usage.take(), Some(usage));
        }

        self.ensure_response_started();

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            self.handle_choice(choice);
        }

        if self.pending_finish_status.is_some() && choice.is_none() && self.latest_usage.is_some() {
            self.emit_final_if_needed(None);
        }
    }

    fn handle_choice(&mut self, choice: &Value) {
        let delta = choice.get("delta");
        let finish_status = choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .and_then(map_openai_finish_reason_to_responses_status);

        if let Some(text) = delta
            .and_then(|entry| entry.get("content"))
            .and_then(|entry| entry.as_str())
        {
            if !text.is_empty() {
                self.handle_text_delta(text);
            }
        }

        if let Some(tool_calls) = delta
            .and_then(|entry| entry.get("tool_calls"))
            .and_then(|entry| entry.as_array())
        {
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                self.handle_tool_call_delta(tool_call, fallback_index);
            }
        }

        if let Some(status) = finish_status {
            self.pending_finish_status = Some(status);
            if self.latest_usage.is_some() {
                self.emit_final_if_needed(None);
            }
        }
    }

    fn ensure_response_started(&mut self) {
        if self.response_started {
            return;
        }
        self.response_started = true;
        let created_seq = self.next_sequence_number();
        let created = build_response_created_event(
            created_seq,
            &self.response_id,
            &self.model,
            self.created_at,
        );
        self.outputs.push_back(created.into_bytes());

        let progress_seq = self.next_sequence_number();
        let in_progress = build_response_in_progress_event(
            progress_seq,
            &self.response_id,
            &self.model,
            self.created_at,
        );
        self.outputs.push_back(in_progress.into_bytes());
    }

    fn handle_text_delta(&mut self, text: &str) {
        let (output_index, item_id) = self.ensure_message_item();
        if let Some(message) = self.message.as_mut() {
            message.text.push_str(text);
        }
        let sequence_number = self.next_sequence_number();
        let delta = build_response_output_text_delta(sequence_number, output_index, &item_id, text);
        self.outputs.push_back(delta.into_bytes());
    }

    fn ensure_message_item(&mut self) -> (usize, String) {
        if let Some(message) = &self.message {
            return (message.output_index, message.item_id.clone());
        }

        let output_index = self.allocate_output_index();
        let item_id = format!("msg_{}", uuid::Uuid::new_v4().as_simple());
        let item_added_seq = self.next_sequence_number();
        let item_added = build_response_message_item_added(item_added_seq, output_index, &item_id);
        self.outputs.push_back(item_added.into_bytes());

        let part_added_seq = self.next_sequence_number();
        let part_added = build_response_content_part_added(part_added_seq, output_index, &item_id);
        self.outputs.push_back(part_added.into_bytes());
        self.message = Some(PendingResponseMessage {
            item_id: item_id.clone(),
            output_index,
            text: String::new(),
            done: false,
        });
        (output_index, item_id)
    }

    fn handle_tool_call_delta(&mut self, tool_call: &Value, fallback_index: usize) {
        let openai_index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);

        let mut add_event = None;
        let mut delta_event = None;
        let sequence_number = &mut self.sequence_number;
        let next_output_index = &mut self.next_output_index;

        {
            let entry = self.tool_calls.entry(openai_index).or_insert_with(|| {
                let output_index = *next_output_index;
                *next_output_index += 1;
                let synthetic_call_id = format!("call_{}", uuid::Uuid::new_v4().as_simple());
                PendingResponseToolCall {
                    item_id: format!("fc_{synthetic_call_id}"),
                    call_id: synthetic_call_id,
                    output_index,
                    name: None,
                    arguments: String::new(),
                    pending_argument_deltas: Vec::new(),
                    announced: false,
                    done: false,
                }
            });

            if !entry.announced {
                if let Some(call_id) = tool_call.get("id").and_then(|value| value.as_str()) {
                    if !call_id.trim().is_empty() {
                        entry.call_id = call_id.to_string();
                        entry.item_id = format!("fc_{call_id}");
                    }
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
                if let Some(arguments) = tool_call
                    .get("function")
                    .and_then(|function| function.get("arguments"))
                    .and_then(|value| value.as_str())
                {
                    if !arguments.is_empty() {
                        entry.arguments.push_str(arguments);
                        entry.pending_argument_deltas.push(arguments.to_string());
                    }
                }

                if let Some(name) = entry.name.clone() {
                    *sequence_number += 1;
                    add_event = Some(build_response_function_call_item_added(
                        *sequence_number,
                        entry.output_index,
                        &entry.item_id,
                        &entry.call_id,
                        &name,
                    ));
                    if !entry.pending_argument_deltas.is_empty() {
                        let mut batched = Vec::new();
                        for pending_delta in entry.pending_argument_deltas.drain(..) {
                            *sequence_number += 1;
                            batched.push(build_response_function_call_arguments_delta(
                                *sequence_number,
                                entry.output_index,
                                &entry.item_id,
                                &pending_delta,
                            ));
                        }
                        delta_event = Some(batched);
                    }
                    entry.announced = true;
                }
            } else if let Some(arguments) = tool_call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(|value| value.as_str())
            {
                if !arguments.is_empty() {
                    entry.arguments.push_str(arguments);
                    *sequence_number += 1;
                    delta_event = Some(vec![build_response_function_call_arguments_delta(
                        *sequence_number,
                        entry.output_index,
                        &entry.item_id,
                        arguments,
                    )]);
                }
            }
        }

        if let Some(event) = add_event {
            self.outputs.push_back(event.into_bytes());
        }
        if let Some(events) = delta_event {
            for event in events {
                self.outputs.push_back(event.into_bytes());
            }
        }
    }

    fn emit_final_if_needed(&mut self, fallback_status: Option<&str>) {
        if self.final_emitted || self.native_passthrough {
            return;
        }

        self.ensure_response_started();
        self.close_open_items();

        let finish_status = self
            .pending_finish_status
            .clone()
            .or_else(|| fallback_status.map(str::to_string));
        let response = self.build_completed_response(finish_status.as_deref());

        let sequence_number = self.next_sequence_number();
        let completed = build_response_completed_event(sequence_number, response);
        self.outputs.push_back(completed.into_bytes());
        self.final_emitted = true;
    }

    fn close_open_items(&mut self) {
        let mut close_order = Vec::new();
        if let Some(message) = self.message.as_ref() {
            if !message.done {
                close_order.push((message.output_index, None));
            }
        }
        for (openai_index, tool_call) in &self.tool_calls {
            if !tool_call.done {
                close_order.push((tool_call.output_index, Some(*openai_index)));
            }
        }
        close_order.sort_by_key(|(output_index, _)| *output_index);

        for (_, tool_index) in close_order {
            match tool_index {
                Some(openai_index) => self.close_tool_item(openai_index),
                None => self.close_message_item(),
            }
        }
    }

    fn close_message_item(&mut self) {
        let Some((output_index, item_id, text)) = self
            .message
            .as_ref()
            .filter(|message| !message.done)
            .map(|message| {
                (
                    message.output_index,
                    message.item_id.clone(),
                    message.text.clone(),
                )
            })
        else {
            return;
        };

        let text_done_seq = self.next_sequence_number();
        let text_done =
            build_response_output_text_done(text_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(text_done.into_bytes());

        let part_done_seq = self.next_sequence_number();
        let part_done =
            build_response_content_part_done(part_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(part_done.into_bytes());

        let item_done_seq = self.next_sequence_number();
        let item_done =
            build_response_message_item_done(item_done_seq, output_index, &item_id, &text);
        self.outputs.push_back(item_done.into_bytes());

        if let Some(message) = self.message.as_mut() {
            message.done = true;
        }
    }

    fn close_tool_item(&mut self, openai_index: usize) {
        let Some(snapshot) = self
            .tool_calls
            .get(&openai_index)
            .filter(|entry| !entry.done)
            .map(|entry| {
                (
                    entry.announced,
                    entry.output_index,
                    entry.item_id.clone(),
                    entry.call_id.clone(),
                    entry
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("tool_{openai_index}")),
                    crate::protocol::accio::normalize_tool_args(&entry.arguments),
                )
            })
        else {
            return;
        };

        let (announced, output_index, item_id, call_id, name, arguments) = snapshot;

        let add_event = if !announced {
            let add_seq = self.next_sequence_number();
            Some(build_response_function_call_item_added(
                add_seq,
                output_index,
                &item_id,
                &call_id,
                &name,
            ))
        } else {
            None
        };

        let args_done_seq = self.next_sequence_number();
        let args_done_event = build_response_function_call_arguments_done(
            args_done_seq,
            output_index,
            &item_id,
            &arguments,
        );

        let item_done_seq = self.next_sequence_number();
        let item_done_event = build_response_function_call_item_done(
            item_done_seq,
            output_index,
            &item_id,
            &call_id,
            &name,
            &arguments,
        );

        if let Some(entry) = self.tool_calls.get_mut(&openai_index) {
            entry.name = Some(name);
            entry.arguments = arguments;
            entry.pending_argument_deltas.clear();
            entry.announced = true;
            entry.done = true;
        }

        if let Some(event) = add_event {
            self.outputs.push_back(event.into_bytes());
        }
        self.outputs.push_back(args_done_event.into_bytes());
        self.outputs.push_back(item_done_event.into_bytes());
    }

    fn allocate_output_index(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        index
    }

    fn next_sequence_number(&mut self) -> u64 {
        self.sequence_number = self.sequence_number.saturating_add(1);
        self.sequence_number
    }

    fn build_completed_response(&self, finish_status: Option<&str>) -> Value {
        let mut output = Vec::<(usize, Value)>::new();

        if let Some(message) = self.message.as_ref() {
            output.push((
                message.output_index,
                json!({
                    "id": message.item_id,
                    "type": "message",
                    "status": "completed",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": message.text,
                    }],
                }),
            ));
        }

        for tool_call in self.tool_calls.values() {
            output.push((
                tool_call.output_index,
                json!({
                    "id": tool_call.item_id,
                    "type": "function_call",
                    "status": "completed",
                    "call_id": tool_call.call_id,
                    "name": tool_call.name.clone().unwrap_or_default(),
                    "arguments": crate::protocol::accio::normalize_tool_args(&tool_call.arguments),
                }),
            ));
        }

        output.sort_by_key(|(output_index, _)| *output_index);
        let output = output.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
        let has_tool_calls = self
            .tool_calls
            .values()
            .any(|entry| !entry.call_id.is_empty());
        let status = finish_status.map(str::to_string).unwrap_or_else(|| {
            if has_tool_calls {
                "tool_calls".into()
            } else {
                "completed".into()
            }
        });

        let mut response = json!({
            "id": self.response_id,
            "object": "response",
            "model": self.model,
            "output": output,
            "status": status,
        });

        if let Some(usage) = self.latest_usage.as_ref() {
            response["usage"] = json!({
                "input_tokens": usage.prompt_tokens,
                "output_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
                "input_tokens_details": {
                    "cached_tokens": usage.cache_read_input_tokens.unwrap_or(0),
                },
            });
        }

        response
    }
}

fn frame_is_native_responses(frame: &SseFrame) -> bool {
    if frame
        .event_name
        .as_deref()
        .map(|name| name.starts_with("response.") || name == "error")
        .unwrap_or(false)
    {
        return true;
    }

    serde_json::from_str::<Value>(&frame.data)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(|entry| entry.as_str())
                .map(str::to_string)
        })
        .map(|value| value.starts_with("response.") || value == "error")
        .unwrap_or(false)
}

fn frame_is_openai_chat(data: &str) -> bool {
    serde_json::from_str::<Value>(data)
        .ok()
        .and_then(|value| {
            value
                .get("choices")
                .and_then(|choices| choices.as_array())
                .and_then(|choices| {
                    choices.first().map(|choice| {
                        choice.get("delta").is_some()
                            || choice
                                .get("message")
                                .and_then(|message| message.get("content"))
                                .is_some()
                    })
                })
        })
        .unwrap_or(false)
}

fn build_openai_chat_tool_call_start_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    index: usize,
    call_id: &str,
    name: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": index,
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": "",
                    }
                }]
            },
            "finish_reason": null,
        }],
    })
}

fn build_openai_chat_tool_call_arguments_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    index: usize,
    arguments_delta: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": index,
                    "function": {
                        "arguments": arguments_delta,
                    }
                }]
            },
            "finish_reason": null,
        }],
    })
}

fn build_openai_chat_stop_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": match finish_reason.unwrap_or("stop") {
                "completed" => "stop",
                "incomplete" => "length",
                other => other,
            },
        }],
    });

    if let Some(usage) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        });
    }

    chunk
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

fn merge_stream_usage(
    existing: Option<TokenUsage>,
    incoming: Option<TokenUsage>,
) -> Option<TokenUsage> {
    match (existing, incoming) {
        (None, None) => None,
        (Some(current), None) => Some(current),
        (None, Some(new_usage)) => Some(new_usage),
        (Some(current), Some(new_usage)) => Some(TokenUsage {
            prompt_tokens: current.prompt_tokens.max(new_usage.prompt_tokens),
            completion_tokens: current.completion_tokens.max(new_usage.completion_tokens),
            total_tokens: current.total_tokens.max(new_usage.total_tokens).max(
                current
                    .prompt_tokens
                    .max(new_usage.prompt_tokens)
                    .saturating_add(current.completion_tokens.max(new_usage.completion_tokens)),
            ),
            cache_creation_input_tokens: new_usage
                .cache_creation_input_tokens
                .or(current.cache_creation_input_tokens),
            cache_read_input_tokens: new_usage
                .cache_read_input_tokens
                .or(current.cache_read_input_tokens),
        }),
    }
}

fn map_openai_finish_reason_to_responses_status(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(
        match value {
            "tool_calls" | "function_call" => "tool_calls",
            "stop" | "completed" => "completed",
            "length" | "max_tokens" => "incomplete",
            other => other,
        }
        .to_string(),
    )
}

fn build_response_created_event(
    sequence_number: u64,
    response_id: &str,
    model: &str,
    created_at: i64,
) -> String {
    let payload = json!({
        "type": "response.created",
        "sequence_number": sequence_number,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "in_progress",
            "output": [],
        }
    });
    format_sse_event(Some("response.created"), &payload.to_string())
}

fn build_response_in_progress_event(
    sequence_number: u64,
    response_id: &str,
    model: &str,
    created_at: i64,
) -> String {
    let payload = json!({
        "type": "response.in_progress",
        "sequence_number": sequence_number,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "in_progress",
        }
    });
    format_sse_event(Some("response.in_progress"), &payload.to_string())
}

fn build_response_message_item_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "message",
            "status": "in_progress",
            "role": "assistant",
            "content": [],
        }
    });
    format_sse_event(Some("response.output_item.added"), &payload.to_string())
}

fn build_response_content_part_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
) -> String {
    let payload = json!({
        "type": "response.content_part.added",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "part": {
            "type": "output_text",
            "text": "",
        }
    });
    format_sse_event(Some("response.content_part.added"), &payload.to_string())
}

fn build_response_output_text_delta(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> String {
    let payload = json!({
        "type": "response.output_text.delta",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "delta": delta,
    });
    format_sse_event(Some("response.output_text.delta"), &payload.to_string())
}

fn build_response_output_text_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.output_text.done",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "text": text,
    });
    format_sse_event(Some("response.output_text.done"), &payload.to_string())
}

fn build_response_content_part_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.content_part.done",
        "sequence_number": sequence_number,
        "item_id": item_id,
        "output_index": output_index,
        "content_index": 0,
        "part": {
            "type": "output_text",
            "text": text,
        }
    });
    format_sse_event(Some("response.content_part.done"), &payload.to_string())
}

fn build_response_message_item_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    text: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "message",
            "status": "completed",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": text,
            }],
        }
    });
    format_sse_event(Some("response.output_item.done"), &payload.to_string())
}

fn build_response_function_call_item_added(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    call_id: &str,
    name: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "function_call",
            "status": "in_progress",
            "call_id": call_id,
            "name": name,
            "arguments": "",
        }
    });
    format_sse_event(Some("response.output_item.added"), &payload.to_string())
}

fn build_response_function_call_arguments_delta(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    delta: &str,
) -> String {
    let payload = json!({
        "type": "response.function_call_arguments.delta",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item_id": item_id,
        "delta": delta,
    });
    format_sse_event(
        Some("response.function_call_arguments.delta"),
        &payload.to_string(),
    )
}

fn build_response_function_call_arguments_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    arguments: &str,
) -> String {
    let payload = json!({
        "type": "response.function_call_arguments.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item_id": item_id,
        "arguments": arguments,
    });
    format_sse_event(
        Some("response.function_call_arguments.done"),
        &payload.to_string(),
    )
}

fn build_response_function_call_item_done(
    sequence_number: u64,
    output_index: usize,
    item_id: &str,
    call_id: &str,
    name: &str,
    arguments: &str,
) -> String {
    let payload = json!({
        "type": "response.output_item.done",
        "sequence_number": sequence_number,
        "output_index": output_index,
        "item": {
            "id": item_id,
            "type": "function_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        }
    });
    format_sse_event(Some("response.output_item.done"), &payload.to_string())
}

fn build_response_completed_event(sequence_number: u64, response: Value) -> String {
    let payload = json!({
        "type": "response.completed",
        "sequence_number": sequence_number,
        "response": response,
    });
    format_sse_event(Some("response.completed"), &payload.to_string())
}

pub fn unpack_responses_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let model = body
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();

    let output = body
        .get("output")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Responses API response missing `output` array")
        })?;

    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for item in output {
        match item.get("type").and_then(|value| value.as_str()) {
            Some("message") => {
                if let Some(content) = item.get("content").and_then(|value| value.as_array()) {
                    for block in content {
                        if block.get("type").and_then(|value| value.as_str()) == Some("output_text")
                        {
                            if let Some(text) = block.get("text").and_then(|value| value.as_str()) {
                                text_parts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            Some("function_call") | Some("custom_tool_call") => {
                tool_calls.push(CanonicalToolCall {
                    id: item
                        .get("call_id")
                        .or_else(|| item.get("id"))
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    call_type: item
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("function")
                        .to_string(),
                    name: item
                        .get("name")
                        .or_else(|| item.get("function").and_then(|value| value.get("name")))
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    arguments: item
                        .get("arguments")
                        .or_else(|| {
                            item.get("function")
                                .and_then(|value| value.get("arguments"))
                        })
                        .map(normalize_arguments_value),
                    raw: std::collections::HashMap::new(),
                });
            }
            _ => {}
        }
    }

    let usage = body.get("usage").map(|usage| {
        let prompt_tokens = usage
            .get("input_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let completion_tokens = usage
            .get("output_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: usage
                .get("total_tokens")
                .and_then(|value| value.as_u64())
                .unwrap_or(prompt_tokens + completion_tokens),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: usage
                .get("input_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(|value| value.as_u64()),
        }
    });

    let has_tool_calls = !tool_calls.is_empty();

    Ok(CanonicalRelayResponse {
        model,
        text: text_parts.join(""),
        usage,
        tool_calls,
        upstream_status: body
            .get("_upstream_status")
            .and_then(|value| value.as_u64())
            .map(|status| status as u16),
        finish_reason: map_responses_finish_reason(
            body.get("status").and_then(|value| value.as_str()),
            has_tool_calls,
        ),
    })
}

fn normalize_responses_input_item(item: &Value) -> Result<Vec<CanonicalMessage>, GatewayError> {
    let item_type = item.get("type").and_then(|value| value.as_str());
    match item_type {
        Some("function_call_output") | Some("custom_tool_call_output") => {
            Ok(vec![CanonicalMessage {
                role: MessageRole::Tool,
                content: parse_responses_output_content(item.get("output")),
                name: None,
                tool_call_id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                tool_calls: vec![],
            }])
        }
        Some("function_call") | Some("custom_tool_call") => Ok(vec![CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                call_type: item_type.unwrap_or("function").to_string(),
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                arguments: item.get("arguments").map(normalize_arguments_value),
                raw: std::collections::HashMap::new(),
            }],
        }]),
        _ => {
            let role_str = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let role = match role_str {
                "system" => MessageRole::System,
                "assistant" => MessageRole::Assistant,
                _ => MessageRole::User,
            };

            let content = match item.get("content") {
                Some(Value::String(s)) => vec![ContentPart::Text { text: s.clone() }],
                Some(Value::Array(parts)) => parts
                    .iter()
                    .filter_map(parse_responses_content_part)
                    .collect(),
                Some(other) => vec![ContentPart::Raw {
                    value: other.clone(),
                }],
                None => vec![],
            };

            Ok(vec![CanonicalMessage {
                role,
                content,
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }])
        }
    }
}

fn parse_responses_content_part(part: &Value) -> Option<ContentPart> {
    match part.get("type").and_then(|value| value.as_str()) {
        Some("input_text") | Some("output_text") | Some("text") => part
            .get("text")
            .and_then(|value| value.as_str())
            .map(|text| ContentPart::Text {
                text: text.to_string(),
            }),
        _ => Some(ContentPart::Raw {
            value: part.clone(),
        }),
    }
}

fn parse_responses_output_content(output: Option<&Value>) -> Vec<ContentPart> {
    match output {
        None | Some(Value::Null) => vec![],
        Some(Value::String(text)) => vec![ContentPart::Text { text: text.clone() }],
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(parse_responses_content_part)
            .collect(),
        Some(other) => vec![ContentPart::Raw {
            value: other.clone(),
        }],
    }
}

fn parse_responses_tools(raw: Option<&Value>) -> Vec<CanonicalTool> {
    let arr = match raw.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .map(|tool| {
            let is_function = tool.get("type").and_then(|value| value.as_str()) == Some("function");
            let function = if is_function {
                tool.get("function").or(Some(tool))
            } else {
                Some(tool)
            };

            CanonicalTool {
                tool_type: tool
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("function")
                    .to_string(),
                name: function
                    .and_then(|value| value.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                description: function
                    .and_then(|value| value.get("description"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                input_schema: function
                    .and_then(|value| {
                        value
                            .get("parameters")
                            .or_else(|| value.get("input_schema"))
                    })
                    .cloned(),
                raw: tool
                    .as_object()
                    .map(|map| {
                        map.iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn normalize_arguments_value(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        text.to_string()
    } else {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    }
}

fn map_responses_finish_reason(value: Option<&str>, has_tool_calls: bool) -> Option<String> {
    match value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some("tool_calls") => Some("tool_calls".to_string()),
        Some("completed") if has_tool_calls => Some("tool_calls".to_string()),
        Some(other) => Some(other.to_string()),
        None if has_tool_calls => Some("tool_calls".to_string()),
        None => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::{Stream, StreamExt};
    use serde_json::json;

    use crate::protocol::accio;
    use crate::protocol::sse_parse::{
        parse_sse_line as parse_sse_frame_line, SseFrame, SseParseState,
    };

    fn make_bytes_stream(
        chunks: Vec<Result<Bytes, rquest::Error>>,
    ) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
        futures::stream::iter(chunks)
    }

    async fn collect_sse_frames(
        stream: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    ) -> Vec<SseFrame> {
        let chunks = stream.collect::<Vec<_>>().await;
        let mut state = SseParseState::new();
        let mut frames = Vec::new();

        for chunk in chunks {
            let bytes = chunk.unwrap();
            let text = String::from_utf8_lossy(&bytes);
            for line in text.split('\n') {
                if let Some(frame) = parse_sse_frame_line(line, &mut state) {
                    frames.push(frame);
                }
            }
        }

        if let Some(frame) = parse_sse_frame_line("", &mut state) {
            frames.push(frame);
        }

        frames
    }

    #[test]
    fn normalize_string_input() {
        let body = json!({
            "model": "gpt-4o",
            "input": "Tell me a joke.",
        });
        let req = normalize_responses(body).unwrap();
        assert_eq!(req.requested_model.as_deref(), Some("gpt-4o"));
        assert_eq!(req.endpoint_kind, EndpointKind::Responses);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::User);
        assert_eq!(
            req.messages[0].content[0].as_text(),
            Some("Tell me a joke.")
        );
    }

    #[test]
    fn normalize_with_instructions() {
        let body = json!({
            "model": "gpt-4o",
            "instructions": "Be concise.",
            "input": "Hello",
        });
        let req = normalize_responses(body).unwrap();
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert_eq!(req.messages[0].content[0].as_text(), Some("Be concise."));
        assert_eq!(req.messages[1].role, MessageRole::User);
    }

    #[test]
    fn pack_responses_infers_default_instructions_without_system_prompt() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Say hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4",
                "messages": [{ "role": "user", "content": "Say hello" }],
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses(&req, "gpt-5.4", true);
        assert_eq!(
            body.get("instructions").and_then(|value| value.as_str()),
            Some(
                "You are a helpful assistant. Follow the conversation in `input` and respond directly."
            )
        );
        assert!(body
            .get("input")
            .and_then(|value| value.as_array())
            .is_some());
    }

    #[test]
    fn pack_responses_bridge_strips_max_output_tokens_for_minimal_provider_compatibility() {
        let mut extra = std::collections::HashMap::new();
        extra.insert("max_tokens".to_string(), json!(256));
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::Anthropic,
            endpoint_kind: EndpointKind::Messages,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Say hello".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4",
                "system": "You are terse.",
                "max_tokens": 256,
                "messages": [{ "role": "user", "content": "Say hello" }],
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra,
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", true);
        assert!(body
            .get("input")
            .and_then(|value| value.as_array())
            .is_some());
        assert!(body.get("max_output_tokens").is_none());
    }

    #[test]
    fn pack_responses_bridge_serializes_text_tool_results_as_plain_output_strings() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Text {
                    text: "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}".to_string(),
                }],
                name: None,
                tool_call_id: Some("call_1".to_string()),
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(
            body["input"][0],
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}"
            })
        );
    }

    #[test]
    fn pack_responses_bridge_serializes_json_tool_results_as_plain_output_strings() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::Anthropic,
            endpoint_kind: EndpointKind::Messages,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::Tool,
                content: vec![ContentPart::Json {
                    value: json!({"city": "Hangzhou", "weather": "sunny"}),
                }],
                name: None,
                tool_call_id: Some("call_1".to_string()),
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(
            body["input"][0],
            json!({
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "{\"city\":\"Hangzhou\",\"weather\":\"sunny\"}"
            })
        );
    }

    #[test]
    fn pack_responses_bridge_uses_output_text_for_assistant_history_messages() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "Previous assistant summary.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(
            body["input"][0],
            json!({
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Previous assistant summary."
                }]
            })
        );
    }

    #[test]
    fn pack_responses_bridge_skips_empty_assistant_placeholders_before_function_calls() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::Assistant,
                content: vec![ContentPart::Text {
                    text: String::new(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![CanonicalToolCall {
                    id: Some("call_weather".to_string()),
                    call_type: "function".to_string(),
                    name: Some("weather".to_string()),
                    arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                    raw: std::collections::HashMap::new(),
                }],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        let input = body["input"].as_array().cloned().unwrap_or_default();
        assert_eq!(input.len(), 1);
        assert_eq!(
            input[0],
            json!({
                "type": "function_call",
                "call_id": "call_weather",
                "name": "weather",
                "arguments": "{\"city\":\"Hangzhou\"}"
            })
        );
    }

    #[test]
    fn pack_responses_bridge_rewraps_untyped_raw_parts_as_input_text() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::Responses,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Raw {
                    value: json!({"city": "Hangzhou"}),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(
            body["input"][0],
            json!({
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": "{\"city\":\"Hangzhou\"}"
                }]
            })
        );
    }

    #[test]
    fn pack_responses_maps_anthropic_any_tool_choice_to_required() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::Anthropic,
            endpoint_kind: EndpointKind::Messages,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use the tool".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![CanonicalTool {
                tool_type: "function".to_string(),
                name: Some("weather".to_string()),
                description: None,
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } }
                })),
                raw: std::collections::HashMap::new(),
            }],
            tool_choice: Some(json!({ "type": "any" })),
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4",
                "tool_choice": { "type": "any" }
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(body.get("tool_choice"), Some(&json!("required")));
    }

    #[test]
    fn pack_responses_maps_openai_specific_tool_choice_to_responses_function_object() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use the tool".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![CanonicalTool {
                tool_type: "function".to_string(),
                name: Some("weather".to_string()),
                description: None,
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } }
                })),
                raw: std::collections::HashMap::new(),
            }],
            tool_choice: Some(json!({ "type": "function", "function": { "name": "weather" } })),
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": "gpt-5.4",
                "tool_choice": { "type": "function", "function": { "name": "weather" } }
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let body = pack_responses_bridge(&req, "gpt-5.4", false);
        assert_eq!(
            body.get("tool_choice"),
            Some(&json!({"type": "function", "name": "weather"}))
        );
    }

    #[test]
    fn normalize_messages_array_input() {
        let body = json!({
            "model": "gpt-4o",
            "input": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"},
                {"role": "user", "content": "How are you?"},
            ],
        });
        let req = normalize_responses(body).unwrap();
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.messages[0].role, MessageRole::User);
        assert_eq!(req.messages[1].role, MessageRole::Assistant);
        assert_eq!(req.messages[2].role, MessageRole::User);
    }

    #[test]
    fn normalize_missing_input_returns_error() {
        let body = json!({"model": "gpt-4o"});
        assert!(normalize_responses(body).is_err());
    }

    #[test]
    fn normalize_captures_previous_response_id() {
        let body = json!({
            "model": "gpt-4o",
            "input": "Continue",
            "previous_response_id": "resp_abc123",
        });
        let req = normalize_responses(body).unwrap();
        assert_eq!(req.previous_response_id.as_deref(), Some("resp_abc123"));
    }

    #[test]
    fn build_responses_success_structure() {
        let usage = TokenUsage {
            prompt_tokens: 12,
            completion_tokens: 8,
            total_tokens: 20,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: Some(4),
        };
        let resp = build_responses_success("resp_abc", "gpt-4o", "Hello!", Some(&usage), &[], None);
        assert_eq!(resp["id"], "resp_abc");
        assert_eq!(resp["object"], "response");
        assert_eq!(resp["status"], "completed");
        assert_eq!(resp["output"][0]["content"][0]["text"], "Hello!");
        assert_eq!(resp["usage"]["input_tokens"], 12);
        assert_eq!(resp["usage"]["output_tokens"], 8);
        assert_eq!(resp["usage"]["input_tokens_details"]["cached_tokens"], 4);
    }

    #[test]
    fn unpack_responses_response_reads_usage() {
        let body = json!({
            "id": "resp_test",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello"
                }]
            }],
            "usage": {
                "input_tokens": 9,
                "output_tokens": 6,
                "total_tokens": 15,
                "input_tokens_details": {
                    "cached_tokens": 3
                }
            }
        });
        let response = unpack_responses_response(&body).unwrap();
        assert_eq!(response.text, "Hello");
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 9);
        assert_eq!(usage.completion_tokens, 6);
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(usage.cache_read_input_tokens, Some(3));
    }

    #[test]
    fn normalize_responses_parses_function_call_and_output_items() {
        let body = json!({
            "model": "gpt-4o",
            "tools": [{
                "type": "function",
                "name": "weather",
                "parameters": {"type": "object"}
            }],
            "tool_choice": "required",
            "input": [
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "weather",
                    "arguments": {"city": "Hangzhou"}
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "{\"ok\":true}"
                }
            ]
        });
        let req = normalize_responses(body).unwrap();
        assert_eq!(req.tools.len(), 1);
        assert_eq!(req.tool_choice, Some(json!("required")));
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, MessageRole::Assistant);
        assert_eq!(req.messages[0].tool_calls.len(), 1);
        assert_eq!(req.messages[1].role, MessageRole::Tool);
        assert_eq!(req.messages[1].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn normalize_responses_accepts_top_level_function_tool_shape() {
        let body = json!({
            "model": "gpt-4o",
            "input": "Use the weather tool",
            "tools": [{
                "type": "function",
                "name": "weather",
                "description": "Read weather",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    },
                    "required": ["city"]
                }
            }]
        });

        let req = normalize_responses(body).unwrap();
        assert_eq!(req.tools.len(), 1);
        assert_eq!(req.tools[0].name.as_deref(), Some("weather"));
        assert_eq!(req.tools[0].description.as_deref(), Some("Read weather"));
        assert_eq!(
            req.tools[0]
                .input_schema
                .as_ref()
                .and_then(|schema| schema.get("required"))
                .and_then(|value| value.as_array())
                .map(|value| value.len()),
            Some(1)
        );
    }

    #[test]
    fn build_responses_success_emits_function_call_items() {
        let resp = build_responses_success(
            "resp_tools",
            "gpt-4o",
            "",
            None,
            &[CanonicalToolCall {
                id: Some("call_1".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: std::collections::HashMap::new(),
            }],
            Some("tool_calls"),
        );
        assert_eq!(resp["status"], "tool_calls");
        assert_eq!(resp["output"][0]["type"], "function_call");
        assert_eq!(resp["output"][0]["call_id"], "call_1");
    }

    #[test]
    fn unpack_responses_response_reads_function_call_items() {
        let body = json!({
            "id": "resp_tool",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "function_call",
                "call_id": "call_1",
                "name": "weather",
                "arguments": {"city": "Hangzhou"}
            }]
        });
        let response = unpack_responses_response(&body).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn unpack_responses_response_reads_nested_function_call_payload() {
        let body = json!({
            "id": "resp_tool_nested",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "function_call",
                "call_id": "call_weather",
                "function": {
                    "name": "weather",
                    "arguments": {
                        "city": "Hangzhou"
                    }
                }
            }]
        });
        let response = unpack_responses_response(&body).unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            response.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[tokio::test]
    async fn translate_openai_text_stream_to_responses_events() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}

"#,
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ];

        let frames = collect_sse_frames(translate_openai_sse_to_responses(
            make_bytes_stream(chunks),
            "gpt-4o".to_string(),
        ))
        .await;

        let event_names = frames
            .iter()
            .map(|frame| frame.event_name.clone().unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(
            event_names.first().map(String::as_str),
            Some("response.created")
        );
        assert!(event_names.contains(&"response.output_text.delta".to_string()));
        assert_eq!(
            event_names.last().map(String::as_str),
            Some("response.completed")
        );

        let completed = frames
            .iter()
            .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(completed["response"]["status"], "completed");
        assert_eq!(
            completed["response"]["output"][0]["content"][0]["text"],
            "Hello"
        );
        assert_eq!(completed["response"]["usage"]["input_tokens"], 5);
        assert_eq!(completed["response"]["usage"]["output_tokens"], 2);
    }

    #[tokio::test]
    async fn translate_openai_tool_stream_to_responses_events() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"weather","arguments":""}}]},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":\"Hang"}}]},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"zhou\"}"}}]},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}

"#,
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ];

        let frames = collect_sse_frames(translate_openai_sse_to_responses(
            make_bytes_stream(chunks),
            "gpt-4o".to_string(),
        ))
        .await;

        assert!(
            frames.iter().any(|frame| {
                frame.event_name.as_deref() == Some("response.function_call_arguments.delta")
                    && frame.data.contains(r#""delta":"{\"city\":\"Hang""#)
            }),
            "expected function call argument delta event"
        );
        assert!(
            frames.iter().any(|frame| {
                frame.event_name.as_deref() == Some("response.function_call_arguments.done")
                    && frame
                        .data
                        .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)
            }),
            "expected function call argument done event"
        );

        let completed = frames
            .iter()
            .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(completed["response"]["status"], "tool_calls");
        assert_eq!(completed["response"]["output"][0]["type"], "function_call");
        assert_eq!(completed["response"]["output"][0]["id"], "fc_call_1");
        assert_eq!(completed["response"]["output"][0]["call_id"], "call_1");
        assert_eq!(
            completed["response"]["output"][0]["arguments"],
            "{\"city\":\"Hangzhou\"}"
        );
    }

    #[tokio::test]
    async fn translate_responses_text_stream_to_openai_chat_events() {
        let chunks = vec![
            Ok(Bytes::from(format_sse_event(
                Some("response.created"),
                &json!({
                    "type": "response.created",
                    "response": {
                        "id": "resp_test",
                        "model": "gpt-5.4",
                        "created_at": 1700000000
                    }
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.output_text.delta"),
                &json!({
                    "type": "response.output_text.delta",
                    "delta": "Hel"
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.output_text.delta"),
                &json!({
                    "type": "response.output_text.delta",
                    "delta": "lo"
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.completed"),
                &json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_test",
                        "model": "gpt-5.4",
                        "created_at": 1700000000,
                        "status": "completed",
                        "output": [{
                            "type": "message",
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": "Hello"
                            }]
                        }],
                        "usage": {
                            "input_tokens": 5,
                            "output_tokens": 2,
                            "total_tokens": 7
                        }
                    }
                })
                .to_string(),
            ))),
        ];

        let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
            make_bytes_stream(chunks),
            "gpt-5.4".to_string(),
        ))
        .await;

        assert!(
            frames
                .iter()
                .any(|frame| frame.data.contains(r#""content":"Hel""#)),
            "expected chat delta chunk: {frames:?}"
        );
        assert!(
            frames
                .iter()
                .any(|frame| frame.data.contains(r#""finish_reason":"stop""#)),
            "expected stop chunk: {frames:?}"
        );
        let stop = frames
            .iter()
            .find(|frame| frame.data.contains(r#""finish_reason":"stop""#))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(stop["usage"]["total_tokens"], 7);
    }

    #[tokio::test]
    async fn translate_responses_tool_stream_to_openai_chat_events() {
        let chunks = vec![
            Ok(Bytes::from(format_sse_event(
                Some("response.output_item.added"),
                &json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": {
                        "id": "fc_call_1",
                        "type": "function_call",
                        "status": "in_progress",
                        "call_id": "call_1",
                        "name": "weather",
                        "arguments": ""
                    }
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.function_call_arguments.delta"),
                &json!({
                    "type": "response.function_call_arguments.delta",
                    "output_index": 0,
                    "item_id": "fc_call_1",
                    "delta": "{\"city\":\"Hangzhou\"}"
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.completed"),
                &json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_tool",
                        "model": "gpt-5.4",
                        "created_at": 1700000001,
                        "status": "tool_calls",
                        "output": [{
                            "type": "function_call",
                            "call_id": "call_1",
                            "name": "weather",
                            "arguments": "{\"city\":\"Hangzhou\"}"
                        }],
                        "usage": {
                            "input_tokens": 8,
                            "output_tokens": 3,
                            "total_tokens": 11
                        }
                    }
                })
                .to_string(),
            ))),
        ];

        let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
            make_bytes_stream(chunks),
            "gpt-5.4".to_string(),
        ))
        .await;

        assert!(
            frames.iter().any(|frame| {
                frame.data.contains(r#""tool_calls":[{"#)
                    && frame.data.contains(r#""id":"call_1""#)
                    && frame.data.contains(r#""name":"weather""#)
            }),
            "expected tool call start chunk: {frames:?}"
        );
        assert!(
            frames.iter().any(|frame| frame
                .data
                .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)),
            "expected tool call arguments chunk"
        );
        assert!(
            frames
                .iter()
                .any(|frame| frame.data.contains(r#""finish_reason":"tool_calls""#)),
            "expected tool_calls finish reason"
        );
    }

    #[tokio::test]
    async fn translate_responses_completed_xml_tool_text_to_openai_chat_events() {
        let chunks = vec![
            Ok(Bytes::from(format_sse_event(
                Some("response.output_text.delta"),
                &json!({
                    "type": "response.output_text.delta",
                    "output_index": 0,
                    "item_id": "msg_1",
                    "delta": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
                })
                .to_string(),
            ))),
            Ok(Bytes::from(format_sse_event(
                Some("response.completed"),
                &json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_xml_tool",
                        "model": "qwen3.5-flash",
                        "created_at": 1700000002_i64,
                        "status": "completed",
                        "output": [{
                            "id": "msg_1",
                            "type": "message",
                            "role": "assistant",
                            "status": "completed",
                            "content": [{
                                "type": "output_text",
                                "text": "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>"
                            }]
                        }],
                        "usage": {
                            "input_tokens": 8,
                            "output_tokens": 4,
                            "total_tokens": 12
                        }
                    }
                })
                .to_string(),
            ))),
        ];

        let frames = collect_sse_frames(translate_responses_sse_to_openai_chat(
            make_bytes_stream(chunks),
            "qwen3.5-flash".to_string(),
        ))
        .await;

        assert!(
            frames.iter().any(|frame| {
                frame.data.contains(r#""tool_calls":[{"#)
                    && frame.data.contains(r#""name":"weather""#)
            }),
            "expected tool call start chunk from XML fallback: {frames:?}"
        );
        assert!(
            frames.iter().any(|frame| frame
                .data
                .contains(r#""arguments":"{\"city\":\"Hangzhou\"}""#)),
            "expected tool call arguments chunk from XML fallback: {frames:?}"
        );
        assert!(
            frames
                .iter()
                .any(|frame| frame.data.contains(r#""finish_reason":"tool_calls""#)),
            "expected tool_calls finish reason from XML fallback: {frames:?}"
        );
    }

    #[test]
    fn accumulate_responses_sse_bytes_restores_tool_calls_from_item_events() {
        let sse = [
            format_sse_event(
                Some("response.output_item.added"),
                &json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": {
                        "id": "fc_call_1",
                        "type": "function_call",
                        "status": "in_progress",
                        "call_id": "call_1",
                        "name": "weather",
                        "arguments": ""
                    }
                })
                .to_string(),
            ),
            format_sse_event(
                Some("response.function_call_arguments.delta"),
                &json!({
                    "type": "response.function_call_arguments.delta",
                    "output_index": 0,
                    "item_id": "fc_call_1",
                    "delta": "{\"city\":\"Hangzhou\"}"
                })
                .to_string(),
            ),
            format_sse_event(
                Some("response.completed"),
                &json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_1",
                        "model": "gpt-5.4",
                        "status": "completed",
                        "output": [],
                        "usage": {
                            "input_tokens": 8,
                            "output_tokens": 3,
                            "total_tokens": 11
                        }
                    }
                })
                .to_string(),
            ),
        ]
        .join("");

        let response = accumulate_responses_sse_bytes(sse.as_bytes(), "gpt-5.4").unwrap();
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            response.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[tokio::test]
    async fn translate_native_responses_stream_passthroughs() {
        let created = br#"event: response.created
data: {"type":"response.created","response":{"id":"resp_1","model":"gpt-4o","created_at":1}}

"#;
        let completed = br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}}

"#;
        let done = b"data: [DONE]\n\n";
        let chunks = vec![
            Ok(Bytes::from_static(created)),
            Ok(Bytes::from_static(completed)),
            Ok(Bytes::from_static(done)),
        ];

        let translated =
            translate_openai_sse_to_responses(make_bytes_stream(chunks), "gpt-4o".to_string())
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
                .collect::<String>();

        let original = [
            String::from_utf8_lossy(created).to_string(),
            String::from_utf8_lossy(completed).to_string(),
            String::from_utf8_lossy(done).to_string(),
        ]
        .join("");

        assert_eq!(translated, original);
    }

    #[tokio::test]
    async fn translate_native_responses_error_passthroughs() {
        let error = br#"event: error
data: {"type":"error","sequence_number":1,"code":"bad_request","message":"boom"}

"#;
        let done = b"data: [DONE]\n\n";
        let chunks = vec![Ok(Bytes::from_static(error)), Ok(Bytes::from_static(done))];

        let translated =
            translate_openai_sse_to_responses(make_bytes_stream(chunks), "gpt-4o".to_string())
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
                .collect::<String>();

        let original = [
            String::from_utf8_lossy(error).to_string(),
            String::from_utf8_lossy(done).to_string(),
        ]
        .join("");

        assert_eq!(translated, original);
    }

    #[tokio::test]
    async fn translate_openai_tool_stream_waits_for_real_name_before_added_event() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"arguments":"{\"city\":\"Hang"}}]},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"weather","arguments":"zhou\"}"}}]},"finish_reason":null}]}

"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"id":"chatcmpl_1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}

"#,
            )),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ];

        let frames = collect_sse_frames(translate_openai_sse_to_responses(
            make_bytes_stream(chunks),
            "gpt-4o".to_string(),
        ))
        .await;

        let added = frames
            .iter()
            .find(|frame| frame.event_name.as_deref() == Some("response.output_item.added"))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(added["item"]["type"], "function_call");
        assert_eq!(added["item"]["name"], "weather");

        let completed = frames
            .iter()
            .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(completed["response"]["output"][0]["id"], "fc_call_1");
        assert_eq!(completed["response"]["output"][0]["call_id"], "call_1");
        assert_eq!(completed["response"]["output"][0]["name"], "weather");
        assert_eq!(
            completed["response"]["output"][0]["arguments"],
            "{\"city\":\"Hangzhou\"}"
        );
    }

    #[tokio::test]
    async fn translate_anthropic_like_stream_to_responses_events() {
        let chunks = vec![
            Ok(Bytes::from_static(
                br#"event: message_start
data: {"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":13,"output_tokens":0}}}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":9,"cache_read_input_tokens":4}}

"#,
            )),
            Ok(Bytes::from_static(
                br#"event: message_stop
data: {"type":"message_stop"}

"#,
            )),
        ];

        let openai_stream = accio::translate_anthropic_like_stream_to_openai(
            make_bytes_stream(chunks),
            "claude-sonnet-4-6".to_string(),
        );
        let openai_chunks = openai_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|chunk| String::from_utf8_lossy(&chunk.unwrap()).to_string())
            .collect::<String>();
        let frames = collect_sse_frames(translate_openai_sse_to_responses(
            make_bytes_stream(vec![Ok(Bytes::from(openai_chunks.clone()))]),
            "claude-sonnet-4-6".to_string(),
        ))
        .await;

        let completed = frames
            .iter()
            .find(|frame| frame.event_name.as_deref() == Some("response.completed"))
            .map(|frame| serde_json::from_str::<Value>(&frame.data).unwrap())
            .unwrap();
        assert_eq!(completed["response"]["status"], "completed");
        assert!(completed["response"]["output"][0]["id"].as_str().is_some());
        assert_eq!(completed["response"]["output"][0]["status"], "completed");
        assert_eq!(
            completed["response"]["output"][0]["content"][0]["text"],
            "Hi"
        );
        assert_eq!(completed["response"]["usage"]["input_tokens"], 13);
        assert_eq!(completed["response"]["usage"]["output_tokens"], 9);
        assert_eq!(
            completed["response"]["usage"]["input_tokens_details"]["cached_tokens"],
            4
        );
    }
}
