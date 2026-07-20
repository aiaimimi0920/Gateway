use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};

use crate::error::{ErrorKind, FallbackHint, GatewayError};
use crate::implementation_lines;
use crate::protocol::anthropic::PromptCacheTelemetry;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool,
    CanonicalToolCall, ContentPart, MessageRole, TokenUsage,
};

pub fn pack_accio(_req: &CanonicalRelayRequest, _model: &str, _stream: bool) -> Value {
    json!({ "compiledOut": true, "provider": "accio_compatible" })
}

pub fn inspect_prompt_cache_telemetry(
    _req: &CanonicalRelayRequest,
    _model: &str,
) -> PromptCacheTelemetry {
    PromptCacheTelemetry::default()
}

pub fn unpack_accio_response(_body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "response decoding requested for a compiled-out Accio line",
        ),
    )
}

pub fn normalize_accio(_body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "direct Accio ingress requested for a compiled-out Accio line",
        ),
    )
}

pub fn translate_accio_sse_to_openai(
    _line: &[u8],
    _model: &str,
    _response_id: &str,
    _created: i64,
) -> Option<Vec<u8>> {
    None
}

pub async fn accumulate_accio_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(
        implementation_lines::accio_web_reverse_api_compiled_out_error(
            "stream accumulation requested for a compiled-out Accio line",
        ),
    )
}

pub fn translate_anthropic_like_stream_to_openai(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::empty()
}

pub fn translate_accio_stream(
    _inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::empty()
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    announced: bool,
}

pub(crate) enum ParsedEvent {
    Start {
        model: Option<String>,
        usage: Option<TokenUsage>,
    },
    ProviderError {
        code: String,
        message: String,
    },
    Text(String),
    ToolStart {
        index: i64,
        id: String,
        name: String,
    },
    ToolDelta {
        index: i64,
        partial: String,
    },
    ToolEnd {
        index: i64,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    Finish {
        reason: Option<String>,
        usage: Option<TokenUsage>,
    },
    Done,
}

struct TranslatorState {
    buffer: Vec<u8>,
    model: String,
    response_id: String,
    created: i64,
    usage: Option<TokenUsage>,
    pending: HashMap<i64, PendingToolCall>,
    outputs: VecDeque<Vec<u8>>,
    finish_seen: bool,
    done_sent: bool,
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn request_value<'a>(req: &'a CanonicalRelayRequest, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = req.extra.get(*key) {
            return Some(value);
        }
        if let Some(value) = req.raw_body.get(*key) {
            return Some(value);
        }
    }
    None
}

fn request_string(req: &CanonicalRelayRequest, keys: &[&str]) -> Option<String> {
    request_value(req, keys).and_then(value_to_string)
}

fn request_u64(req: &CanonicalRelayRequest, keys: &[&str]) -> Option<u64> {
    request_value(req, keys).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|v| v.max(0) as u64))
            .or_else(|| value.as_str().and_then(|v| v.parse::<u64>().ok()))
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn normalize_stop_sequences(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string()]
            }
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::trim).map(str::to_string))
            .filter(|item| !item.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn normalize_tool_args(arguments: &str) -> String {
    let arguments = arguments.trim();
    if arguments.is_empty() {
        "{}".into()
    } else {
        arguments.into()
    }
}

fn map_finish_reason(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "end_turn" | "stop" | "stop_sequence" | "complete" | "COMPLETE" => "stop",
            "tool_use" | "tool_call" | "TOOL_CALL" | "function_call" => "tool_calls",
            "max_tokens" | "length" | "MAX_TOKENS" => "length",
            other => other,
        }
        .to_string(),
    )
}

fn usage_from_value(value: Option<&Value>) -> Option<TokenUsage> {
    let value = value?;
    let usage = value
        .get("usageMetadata")
        .or_else(|| value.get("usage"))
        .unwrap_or(value);
    let prompt_tokens = usage
        .get("promptTokenCount")
        .or_else(|| usage.get("inputTokens"))
        .or_else(|| usage.get("input_tokens"))
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("candidatesTokenCount")
        .or_else(|| usage.get("outputTokens"))
        .or_else(|| usage.get("output_tokens"))
        .or_else(|| usage.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .or_else(|| usage.get("totalTokens"))
        .or_else(|| usage.get("total_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(prompt_tokens + completion_tokens);

    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        None
    } else {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(|entry| entry.as_u64()),
            cache_read_input_tokens: usage
                .get("cache_read_input_tokens")
                .and_then(|entry| entry.as_u64())
                .or_else(|| {
                    usage
                        .get("prompt_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                })
                .or_else(|| {
                    usage
                        .get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|entry| entry.as_u64())
                }),
        })
    }
}

fn merge_usage(existing: Option<TokenUsage>, incoming: Option<TokenUsage>) -> Option<TokenUsage> {
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

fn guess_image_mime(image_url: &str) -> &'static str {
    let lower = image_url.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

fn build_contents(req: &CanonicalRelayRequest) -> Vec<Value> {
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

fn ensure_alternating_roles(contents: Vec<Value>) -> Vec<Value> {
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

fn build_properties(req: &CanonicalRelayRequest) -> Map<String, Value> {
    let mut properties = request_value(req, &["properties"])
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    if let Some(metadata) = &req.metadata {
        properties
            .entry("canonical_metadata".to_string())
            .or_insert_with(|| json!(metadata));
    }
    if let Some(metadata) = request_value(req, &["metadata"]) {
        properties
            .entry("openai_metadata".to_string())
            .or_insert_with(|| metadata.clone());
    }
    if let Some(user) = &req.explicit_session_key {
        properties
            .entry("openai_user".to_string())
            .or_insert_with(|| json!(user));
    }
    if let Some(tool_choice) = &req.tool_choice {
        properties
            .entry("openai_tool_choice".to_string())
            .or_insert_with(|| tool_choice.clone());
    }
    if let Some(reasoning) = &req.reasoning {
        properties
            .entry("openai_reasoning".to_string())
            .or_insert_with(|| reasoning.clone());
    }
    if let Some(previous_response_id) = &req.previous_response_id {
        properties
            .entry("openai_previous_response_id".to_string())
            .or_insert_with(|| json!(previous_response_id));
    }
    for (aliases, key) in [
        (&["session_id", "sessionId"][..], "openai_session_id"),
        (
            &["conversation_id", "conversationId"][..],
            "openai_conversation_id",
        ),
        (&["text"][..], "openai_text"),
        (&["truncation"][..], "openai_truncation"),
        (&["include"][..], "openai_include"),
    ] {
        if let Some(value) = request_value(req, aliases) {
            properties
                .entry(key.to_string())
                .or_insert_with(|| value.clone());
        }
    }
    properties
}

fn apply_thinking(body: &mut Map<String, Value>, req: &CanonicalRelayRequest) {
    let reasoning = req
        .reasoning
        .as_ref()
        .or_else(|| request_value(req, &["thinking"]));
    let Some(reasoning) = reasoning.and_then(|v| v.as_object()) else {
        return;
    };
    let thinking_type = reasoning
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("enabled")
        .to_ascii_lowercase();
    if thinking_type != "enabled" {
        return;
    }
    body.insert("include_thoughts".into(), Value::Bool(true));
    if let Some(budget) = reasoning
        .get("budget_tokens")
        .or_else(|| reasoning.get("budgetTokens"))
        .and_then(|v| v.as_u64())
    {
        body.insert("thinking_budget".into(), json!(budget));
        body.insert(
            "thinking_level".into(),
            json!(if budget >= 12_000 {
                "high"
            } else if budget >= 4_000 {
                "medium"
            } else {
                "low"
            }),
        );
        return;
    }
    let level = reasoning
        .get("level")
        .or_else(|| reasoning.get("effort"))
        .and_then(|v| v.as_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "high".into());
    body.insert(
        "thinking_level".into(),
        json!(match level.as_str() {
            "low" | "medium" | "high" => level,
            _ => "high".into(),
        }),
    );
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "name": tool.name.clone().unwrap_or_default(),
        "description": tool.description.clone().unwrap_or_default(),
        "parameters_json": serde_json::to_string(
            tool.input_schema
                .as_ref()
                .unwrap_or(&json!({"type": "object", "properties": {}}))
        ).unwrap_or_else(|_| "{}".into())
    })
}

fn is_claude_model(model: &str) -> bool {
    model.to_ascii_lowercase().contains("claude")
}

fn should_auto_apply_cache_control(req: &CanonicalRelayRequest, model: &str) -> bool {
    is_claude_model(model) && !auto_cache_disabled(req)
}

fn auto_cache_disabled(req: &CanonicalRelayRequest) -> bool {
    for key in ["gateway_auto_cache", "neuro_auto_cache"] {
        let Some(value) = req.raw_body.get(key).or_else(|| req.extra.get(key)) else {
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

fn find_first_cache_control(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if let Some(cache_control) = map.get("cache_control") {
                return Some(cache_control);
            }
            map.values().find_map(find_first_cache_control)
        }
        Value::Array(items) => items.iter().find_map(find_first_cache_control),
        _ => None,
    }
}

fn is_handled_extra_key(key: &str) -> bool {
    matches!(
        key,
        "max_tokens"
            | "max_completion_tokens"
            | "max_output_tokens"
            | "temperature"
            | "top_p"
            | "response_format"
            | "stop"
            | "stop_sequences"
            | "properties"
            | "metadata"
            | "tool_choice"
            | "session_id"
            | "sessionId"
            | "conversation_id"
            | "conversationId"
            | "conversation_name"
            | "conversationName"
            | "request_id"
            | "requestId"
            | "message_id"
            | "messageId"
            | "gateway_auto_cache"
            | "neuro_auto_cache"
            | "text"
            | "include"
            | "truncation"
            | "thinking"
    )
}

pub(crate) fn parse_sse_line(line: &[u8]) -> Option<Vec<ParsedEvent>> {
    let line = std::str::from_utf8(line).ok()?.trim();
    let payload = line.strip_prefix("data:")?.trim();
    if payload.is_empty() {
        return None;
    }
    if payload == "[DONE]" {
        return Some(vec![ParsedEvent::Done]);
    }
    let outer: Value = serde_json::from_str(payload).ok()?;
    let raw = if let Some(raw_json) = outer.get("raw_response_json").and_then(|v| v.as_str()) {
        serde_json::from_str(raw_json).ok()?
    } else {
        outer
    };
    Some(parse_raw_event(&raw))
}

fn drain_parsed_events(buffer: &mut Vec<u8>) -> Vec<ParsedEvent> {
    let mut events = Vec::new();
    loop {
        if let Some((payload, consumed)) = try_parse_aws_eventstream_payload(buffer) {
            buffer.drain(..consumed);
            events.extend(parse_eventstream_payload(&payload));
            continue;
        }

        let Some(pos) = buffer.iter().position(|&b| b == b'\n') else {
            if let Some(parsed) = parse_sse_line(buffer) {
                events.extend(parsed);
                buffer.clear();
            }
            break;
        };
        let line: Vec<u8> = buffer.drain(..=pos).collect();
        if let Some(parsed) = parse_sse_line(&line) {
            events.extend(parsed);
        }
    }
    events
}

fn try_parse_aws_eventstream_payload(buffer: &[u8]) -> Option<(Vec<u8>, usize)> {
    if buffer.len() < 16 {
        return None;
    }
    let total_len = u32::from_be_bytes(buffer[0..4].try_into().ok()?) as usize;
    let headers_len = u32::from_be_bytes(buffer[4..8].try_into().ok()?) as usize;
    if total_len < 16 || total_len > buffer.len() {
        return None;
    }
    if 12 + headers_len > total_len.saturating_sub(4) {
        return None;
    }
    let payload_start = 12 + headers_len;
    let payload_end = total_len - 4;
    Some((buffer[payload_start..payload_end].to_vec(), total_len))
}

fn parse_eventstream_payload(payload: &[u8]) -> Vec<ParsedEvent> {
    let Ok(text) = std::str::from_utf8(payload) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    parse_raw_event(&value)
}

fn parse_raw_event(raw: &Value) -> Vec<ParsedEvent> {
    let mut events = Vec::new();
    if let Some(error) = parse_provider_error(raw) {
        events.push(error);
        return events;
    }

    if let Some(event) = raw.get("messageStart") {
        events.push(ParsedEvent::Start {
            model: event
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            usage: event.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(event) = raw.get("contentBlockStart") {
        if let Some(tool_use) = event
            .get("start")
            .and_then(|v| v.get("toolUse"))
            .or_else(|| event.get("toolUse"))
        {
            events.push(ParsedEvent::ToolStart {
                index: event
                    .get("contentBlockIndex")
                    .or_else(|| event.get("index"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                id: tool_use
                    .get("toolUseId")
                    .or_else(|| tool_use.get("id"))
                    .and_then(value_to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: tool_use
                    .get("name")
                    .and_then(value_to_string)
                    .unwrap_or_default(),
            });
        }
        return events;
    }

    if let Some(event) = raw.get("contentBlockDelta") {
        let index = event
            .get("contentBlockIndex")
            .or_else(|| event.get("index"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if let Some(text) = event
            .get("delta")
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
        {
            events.push(ParsedEvent::Text(text.to_string()));
        }
        if let Some(partial) = event
            .get("delta")
            .and_then(|v| {
                v.get("partial_json")
                    .or_else(|| v.get("partialJson"))
                    .or_else(|| v.get("input"))
                    .or_else(|| v.get("toolUse").and_then(|entry| entry.get("input")))
            })
            .and_then(|v| v.as_str())
        {
            events.push(ParsedEvent::ToolDelta {
                index,
                partial: partial.to_string(),
            });
        }
        return events;
    }

    if let Some(event) = raw.get("contentBlockStop") {
        events.push(ParsedEvent::ToolEnd {
            index: event
                .get("contentBlockIndex")
                .or_else(|| event.get("index"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        });
        return events;
    }

    if let Some(event) = raw.get("messageStop") {
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(
                event
                    .get("stopReason")
                    .or_else(|| event.get("stop_reason"))
                    .and_then(|v| v.as_str()),
            ),
            usage: None,
        });
        return events;
    }

    if let Some(event) = raw.get("metadata") {
        events.push(ParsedEvent::Finish {
            reason: None,
            usage: event.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(event_type) = raw.get("type").and_then(|v| v.as_str()) {
        match event_type {
            "message_start" | "messageStart" => {
                events.push(ParsedEvent::Start {
                    model: raw
                        .get("message")
                        .or_else(|| raw.get("delta").and_then(|v| v.get("message")))
                        .and_then(|v| v.get("model"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    usage: raw
                        .get("message")
                        .or_else(|| raw.get("delta").and_then(|v| v.get("message")))
                        .and_then(|v| v.get("usage"))
                        .and_then(|v| usage_from_value(Some(v))),
                });
            }
            "content_block_start" | "contentBlockStart" => {
                if let Some(block) = raw
                    .get("content_block")
                    .or_else(|| raw.get("contentBlock"))
                    .or_else(|| raw.get("start"))
                {
                    let tool_use = if block.get("type").and_then(|v| v.as_str()) == Some("tool_use")
                    {
                        Some(block)
                    } else {
                        block.get("toolUse")
                    };
                    if let Some(tool_use) = tool_use {
                        events.push(ParsedEvent::ToolStart {
                            index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                            id: tool_use
                                .get("id")
                                .or_else(|| tool_use.get("toolUseId"))
                                .and_then(value_to_string)
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            name: tool_use
                                .get("name")
                                .and_then(value_to_string)
                                .unwrap_or_default(),
                        });
                    }
                }
            }
            "content_block_delta" | "contentBlockDelta" => {
                if let Some(delta) = raw.get("delta") {
                    let mut handled_text = false;
                    let mut handled_tool_delta = false;
                    match delta.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                        "text_delta" => {
                            if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                events.push(ParsedEvent::Text(text.to_string()));
                                handled_text = true;
                            }
                        }
                        "input_json_delta" => {
                            if let Some(partial) =
                                delta.get("partial_json").and_then(|v| v.as_str())
                            {
                                events.push(ParsedEvent::ToolDelta {
                                    index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                                    partial: partial.to_string(),
                                });
                                handled_tool_delta = true;
                            }
                        }
                        _ => {}
                    }
                    if !handled_text {
                        if let Some(text) = delta
                            .get("text")
                            .or_else(|| delta.get("delta").and_then(|v| v.get("text")))
                            .and_then(|v| v.as_str())
                        {
                            events.push(ParsedEvent::Text(text.to_string()));
                        }
                    }
                    if !handled_tool_delta {
                        if let Some(partial) = delta
                            .get("toolUse")
                            .and_then(|v| v.get("input"))
                            .and_then(|v| v.as_str())
                        {
                            events.push(ParsedEvent::ToolDelta {
                                index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                                partial: partial.to_string(),
                            });
                        }
                    }
                }
            }
            "content_block_stop" | "contentBlockStop" => events.push(ParsedEvent::ToolEnd {
                index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
            }),
            "message_delta" | "messageDelta" => events.push(ParsedEvent::Finish {
                reason: map_finish_reason(
                    raw.get("delta")
                        .and_then(|v| v.get("stop_reason").or_else(|| v.get("stopReason")))
                        .and_then(|v| v.as_str()),
                ),
                usage: raw
                    .get("usage")
                    .or_else(|| raw.get("delta").and_then(|v| v.get("usage")))
                    .and_then(|v| usage_from_value(Some(v))),
            }),
            "message_stop" | "messageStop" => events.push(ParsedEvent::Done),
            _ => {}
        }
        return events;
    }

    if let Some(model) = raw
        .get("modelVersion")
        .or_else(|| raw.get("model"))
        .and_then(|v| v.as_str())
    {
        events.push(ParsedEvent::Start {
            model: Some(model.to_string()),
            usage: None,
        });
    }

    if let Some(candidates) = raw.get("candidates").and_then(|v| v.as_array()) {
        if let Some(candidate) = candidates.first() {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|v| v.get("parts"))
                .and_then(|v| v.as_array())
            {
                for part in parts {
                    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                        events.push(ParsedEvent::Text(text.to_string()));
                    }
                    if let Some(function_call) = part
                        .get("functionCall")
                        .or_else(|| part.get("function_call"))
                        .and_then(|v| v.as_object())
                    {
                        events.push(ParsedEvent::ToolCall {
                            id: function_call
                                .get("id")
                                .and_then(value_to_string)
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            name: function_call
                                .get("name")
                                .and_then(value_to_string)
                                .unwrap_or_default(),
                            arguments: function_call
                                .get("argsJson")
                                .or_else(|| function_call.get("args"))
                                .map(|v| {
                                    if let Some(text) = v.as_str() {
                                        text.to_string()
                                    } else {
                                        serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                                    }
                                })
                                .unwrap_or_else(|| "{}".into()),
                        });
                    }
                }
            }
            events.push(ParsedEvent::Finish {
                reason: map_finish_reason(
                    candidate
                        .get("finishReason")
                        .or_else(|| raw.get("finishReason"))
                        .and_then(|v| v.as_str()),
                ),
                usage: raw
                    .get("usageMetadata")
                    .and_then(|v| usage_from_value(Some(v))),
            });
        }
        return events;
    }

    if let Some(output_message) = raw
        .get("output")
        .and_then(|v| v.get("message"))
        .and_then(|v| v.as_object())
    {
        if let Some(content) = output_message.get("content").and_then(|v| v.as_array()) {
            for block in content {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    events.push(ParsedEvent::Text(text.to_string()));
                }
                if let Some(tool_use) = block.get("toolUse").and_then(|v| v.as_object()) {
                    events.push(ParsedEvent::ToolCall {
                        id: tool_use
                            .get("toolUseId")
                            .and_then(value_to_string)
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        name: tool_use
                            .get("name")
                            .and_then(value_to_string)
                            .unwrap_or_default(),
                        arguments: tool_use
                            .get("input")
                            .map(|v| {
                                if let Some(text) = v.as_str() {
                                    text.to_string()
                                } else {
                                    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                                }
                            })
                            .unwrap_or_else(|| "{}".into()),
                    });
                }
            }
        }
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(raw.get("stopReason").and_then(|v| v.as_str())),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(message) = raw.get("message").and_then(|v| v.as_object()) {
        if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
            for block in content {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    events.push(ParsedEvent::Text(text.to_string()));
                }
            }
        } else if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            events.push(ParsedEvent::Text(text.to_string()));
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for tool_call in tool_calls {
                let function = tool_call.get("function").and_then(|v| v.as_object());
                events.push(ParsedEvent::ToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(value_to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: tool_call
                        .get("name")
                        .or_else(|| function.and_then(|f| f.get("name")))
                        .and_then(value_to_string)
                        .unwrap_or_default(),
                    arguments: tool_call
                        .get("arguments")
                        .or_else(|| tool_call.get("parameters"))
                        .or_else(|| function.and_then(|f| f.get("arguments")))
                        .or_else(|| function.and_then(|f| f.get("parameters")))
                        .map(|v| {
                            if let Some(text) = v.as_str() {
                                text.to_string()
                            } else {
                                serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                            }
                        })
                        .unwrap_or_else(|| "{}".into()),
                });
            }
        }

        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(
                raw.get("finish_reason")
                    .or_else(|| message.get("finish_reason"))
                    .and_then(|v| v.as_str()),
            ),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(choices) = raw.get("choices").and_then(|v| v.as_array()) {
        if let Some(choice) = choices.first() {
            if let Some(text) = choice
                .get("delta")
                .and_then(|v| v.get("content"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    choice
                        .get("message")
                        .and_then(|v| v.get("content"))
                        .and_then(|v| v.as_str())
                })
            {
                events.push(ParsedEvent::Text(text.to_string()));
            }
            let tool_calls = choice
                .get("delta")
                .and_then(|v| v.get("tool_calls"))
                .or_else(|| choice.get("message").and_then(|v| v.get("tool_calls")))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for tool_call in tool_calls {
                let Some(function) = tool_call.get("function").and_then(|v| v.as_object()) else {
                    continue;
                };
                events.push(ParsedEvent::ToolCall {
                    id: tool_call
                        .get("id")
                        .and_then(value_to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: function
                        .get("name")
                        .and_then(value_to_string)
                        .unwrap_or_default(),
                    arguments: function
                        .get("arguments")
                        .map(|v| {
                            if let Some(text) = v.as_str() {
                                text.to_string()
                            } else {
                                serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                            }
                        })
                        .unwrap_or_else(|| "{}".into()),
                });
            }
            events.push(ParsedEvent::Finish {
                reason: map_finish_reason(choice.get("finish_reason").and_then(|v| v.as_str())),
                usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
            });
        }
        return events;
    }

    if let Some(tool_calls) = raw.get("tool_calls").and_then(|v| v.as_array()) {
        for tool_call in tool_calls {
            let function = tool_call.get("function").and_then(|v| v.as_object());
            events.push(ParsedEvent::ToolCall {
                id: tool_call
                    .get("id")
                    .and_then(value_to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: tool_call
                    .get("name")
                    .or_else(|| function.and_then(|f| f.get("name")))
                    .and_then(value_to_string)
                    .unwrap_or_default(),
                arguments: tool_call
                    .get("arguments")
                    .or_else(|| tool_call.get("parameters"))
                    .or_else(|| function.and_then(|f| f.get("arguments")))
                    .or_else(|| function.and_then(|f| f.get("parameters")))
                    .map(|v| {
                        if let Some(text) = v.as_str() {
                            text.to_string()
                        } else {
                            serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
                        }
                    })
                    .unwrap_or_else(|| "{}".into()),
            });
        }
        if let Some(text) = raw.get("text").and_then(|v| v.as_str()) {
            events.push(ParsedEvent::Text(text.to_string()));
        }
        events.push(ParsedEvent::Finish {
            reason: map_finish_reason(raw.get("finish_reason").and_then(|v| v.as_str())),
            usage: raw.get("usage").and_then(|v| usage_from_value(Some(v))),
        });
        return events;
    }

    if let Some(event_type) = raw.get("type").and_then(|v| v.as_str()) {
        match event_type {
            "message-start" => {
                events.push(ParsedEvent::Start {
                    model: raw
                        .get("delta")
                        .and_then(|v| v.get("message"))
                        .and_then(|v| v.get("model"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    usage: None,
                });
                return events;
            }
            "tool-call-start" => {
                let tool_call = raw
                    .get("delta")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.get("tool_calls"))
                    .and_then(|v| {
                        if let Some(array) = v.as_array() {
                            array.first()
                        } else {
                            Some(v)
                        }
                    });
                if let Some(tool_call) = tool_call {
                    let function = tool_call.get("function").and_then(|v| v.as_object());
                    events.push(ParsedEvent::ToolStart {
                        index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                        id: tool_call
                            .get("id")
                            .and_then(value_to_string)
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                        name: function
                            .and_then(|f| f.get("name"))
                            .and_then(value_to_string)
                            .unwrap_or_default(),
                    });
                }
                return events;
            }
            "tool-call-delta" => {
                if let Some(partial) = raw
                    .get("delta")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.get("tool_calls"))
                    .and_then(|v| v.get("function"))
                    .and_then(|v| v.get("arguments"))
                    .and_then(|v| v.as_str())
                {
                    events.push(ParsedEvent::ToolDelta {
                        index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                        partial: partial.to_string(),
                    });
                }
                return events;
            }
            "tool-call-end" => {
                events.push(ParsedEvent::ToolEnd {
                    index: raw.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                });
                return events;
            }
            "content-delta" => {
                if let Some(text) = raw
                    .get("delta")
                    .and_then(|v| v.get("message"))
                    .and_then(|v| v.get("content"))
                    .and_then(|v| v.get("text"))
                    .and_then(|v| v.as_str())
                {
                    events.push(ParsedEvent::Text(text.to_string()));
                }
                return events;
            }
            "message-end" => {
                events.push(ParsedEvent::Finish {
                    reason: map_finish_reason(
                        raw.get("delta")
                            .and_then(|v| v.get("finish_reason"))
                            .and_then(|v| v.as_str()),
                    ),
                    usage: raw
                        .get("delta")
                        .and_then(|v| v.get("usage"))
                        .and_then(|v| usage_from_value(Some(v))),
                });
                return events;
            }
            _ => {}
        }
    }
    events
}

pub fn detect_accio_provider_error(chunk: &[u8]) -> Option<GatewayError> {
    let mut buffer = chunk.to_vec();
    for event in drain_parsed_events(&mut buffer) {
        if let ParsedEvent::ProviderError { code, message } = event {
            return Some(classify_accio_provider_error(&code, &message));
        }
    }
    None
}

fn parse_provider_error(raw: &Value) -> Option<ParsedEvent> {
    let turn_complete = raw
        .get("turn_complete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let code = raw
        .get("error_code")
        .or_else(|| raw.get("errorCode"))
        .and_then(value_to_string)?;
    if !turn_complete && code.is_empty() {
        return None;
    }
    if matches!(code.as_str(), "" | "0" | "200") {
        return None;
    }
    let message = raw
        .get("error_message")
        .or_else(|| raw.get("errorMessage"))
        .and_then(value_to_string)
        .unwrap_or_else(|| "unknown accio upstream error".to_string());
    Some(ParsedEvent::ProviderError { code, message })
}

fn classify_accio_provider_error(code: &str, message: &str) -> GatewayError {
    let lower = message.to_ascii_lowercase();
    let err = if code == "5015" || lower.contains("user not activated") {
        GatewayError {
            kind: ErrorKind::ServiceUnavailable,
            message: format!("Accio account unavailable: {}", message.trim()),
            code: None,
            http_status: Some(503),
            retryable: false,
            fallback_hint: FallbackHint::FallbackProvider {
                reason: "Accio account is not activated; try another provider account.".to_string(),
            },
            provider_name: None,
        }
    } else {
        GatewayError::server_error(format!("Accio upstream error: {}", message.trim()))
    };
    err.with_code(code.to_string())
        .with_provider("accio_compatible")
}

fn text_chunk(response_id: &str, model: &str, created: i64, text: &str) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
        })
    )
    .into_bytes()
}

fn tool_chunk(
    response_id: &str,
    model: &str,
    created: i64,
    index: usize,
    tool_id: Option<&str>,
    tool_name: Option<&str>,
    arguments_fragment: &str,
    include_identity: bool,
) -> Vec<u8> {
    let mut tool_delta = json!({
        "index": index,
        "function": {"arguments": arguments_fragment}
    });
    if include_identity {
        tool_delta["id"] = json!(tool_id.unwrap_or_default());
        tool_delta["type"] = json!("function");
        tool_delta["function"]["name"] = json!(tool_name.unwrap_or_default());
    }
    format!(
        "data: {}\n\n",
        json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{"index": 0, "delta": {"tool_calls": [tool_delta]}, "finish_reason": null}]
        })
    )
    .into_bytes()
}

fn finish_chunk(
    response_id: &str,
    model: &str,
    created: i64,
    reason: &str,
    usage: Option<&TokenUsage>,
) -> Vec<u8> {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": {}, "finish_reason": reason}]
    });
    if let Some(usage) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
            "cache_creation_input_tokens": usage.cache_creation_input_tokens,
            "cache_read_input_tokens": usage.cache_read_input_tokens
        });
    }
    format!("data: {}\n\n", chunk).into_bytes()
}

fn translate_events_with_state(
    events: Vec<ParsedEvent>,
    state: &mut TranslatorState,
) -> Vec<Vec<u8>> {
    let mut outputs = Vec::new();
    for event in events {
        match event {
            ParsedEvent::ProviderError { .. } => {}
            ParsedEvent::Text(text) => outputs.push(text_chunk(
                &state.response_id,
                &state.model,
                state.created,
                &text,
            )),
            ParsedEvent::ToolStart { index, id, name } => {
                state.pending.insert(
                    index,
                    PendingToolCall {
                        id,
                        name,
                        arguments: String::new(),
                        announced: false,
                    },
                );
            }
            ParsedEvent::ToolDelta { index, partial } => {
                if let Some(call) = state.pending.get_mut(&index) {
                    call.arguments.push_str(&partial);
                    outputs.push(tool_chunk(
                        &state.response_id,
                        &state.model,
                        state.created,
                        index.max(0) as usize,
                        Some(&call.id),
                        Some(&call.name),
                        &partial,
                        !call.announced,
                    ));
                    call.announced = true;
                }
            }
            ParsedEvent::ToolEnd { index } => {
                if let Some(call) = state.pending.remove(&index) {
                    if !call.announced {
                        outputs.push(tool_chunk(
                            &state.response_id,
                            &state.model,
                            state.created,
                            index.max(0) as usize,
                            Some(&call.id),
                            Some(&call.name),
                            "",
                            true,
                        ));
                    }
                }
            }
            ParsedEvent::ToolCall {
                id,
                name,
                arguments,
            } => outputs.push(tool_chunk(
                &state.response_id,
                &state.model,
                state.created,
                0,
                Some(&id),
                Some(&name),
                &arguments,
                true,
            )),
            ParsedEvent::Finish { reason, usage } => {
                state.finish_seen = true;
                state.usage = merge_usage(state.usage.clone(), usage.clone());
                let mut pending_indexes: Vec<i64> = state.pending.keys().copied().collect();
                pending_indexes.sort_unstable();
                for index in pending_indexes {
                    if let Some(call) = state.pending.remove(&index) {
                        outputs.push(tool_chunk(
                            &state.response_id,
                            &state.model,
                            state.created,
                            index.max(0) as usize,
                            Some(&call.id),
                            Some(&call.name),
                            &call.arguments,
                            true,
                        ));
                    }
                }
                outputs.push(finish_chunk(
                    &state.response_id,
                    &state.model,
                    state.created,
                    reason.as_deref().unwrap_or("stop"),
                    state.usage.as_ref(),
                ));
            }
            ParsedEvent::Done => {
                state.done_sent = true;
                outputs.push(b"data: [DONE]\n\n".to_vec());
            }
            ParsedEvent::Start { model, usage } => {
                if let Some(model) = model {
                    state.model = model;
                }
                state.usage = merge_usage(state.usage.clone(), usage);
            }
        }
    }
    outputs
}

fn to_tool_call(call: PendingToolCall) -> CanonicalToolCall {
    CanonicalToolCall {
        id: Some(call.id),
        call_type: "function".into(),
        name: Some(call.name),
        arguments: Some(normalize_tool_args(&call.arguments)),
        raw: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalTool, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use serde_json::json;

    fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("claude-sonnet-4-6".into()),
            stream: false,
            messages,
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn text_msg(role: MessageRole, text: &str) -> CanonicalMessage {
        CanonicalMessage {
            role,
            content: vec![ContentPart::Text { text: text.into() }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn aws_eventstream_frame(payload: Value) -> Vec<u8> {
        let payload = serde_json::to_vec(&payload).unwrap();
        let total_len = 16 + payload.len();
        let mut frame = Vec::with_capacity(total_len);
        frame.extend_from_slice(&(total_len as u32).to_be_bytes());
        frame.extend_from_slice(&(0u32).to_be_bytes());
        frame.extend_from_slice(&(0u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&(0u32).to_be_bytes());
        frame
    }

    #[test]
    fn pack_includes_current_accio_fields() {
        let mut req = make_request(vec![
            text_msg(MessageRole::System, "system"),
            text_msg(MessageRole::User, "hello"),
        ]);
        req.tools.push(CanonicalTool {
            tool_type: "function".into(),
            name: Some("weather".into()),
            description: Some("Lookup weather".into()),
            input_schema: Some(json!({"type":"object"})),
            raw: HashMap::new(),
        });
        req.reasoning = Some(json!({"type":"enabled","budget_tokens":5000}));
        req.explicit_session_key = Some("user_123".into());
        req.extra
            .insert("conversation_id".into(), json!("conv_123"));

        let body = pack_accio(&req, "claude-sonnet-4-6", true);
        assert_eq!(body["system_instruction"], "system");
        assert_eq!(body["conversation_id"], "conv_123");
        assert_eq!(body["thinking_level"], "medium");
        assert_eq!(body["tools"][0]["name"], "weather");
        assert_eq!(body["properties"]["openai_user"], "user_123");
    }

    #[test]
    fn pack_auto_applies_cache_control_for_claude_when_missing() {
        let req = make_request(vec![text_msg(MessageRole::User, "hello")]);
        let body = pack_accio(&req, "claude-sonnet-4-6", false);
        assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
    }

    #[test]
    fn pack_preserves_existing_cache_control_from_raw_body() {
        let mut req = make_request(vec![text_msg(MessageRole::User, "hello")]);
        req.raw_body = json!({
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hello",
                    "cache_control": {"type": "ephemeral", "ttl": "1h"}
                }]
            }]
        });
        let body = pack_accio(&req, "claude-sonnet-4-6", false);
        assert_eq!(
            body["cache_control"],
            json!({"type": "ephemeral", "ttl": "1h"})
        );
    }

    #[test]
    fn pack_respects_gateway_auto_cache_opt_out() {
        let mut req = make_request(vec![text_msg(MessageRole::User, "hello")]);
        req.raw_body = json!({
            "gateway_auto_cache": false
        });
        let body = pack_accio(&req, "claude-sonnet-4-6", false);
        assert!(body.get("cache_control").is_none());
        let telemetry = inspect_prompt_cache_telemetry(&req, "claude-sonnet-4-6");
        assert!(!telemetry.client_has_cache_control);
        assert!(!telemetry.auto_cache_applied);
    }

    #[test]
    fn unpack_reads_function_call_parts() {
        let body = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Need tool"},
                        {"functionCall": {"id":"call_1","name":"weather","argsJson":"{\"city\":\"Hangzhou\"}"}}
                    ]
                },
                "finishReason": "tool_use"
            }],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15},
            "modelVersion": "claude-sonnet-4-6"
        });
        let response = unpack_accio_response(&body).unwrap();
        assert_eq!(response.text, "Need tool");
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(response.usage.unwrap().cache_read_input_tokens, None);
    }

    #[test]
    fn unpack_reads_bedrock_tool_use_blocks() {
        let body = json!({
            "output": {
                "message": {
                    "content": [
                        {"text": "Need a tool"},
                        {"toolUse": {"toolUseId": "toolu_1", "name": "weather", "input": {"city": "Hangzhou"}}}
                    ]
                }
            },
            "stopReason": "tool_use",
            "usage": {"inputTokens": 11, "outputTokens": 7, "totalTokens": 18},
            "model": "bedrock-claude"
        });
        let response = unpack_accio_response(&body).unwrap();
        assert_eq!(response.text, "Need a tool");
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("toolu_1"));
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(response.usage.unwrap().total_tokens, 18);
    }

    #[test]
    fn unpack_reads_cohere_tool_calls() {
        let body = json!({
            "message": {
                "content": [{"text": "Let me call a tool"}],
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "weather",
                        "arguments": {"city": "Hangzhou"}
                    }
                }]
            },
            "finish_reason": "tool_call",
            "model": "command-r"
        });
        let response = unpack_accio_response(&body).unwrap();
        assert_eq!(response.text, "Let me call a tool");
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            response.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn translate_bedrock_stream_tool_use_to_openai_chunk() {
        let line = br#"data: {"type":"contentBlockStart","index":0,"start":{"toolUse":{"toolUseId":"toolu_1","name":"weather"}}}"#;
        let outputs = parse_sse_line(line).unwrap();
        assert!(matches!(&outputs[0], ParsedEvent::ToolStart { .. }));
    }

    #[tokio::test]
    async fn translate_eventstream_bedrock_frames_to_openai_chunks() {
        use futures::StreamExt;

        let frames = vec![
            aws_eventstream_frame(json!({
                "messageStart": { "model": "bedrock-claude" }
            })),
            aws_eventstream_frame(json!({
                "contentBlockStart": {
                    "contentBlockIndex": 0,
                    "start": { "toolUse": { "toolUseId": "toolu_1", "name": "weather" } }
                }
            })),
            aws_eventstream_frame(json!({
                "contentBlockDelta": {
                    "contentBlockIndex": 0,
                    "delta": { "partial_json": "{\"city\":\"Hangzhou\"}" }
                }
            })),
            aws_eventstream_frame(json!({
                "messageStop": { "stopReason": "tool_use" }
            })),
            aws_eventstream_frame(json!({
                "metadata": { "usage": { "inputTokens": 8, "outputTokens": 5, "totalTokens": 13 } }
            })),
        ];

        let chunks: Vec<Result<Bytes, rquest::Error>> = frames
            .into_iter()
            .map(|frame| Ok(Bytes::from(frame)))
            .collect();
        let mut translated = Box::pin(translate_anthropic_like_stream_to_openai(
            futures::stream::iter(chunks),
            "bedrock-claude".to_string(),
        ));

        let mut collected = Vec::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push(String::from_utf8(bytes.to_vec()).unwrap());
        }

        let full_output = collected.join("");
        assert!(full_output.contains("\"tool_calls\""));
        assert!(full_output.contains("\"weather\""));
        assert!(
            full_output.contains("\"tool_calls\"")
                || full_output.contains("\"finish_reason\":\"tool_calls\"")
        );
    }

    #[tokio::test]
    async fn translate_stream_synthesizes_done_on_eof_after_finish() {
        use futures::StreamExt;

        let chunks = vec![
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{"content":"Hello"},"index":0}],"object":"chat.completion.chunk","created":1,"model":"MiniMax-M2.5","id":"chatcmpl-test"}"#,
            )),
            Ok(Bytes::from_static(
                br#"data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}],"object":"chat.completion.chunk","created":1,"model":"MiniMax-M2.5","id":"chatcmpl-test"}"#,
            )),
        ];

        let mut translated = Box::pin(translate_anthropic_like_stream_to_openai(
            futures::stream::iter(chunks),
            "MiniMax-M2.5".to_string(),
        ));

        let mut collected = String::new();
        while let Some(Ok(bytes)) = translated.next().await {
            collected.push_str(&String::from_utf8(bytes.to_vec()).unwrap());
        }

        assert!(collected.contains("\"content\":\"Hello\""));
        assert!(collected.contains("\"finish_reason\":\"stop\""));
        assert!(collected.contains("data: [DONE]"));
    }

    #[test]
    fn translate_text_delta_to_openai_chunk() {
        let line = br#"data:{"partial":true,"raw_response_json":"{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}"}"#;
        let output = String::from_utf8(
            translate_accio_sse_to_openai(line, "claude-sonnet-4-6", "chatcmpl-test", 1700000000)
                .unwrap(),
        )
        .unwrap();
        let payload: Value =
            serde_json::from_str(output.trim_start_matches("data: ").trim()).unwrap();
        assert_eq!(payload["choices"][0]["delta"]["content"], "Hello");
    }

    #[test]
    fn unpack_rejects_accio_provider_error_payload() {
        let body = json!({
            "turn_complete": true,
            "error_code": "5015",
            "error_message": "user not activated"
        });

        let err = unpack_accio_response(&body).unwrap_err();
        assert_eq!(err.kind, ErrorKind::ServiceUnavailable);
        assert_eq!(err.http_status, Some(503));
        assert_eq!(err.code.as_deref(), Some("5015"));
        assert_eq!(err.provider_name.as_deref(), Some("accio_compatible"));
        assert!(matches!(
            err.fallback_hint,
            FallbackHint::FallbackProvider { .. }
        ));
        assert!(err.message.contains("Accio account unavailable"));
    }

    #[test]
    fn detect_provider_error_from_wrapped_sse_line() {
        let line = br#"data:{"turn_complete":true,"error_code":"5015","error_message":"user not activated"}"#;

        let err = detect_accio_provider_error(line).unwrap();
        assert_eq!(err.kind, ErrorKind::ServiceUnavailable);
        assert_eq!(err.http_status, Some(503));
        assert_eq!(err.code.as_deref(), Some("5015"));
        assert_eq!(err.provider_name.as_deref(), Some("accio_compatible"));
        assert!(matches!(
            err.fallback_hint,
            FallbackHint::FallbackProvider { .. }
        ));
    }
}
