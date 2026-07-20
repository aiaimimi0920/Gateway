use std::collections::{HashMap, VecDeque};

use base64::Engine;
use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::Stream;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, CanonicalRelayResponse, CanonicalTool,
    CanonicalToolCall, ContentPart, MessageRole, TokenUsage,
};

pub const KIRO_DEFAULT_MODEL: &str = "claude-sonnet-4.6";
pub const KIRO_GENERATE_ASSISTANT_RESPONSE_PATH: &str = "/generateAssistantResponse";
pub const KIRO_DEFAULT_VERSION: &str = "0.11.107";
pub const KIRO_DEFAULT_SYSTEM_VERSION: &str = "win32#10.0.22631";
pub const KIRO_DEFAULT_NODE_VERSION: &str = "22.22.0";

const TOOL_NAME_MAX_LEN: usize = 63;
const EVENT_STREAM_CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Debug, Clone)]
enum KiroEvent {
    AssistantResponse {
        content: String,
    },
    ToolUse {
        name: String,
        tool_use_id: String,
        input: String,
        stop: bool,
    },
    ContextUsage {
        context_usage_percentage: f64,
    },
    Error {
        error_code: String,
        error_message: String,
    },
    Exception {
        exception_type: String,
        message: String,
    },
    Unknown,
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    announced: bool,
    index: usize,
}

#[derive(Debug)]
struct TranslatorState {
    response_id: String,
    anthropic_message_id: String,
    created: i64,
    model: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    context_overflow: bool,
    message_started: bool,
    open_text_index: Option<usize>,
    next_block_index: usize,
    tool_name_map: HashMap<String, String>,
    pending_tools: HashMap<String, PendingToolCall>,
    tool_order: Vec<String>,
    tool_calls_seen: bool,
    outputs: VecDeque<Vec<u8>>,
}

#[derive(Debug, Default)]
struct EventStreamParser {
    buffer: Vec<u8>,
}

#[derive(Debug)]
struct EventStreamFrame {
    headers: HashMap<String, HeaderValue>,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
enum HeaderValue {
    Bool,
    String(String),
    Int,
    Bytes,
    Timestamp,
    Uuid,
}

impl HeaderValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct ExtractedUserParts {
    text: String,
    images: Vec<Value>,
}

pub fn pack_kiro(req: &CanonicalRelayRequest, model: &str) -> Result<Value, GatewayError> {
    let requested_model = map_model(model);
    let tool_name_map = build_tool_name_map(req);
    let mut tools = req
        .tools
        .iter()
        .map(|tool| pack_tool(tool, &tool_name_map))
        .collect::<Vec<_>>();

    let mut messages: Vec<&CanonicalMessage> = req
        .messages
        .iter()
        .filter(|message| message.role != MessageRole::System)
        .collect();
    while matches!(messages.last(), Some(message) if message.role == MessageRole::Assistant) {
        messages.pop();
    }

    if messages.is_empty() {
        return Err(GatewayError::bad_request(
            "Kiro requests require at least one non-system user/tool turn",
        )
        .with_code("kiro_missing_messages"));
    }

    let current_start = messages
        .iter()
        .rposition(|message| matches!(message.role, MessageRole::Assistant))
        .map(|index| index + 1)
        .unwrap_or(0);
    let (history_messages, current_messages) = messages.split_at(current_start);
    if current_messages.is_empty()
        || current_messages
            .iter()
            .any(|message| !matches!(message.role, MessageRole::User | MessageRole::Tool))
    {
        return Err(GatewayError::bad_request(
            "Kiro requires a trailing user/tool turn to build currentMessage",
        )
        .with_code("kiro_invalid_current_message"));
    }

    let mut history =
        build_history_messages(req, history_messages, &requested_model, &tool_name_map)?;
    let mut current = build_user_message(current_messages, &requested_model)?;
    ensure_history_tools_declared(&history, &mut tools, &tool_name_map);
    if !tools.is_empty() {
        let context = current
            .as_object_mut()
            .expect("kiro current message must be object")
            .entry("userInputMessageContext".to_string())
            .or_insert_with(|| json!({}));
        if let Some(context_object) = context.as_object_mut() {
            context_object.insert("tools".to_string(), Value::Array(tools));
        }
    }
    validate_tool_pairing(&mut history, &mut current);

    let mut conversation_state = json!({
        "conversationId": resolve_conversation_id(req),
        "agentContinuationId": uuid::Uuid::new_v4().to_string(),
        "agentTaskType": "vibe",
        "chatTriggerType": "MANUAL",
        "currentMessage": {
            "userInputMessage": current,
        },
        "history": history,
    });

    if let Some(object) = conversation_state.as_object_mut() {
        for (key, value) in &req.extra {
            if is_handled_request_extra_key(key) {
                continue;
            }
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    Ok(json!({
        "conversationState": conversation_state,
    }))
}

pub fn map_model(model: &str) -> String {
    let lowered = model.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return KIRO_DEFAULT_MODEL.to_string();
    }
    if lowered.contains("haiku") {
        return "claude-haiku-4.5".to_string();
    }
    if lowered.contains("sonnet") {
        return if contains_version(&lowered, "4.6") {
            "claude-sonnet-4.6".to_string()
        } else {
            "claude-sonnet-4.5".to_string()
        };
    }
    if lowered.contains("opus") {
        return if contains_version(&lowered, "4.5") {
            "claude-opus-4.5".to_string()
        } else {
            "claude-opus-4.6".to_string()
        };
    }
    model.trim().to_string()
}

pub fn context_window_for_model(model: &str) -> u64 {
    let lowered = model.to_ascii_lowercase();
    let extended = contains_version(&lowered, "4.6")
        && (lowered.contains("sonnet") || lowered.contains("opus"));
    if extended {
        1_000_000
    } else {
        200_000
    }
}

pub fn is_reserved_payload_extra_key(key: &str) -> bool {
    matches!(
        normalize_lookup_key(key).as_str(),
        "kirorefreshtoken"
            | "kiroaccesstoken"
            | "kiroauthmethod"
            | "kiroclientid"
            | "kiroclientsecret"
            | "kiroauthregion"
            | "kiroapiregion"
            | "kiromachineid"
            | "kiroversion"
            | "kirosystemversion"
            | "kironodeversion"
            | "kiroprofilearn"
    )
}

pub fn read_payload_string(
    extra: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<String> {
    let extra = extra?;
    let alias_keys: Vec<String> = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect();
    extra.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }
    })
}

pub fn normalize_machine_id(machine_id: &str) -> Option<String> {
    let trimmed = machine_id.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(trimmed.to_ascii_lowercase());
    }
    let without_dashes: String = trimmed.chars().filter(|c| *c != '-').collect();
    if without_dashes.len() == 32 && without_dashes.chars().all(|c| c.is_ascii_hexdigit()) {
        let normalized = without_dashes.to_ascii_lowercase();
        return Some(format!("{normalized}{normalized}"));
    }
    None
}

pub fn generate_machine_id(
    explicit_machine_id: Option<&str>,
    refresh_token: Option<&str>,
) -> Option<String> {
    if let Some(machine_id) = explicit_machine_id.and_then(normalize_machine_id) {
        return Some(machine_id);
    }
    let refresh_token = refresh_token?.trim();
    if refresh_token.is_empty() {
        return None;
    }
    Some(sha256_hex(&format!("KotlinNativeAPI/{refresh_token}")))
}

fn pack_tool(tool: &CanonicalTool, tool_name_map: &HashMap<String, String>) -> Value {
    let original_name = tool.name.clone().unwrap_or_default();
    let name = tool_name_map
        .iter()
        .find_map(|(short, original)| {
            if original == &original_name {
                Some(short.clone())
            } else {
                None
            }
        })
        .unwrap_or(original_name);
    json!({
        "toolSpecification": {
            "name": name,
            "description": tool.description.clone().unwrap_or_default(),
            "inputSchema": {
                "json": tool.input_schema.clone().unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }
        }
    })
}

fn build_history_messages(
    req: &CanonicalRelayRequest,
    messages: &[&CanonicalMessage],
    model: &str,
    tool_name_map: &HashMap<String, String>,
) -> Result<Vec<Value>, GatewayError> {
    let mut history = Vec::new();
    if let Some(system_message) = req.system_message().filter(|text| !text.trim().is_empty()) {
        history.push(json!({
            "userInputMessage": {
                "content": system_message,
                "modelId": model,
                "origin": "AI_EDITOR",
            }
        }));
        history.push(json!({
            "assistantResponseMessage": {
                "content": "I will follow these instructions."
            }
        }));
    }

    let mut index = 0usize;
    while index < messages.len() {
        match messages[index].role {
            MessageRole::User | MessageRole::Tool => {
                let start = index;
                while index < messages.len()
                    && matches!(messages[index].role, MessageRole::User | MessageRole::Tool)
                {
                    index += 1;
                }
                history.push(json!({ "userInputMessage": build_user_message(&messages[start..index], model)? }));
            }
            MessageRole::Assistant => {
                let start = index;
                while index < messages.len() && messages[index].role == MessageRole::Assistant {
                    index += 1;
                }
                history.push(json!({
                    "assistantResponseMessage": build_assistant_message(&messages[start..index], tool_name_map)
                }));
            }
            MessageRole::System => {
                index += 1;
            }
        }
    }
    Ok(history)
}

fn build_user_message(messages: &[&CanonicalMessage], model: &str) -> Result<Value, GatewayError> {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();
    let mut tool_results = Vec::new();

    for message in messages {
        match message.role {
            MessageRole::User => {
                let extracted = extract_text_and_images(&message.content)?;
                if !extracted.text.is_empty() {
                    text_parts.push(extracted.text);
                }
                images.extend(extracted.images);
            }
            MessageRole::Tool => {
                let Some(tool_call_id) = message.tool_call_id.as_deref() else {
                    continue;
                };
                tool_results.push(json!({
                    "toolUseId": tool_call_id,
                    "content": [{ "text": message_text(message) }],
                    "status": "success",
                }));
            }
            _ => {}
        }
    }

    let content = {
        let joined = text_parts.join("\n");
        if joined.trim().is_empty() && !tool_results.is_empty() {
            " ".to_string()
        } else {
            joined
        }
    };

    let mut object = Map::new();
    object.insert("content".to_string(), Value::String(content));
    object.insert("modelId".to_string(), Value::String(model.to_string()));
    object.insert("origin".to_string(), Value::String("AI_EDITOR".to_string()));
    if !images.is_empty() {
        object.insert("images".to_string(), Value::Array(images));
    }
    if !tool_results.is_empty() {
        object.insert(
            "userInputMessageContext".to_string(),
            json!({ "toolResults": tool_results }),
        );
    }
    Ok(Value::Object(object))
}

fn build_assistant_message(
    messages: &[&CanonicalMessage],
    tool_name_map: &HashMap<String, String>,
) -> Value {
    let mut content_parts = Vec::new();
    let mut tool_uses = Vec::new();

    for message in messages {
        let text = message_text(message);
        if !text.is_empty() {
            content_parts.push(text);
        }
        for tool_call in &message.tool_calls {
            let name = tool_call.name.clone().unwrap_or_default();
            let mapped_name = tool_name_map
                .iter()
                .find_map(|(short, original)| {
                    if original == &name {
                        Some(short.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or(name);
            let input = tool_call
                .arguments
                .as_deref()
                .and_then(|arguments| serde_json::from_str::<Value>(arguments).ok())
                .unwrap_or_else(|| json!({}));
            tool_uses.push(json!({
                "toolUseId": tool_call.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                "name": mapped_name,
                "input": input,
            }));
        }
    }

    let content = {
        let joined = content_parts.join("\n\n");
        if joined.trim().is_empty() && !tool_uses.is_empty() {
            " ".to_string()
        } else {
            joined
        }
    };

    let mut object = Map::new();
    object.insert("content".to_string(), Value::String(content));
    if !tool_uses.is_empty() {
        object.insert("toolUses".to_string(), Value::Array(tool_uses));
    }
    Value::Object(object)
}

fn ensure_history_tools_declared(
    history: &[Value],
    tools: &mut Vec<Value>,
    tool_name_map: &HashMap<String, String>,
) {
    let mut declared = tools
        .iter()
        .filter_map(|tool| {
            tool.get("toolSpecification")
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    for entry in history {
        let Some(tool_uses) = entry
            .get("assistantResponseMessage")
            .and_then(|value| value.get("toolUses"))
            .and_then(|value| value.as_array())
        else {
            continue;
        };
        for tool_use in tool_uses {
            let Some(name) = tool_use.get("name").and_then(|value| value.as_str()) else {
                continue;
            };
            if declared
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(name))
            {
                continue;
            }
            declared.push(name.to_string());
            let description = restore_tool_name(tool_name_map, name);
            tools.push(json!({
                "toolSpecification": {
                    "name": name,
                    "description": format!("History tool placeholder for {description}"),
                    "inputSchema": { "json": {"type":"object","properties":{}} }
                }
            }));
        }
    }
}

fn validate_tool_pairing(history: &mut [Value], current: &mut Value) {
    let all_tool_use_ids = history
        .iter()
        .flat_map(|entry| {
            entry
                .get("assistantResponseMessage")
                .and_then(|value| value.get("toolUses"))
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|tool_use| {
            tool_use
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    let resolved_tool_result_ids = history
        .iter()
        .flat_map(|entry| {
            entry
                .get("userInputMessage")
                .and_then(|value| value.get("userInputMessageContext"))
                .and_then(|value| value.get("toolResults"))
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|tool_result| {
            tool_result
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    if let Some(results) = current
        .get_mut("userInputMessageContext")
        .and_then(|value| value.get_mut("toolResults"))
        .and_then(|value| value.as_array_mut())
    {
        results.retain(|tool_result| {
            tool_result
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .map(|tool_use_id| all_tool_use_ids.iter().any(|id| id == tool_use_id))
                .unwrap_or(false)
        });
    }

    let current_result_ids = current
        .get("userInputMessageContext")
        .and_then(|value| value.get("toolResults"))
        .and_then(|value| value.as_array())
        .map(|results| {
            results
                .iter()
                .filter_map(|tool_result| {
                    tool_result
                        .get("toolUseId")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for entry in history.iter_mut() {
        let Some(tool_uses) = entry
            .get_mut("assistantResponseMessage")
            .and_then(|value| value.get_mut("toolUses"))
            .and_then(|value| value.as_array_mut())
        else {
            continue;
        };
        tool_uses.retain(|tool_use| {
            let Some(tool_use_id) = tool_use.get("toolUseId").and_then(|value| value.as_str())
            else {
                return false;
            };
            resolved_tool_result_ids.iter().any(|id| id == tool_use_id)
                || current_result_ids.iter().any(|id| id == tool_use_id)
        });
    }
}

fn build_tool_name_map(req: &CanonicalRelayRequest) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for tool in &req.tools {
        let Some(name) = tool.name.as_deref() else {
            continue;
        };
        if let Some(short_name) = maybe_shortened_tool_name(name) {
            map.insert(short_name, name.to_string());
        }
    }
    map
}

fn maybe_shortened_tool_name(name: &str) -> Option<String> {
    if name.chars().count() <= TOOL_NAME_MAX_LEN {
        return None;
    }
    let hash = sha256_hex(name);
    let suffix = &hash[..8];
    let prefix_max = TOOL_NAME_MAX_LEN.saturating_sub(1 + suffix.len());
    let mut prefix = String::new();
    for ch in name.chars().take(prefix_max) {
        prefix.push(ch);
    }
    Some(format!("{prefix}_{suffix}"))
}

fn extract_text_and_images(parts: &[ContentPart]) -> Result<ExtractedUserParts, GatewayError> {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    for part in parts {
        match part {
            ContentPart::Text { text } => {
                if !text.is_empty() {
                    text_parts.push(text.clone());
                }
            }
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                let serialized = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
                if !serialized.is_empty() {
                    text_parts.push(serialized);
                }
            }
            ContentPart::ImageUrl { image_url, .. } => {
                if let Some(image) = pack_image_url_as_kiro_image(image_url)? {
                    images.push(image);
                }
            }
        }
    }

    Ok(ExtractedUserParts {
        text: text_parts.join("\n"),
        images,
    })
}

fn pack_image_url_as_kiro_image(image_url: &str) -> Result<Option<Value>, GatewayError> {
    let Some(rest) = image_url.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((meta, bytes)) = rest.split_once(',') else {
        return Ok(None);
    };
    if !meta.to_ascii_lowercase().contains(";base64") {
        return Ok(None);
    }

    let format = match meta.split(';').next().unwrap_or("image/png") {
        "image/jpeg" => "jpeg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        other => {
            return Err(GatewayError::bad_request(format!(
                "Unsupported Kiro image media type: {other}"
            ))
            .with_code("kiro_unsupported_image_media_type"))
        }
    };

    let decoded = urlencoding_decode(bytes);
    let data = base64::engine::general_purpose::STANDARD
        .decode(decoded.as_bytes())
        .map_err(|error| {
            GatewayError::bad_request(format!("Invalid Kiro image base64 payload: {error}"))
                .with_code("kiro_invalid_image_base64")
        })?;
    let base64_bytes = base64::engine::general_purpose::STANDARD.encode(data);

    Ok(Some(json!({
        "format": format,
        "source": {
            "bytes": base64_bytes,
        }
    })))
}

fn message_text(message: &CanonicalMessage) -> String {
    let mut parts = Vec::new();
    for part in &message.content {
        match part {
            ContentPart::Text { text } => parts.push(text.clone()),
            ContentPart::Json { value } | ContentPart::Raw { value } => {
                parts.push(serde_json::to_string(value).unwrap_or_else(|_| value.to_string()));
            }
            ContentPart::ImageUrl { image_url, .. } => parts.push(image_url.clone()),
        }
    }
    parts.join("\n")
}

fn resolve_conversation_id(req: &CanonicalRelayRequest) -> String {
    req.explicit_session_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            req.metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user_id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn is_handled_request_extra_key(key: &str) -> bool {
    matches!(
        normalize_lookup_key(key).as_str(),
        "model"
            | "messages"
            | "stream"
            | "tools"
            | "toolchoice"
            | "user"
            | "reasoning"
            | "thinking"
            | "metadata"
            | "maxtokens"
            | "maxcompletiontokens"
            | "temperature"
            | "topp"
            | "stop"
    )
}

fn contains_version(value: &str, version: &str) -> bool {
    let dotted = version.to_ascii_lowercase();
    let dashed = dotted.replace('.', "-");
    value.contains(&dotted) || value.contains(&dashed)
}

fn normalize_lookup_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '_' && *ch != '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn prompt_tokens_from_context(model: &str, context_usage_percentage: f64) -> u64 {
    let context = context_window_for_model(model) as f64;
    ((context_usage_percentage.max(0.0) / 100.0) * context).round() as u64
}

fn estimate_request_prompt_tokens(req: &CanonicalRelayRequest, model: &str) -> u64 {
    let estimated = estimate_tokens(&req.messages_text());
    if estimated > 0 {
        estimated
    } else {
        prompt_tokens_from_context(model, 0.0)
    }
}

fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        0
    } else {
        ((text.chars().count() as u64) + 3) / 4
    }
}

fn normalize_tool_arguments(arguments: &str) -> String {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        "{}".to_string()
    } else {
        trimmed.to_string()
    }
}

fn restore_tool_name(tool_name_map: &HashMap<String, String>, name: &str) -> String {
    tool_name_map
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn urlencoding_decode(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(parsed) = u8::from_str_radix(hex, 16) {
                    out.push(parsed);
                    index += 3;
                    continue;
                }
                out.push(b'%');
                index += 1;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn classify_kiro_provider_error(code: &str, message: &str) -> GatewayError {
    let lowered = message.to_ascii_lowercase();
    if lowered.contains("monthly_request_count") {
        return GatewayError::quota_exceeded(format!(
            "Kiro monthly request quota exceeded: {message}"
        ))
        .with_provider("kiro_compatible")
        .with_code(code.to_string());
    }
    if lowered.contains("bearer token") || lowered.contains("unauthorized") {
        return GatewayError::unauthorized(format!("Kiro authorization failed: {message}"))
            .with_provider("kiro_compatible")
            .with_code(code.to_string());
    }
    GatewayError::server_error(format!("Kiro upstream error: {message}"))
        .with_provider("kiro_compatible")
        .with_code(code.to_string())
}

pub async fn accumulate_kiro_stream(
    response: rquest::Response,
    model: &str,
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayResponse, GatewayError> {
    use futures::StreamExt;

    let tool_name_map = build_tool_name_map(req);
    let mut parser = EventStreamParser::default();
    let mut text = String::new();
    let mut prompt_tokens = estimate_request_prompt_tokens(req, model);
    let mut completion_tokens = 0u64;
    let mut context_overflow = false;
    let mut tool_calls_seen = false;
    let mut pending_tools: HashMap<String, PendingToolCall> = HashMap::new();
    let mut tool_order: Vec<String> = Vec::new();

    let mut stream = Box::pin(response.bytes_stream());
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| GatewayError::server_error(format!("read kiro stream: {e}")))?;
        let events = parser.feed(&chunk)?;
        for event in events {
            match event {
                KiroEvent::AssistantResponse { content } => {
                    completion_tokens = completion_tokens.saturating_add(estimate_tokens(&content));
                    text.push_str(&content);
                }
                KiroEvent::ToolUse {
                    name,
                    tool_use_id,
                    input,
                    ..
                } => {
                    tool_calls_seen = true;
                    completion_tokens = completion_tokens.saturating_add(estimate_tokens(&input));
                    let entry = pending_tools.entry(tool_use_id.clone()).or_insert_with(|| {
                        tool_order.push(tool_use_id.clone());
                        PendingToolCall {
                            id: tool_use_id.clone(),
                            name: restore_tool_name(&tool_name_map, &name),
                            arguments: String::new(),
                            announced: false,
                            index: tool_order.len().saturating_sub(1),
                        }
                    });
                    if !input.is_empty() {
                        entry.arguments.push_str(&input);
                    }
                }
                KiroEvent::ContextUsage {
                    context_usage_percentage,
                } => {
                    prompt_tokens = prompt_tokens_from_context(model, context_usage_percentage)
                        .max(prompt_tokens);
                    if context_usage_percentage >= 100.0 {
                        context_overflow = true;
                    }
                }
                KiroEvent::Error {
                    error_code,
                    error_message,
                } => {
                    return Err(classify_kiro_provider_error(&error_code, &error_message));
                }
                KiroEvent::Exception {
                    exception_type,
                    message,
                } => {
                    if exception_type == "ContentLengthExceededException" {
                        context_overflow = true;
                    } else {
                        return Err(GatewayError::server_error(format!(
                            "Kiro upstream exception: {exception_type}: {message}"
                        ))
                        .with_provider("kiro_compatible")
                        .with_code(exception_type));
                    }
                }
                KiroEvent::Unknown => {}
            }
        }
    }

    let tool_calls = tool_order
        .into_iter()
        .filter_map(|id| pending_tools.remove(&id))
        .map(|call| CanonicalToolCall {
            id: Some(call.id),
            call_type: "function".to_string(),
            name: Some(call.name),
            arguments: Some(normalize_tool_arguments(&call.arguments)),
            raw: HashMap::new(),
        })
        .collect::<Vec<_>>();

    let finish_reason = if !tool_calls.is_empty() || tool_calls_seen {
        Some("tool_calls".to_string())
    } else if context_overflow {
        Some("length".to_string())
    } else {
        Some("stop".to_string())
    };

    Ok(CanonicalRelayResponse {
        model: map_model(model),
        text,
        usage: Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }),
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}

pub fn translate_kiro_event_stream_to_openai_sse(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_kiro_stream(inner, build_translator_state(model, req), false)
}

pub fn translate_kiro_event_stream_to_anthropic_sse(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_kiro_stream(inner, build_translator_state(model, req), true)
}

fn build_translator_state(model: String, req: CanonicalRelayRequest) -> TranslatorState {
    TranslatorState {
        response_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        anthropic_message_id: format!("msg_{}", uuid::Uuid::new_v4().as_simple()),
        created: now_unix_seconds(),
        model: map_model(&model),
        prompt_tokens: estimate_request_prompt_tokens(&req, &model),
        completion_tokens: 0,
        context_overflow: false,
        message_started: false,
        open_text_index: None,
        next_block_index: 0,
        tool_name_map: build_tool_name_map(&req),
        pending_tools: HashMap::new(),
        tool_order: Vec::new(),
        tool_calls_seen: false,
        outputs: VecDeque::new(),
    }
}

fn translate_kiro_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            EventStreamParser::default(),
            state,
            anthropic,
            false,
        ),
        |(mut stream, mut parser, mut state, anthropic, done)| async move {
            use futures::StreamExt;
            if done {
                return None;
            }

            loop {
                if let Some(output) = state.outputs.pop_front() {
                    let finished = state.outputs.is_empty();
                    return Some((
                        Ok(Bytes::from(output)),
                        (stream, parser, state, anthropic, finished),
                    ));
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let events = match parser.feed(&chunk) {
                            Ok(events) => events,
                            Err(error) => {
                                if anthropic {
                                    state.outputs.push_back(anthropic_event_bytes(
                                        "error",
                                        json!({
                                            "type": "error",
                                            "error": {
                                                "type": "api_error",
                                                "message": error.message,
                                            }
                                        }),
                                    ));
                                    push_anthropic_finish(&mut state);
                                } else {
                                    state.outputs.push_back(
                                        format!(
                                            "data: {}\n\n",
                                            json!({
                                                "error": {
                                                    "message": error.message,
                                                }
                                            })
                                        )
                                        .into_bytes(),
                                    );
                                    push_openai_finish(&mut state);
                                }
                                continue;
                            }
                        };
                        for event in events {
                            if anthropic {
                                push_anthropic_events(&mut state, event);
                            } else {
                                push_openai_events(&mut state, event);
                            }
                        }
                    }
                    Some(Err(error)) => {
                        return Some((Err(error), (stream, parser, state, anthropic, true)))
                    }
                    None => {
                        if anthropic {
                            push_anthropic_finish(&mut state);
                        } else {
                            push_openai_finish(&mut state);
                        }
                        if let Some(output) = state.outputs.pop_front() {
                            let finished = state.outputs.is_empty();
                            return Some((
                                Ok(Bytes::from(output)),
                                (stream, parser, state, anthropic, finished),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

fn push_openai_events(state: &mut TranslatorState, event: KiroEvent) {
    match event {
        KiroEvent::AssistantResponse { content } => {
            if content.is_empty() {
                return;
            }
            state.completion_tokens = state
                .completion_tokens
                .saturating_add(estimate_tokens(&content));
            state.outputs.push_back(
                format!(
                    "data: {}\n\n",
                    json!({
                        "id": state.response_id,
                        "object": "chat.completion.chunk",
                        "created": state.created,
                        "model": state.model,
                        "choices": [{
                            "index": 0,
                            "delta": { "content": content },
                            "finish_reason": Value::Null,
                        }],
                    })
                )
                .into_bytes(),
            );
        }
        KiroEvent::ToolUse {
            name,
            tool_use_id,
            input,
            stop,
        } => {
            state.tool_calls_seen = true;
            let tool_position = next_openai_tool_index(state, &tool_use_id);
            if !state.pending_tools.contains_key(&tool_use_id) {
                state.tool_order.push(tool_use_id.clone());
                state.pending_tools.insert(
                    tool_use_id.clone(),
                    PendingToolCall {
                        id: tool_use_id.clone(),
                        name: restore_tool_name(&state.tool_name_map, &name),
                        arguments: String::new(),
                        announced: false,
                        index: tool_position,
                    },
                );
            }
            let entry = state
                .pending_tools
                .get_mut(&tool_use_id)
                .expect("tool exists");
            if !input.is_empty() {
                state.completion_tokens = state
                    .completion_tokens
                    .saturating_add(estimate_tokens(&input));
                entry.arguments.push_str(&input);
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({
                            "id": state.response_id,
                            "object": "chat.completion.chunk",
                            "created": state.created,
                            "model": state.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": tool_position,
                                        "id": if entry.announced { Value::Null } else { json!(entry.id.clone()) },
                                        "type": if entry.announced { Value::Null } else { json!("function") },
                                        "function": {
                                            "name": if entry.announced { Value::Null } else { json!(entry.name.clone()) },
                                            "arguments": input,
                                        }
                                    }]
                                },
                                "finish_reason": Value::Null,
                            }],
                        })
                    )
                    .into_bytes(),
                );
                entry.announced = true;
            } else if stop && !entry.announced {
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({
                            "id": state.response_id,
                            "object": "chat.completion.chunk",
                            "created": state.created,
                            "model": state.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": tool_position,
                                        "id": entry.id.clone(),
                                        "type": "function",
                                        "function": {
                                            "name": entry.name.clone(),
                                            "arguments": "",
                                        }
                                    }]
                                },
                                "finish_reason": Value::Null,
                            }],
                        })
                    )
                    .into_bytes(),
                );
                entry.announced = true;
            }
        }
        KiroEvent::ContextUsage {
            context_usage_percentage,
        } => {
            state.prompt_tokens =
                prompt_tokens_from_context(&state.model, context_usage_percentage)
                    .max(state.prompt_tokens);
            if context_usage_percentage >= 100.0 {
                state.context_overflow = true;
            }
        }
        KiroEvent::Error {
            error_code,
            error_message,
        } => {
            state.outputs.push_back(
                format!(
                    "data: {}\n\n",
                    json!({ "error": { "message": error_message, "code": error_code } })
                )
                .into_bytes(),
            );
        }
        KiroEvent::Exception {
            exception_type,
            message,
        } => {
            if exception_type == "ContentLengthExceededException" {
                state.context_overflow = true;
            } else {
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({ "error": { "message": message, "code": exception_type } })
                    )
                    .into_bytes(),
                );
            }
        }
        KiroEvent::Unknown => {}
    }
}

fn push_openai_finish(state: &mut TranslatorState) {
    let finish_reason = if state.tool_calls_seen {
        "tool_calls"
    } else if state.context_overflow {
        "length"
    } else {
        "stop"
    };
    state.outputs.push_back(
        format!(
            "data: {}\n\n",
            json!({
                "id": state.response_id,
                "object": "chat.completion.chunk",
                "created": state.created,
                "model": state.model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason,
                }],
                "usage": {
                    "prompt_tokens": state.prompt_tokens,
                    "completion_tokens": state.completion_tokens,
                    "total_tokens": state.prompt_tokens.saturating_add(state.completion_tokens),
                }
            })
        )
        .into_bytes(),
    );
    state.outputs.push_back(b"data: [DONE]\n\n".to_vec());
}

fn push_anthropic_events(state: &mut TranslatorState, event: KiroEvent) {
    ensure_anthropic_message_started(state);
    match event {
        KiroEvent::AssistantResponse { content } => {
            if content.is_empty() {
                return;
            }
            state.completion_tokens = state
                .completion_tokens
                .saturating_add(estimate_tokens(&content));
            let index = open_or_create_text_block(state);
            state.outputs.push_back(anthropic_event_bytes(
                "content_block_delta",
                json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "text_delta",
                        "text": content,
                    }
                }),
            ));
        }
        KiroEvent::ToolUse {
            name,
            tool_use_id,
            input,
            stop,
        } => {
            state.tool_calls_seen = true;
            if let Some(index) = state.open_text_index.take() {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_stop",
                    json!({ "type": "content_block_stop", "index": index }),
                ));
            }
            let tool_position = next_anthropic_block_index(state, &tool_use_id);
            if !state.pending_tools.contains_key(&tool_use_id) {
                state.pending_tools.insert(
                    tool_use_id.clone(),
                    PendingToolCall {
                        id: tool_use_id.clone(),
                        name: restore_tool_name(&state.tool_name_map, &name),
                        arguments: String::new(),
                        announced: false,
                        index: tool_position,
                    },
                );
            }
            let entry = state
                .pending_tools
                .get_mut(&tool_use_id)
                .expect("tool exists");
            if !entry.announced {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_start",
                    json!({
                        "type": "content_block_start",
                        "index": tool_position,
                        "content_block": {
                            "type": "tool_use",
                            "id": entry.id.clone(),
                            "name": entry.name.clone(),
                            "input": {},
                        }
                    }),
                ));
                entry.announced = true;
            }
            if !input.is_empty() {
                state.completion_tokens = state
                    .completion_tokens
                    .saturating_add(estimate_tokens(&input));
                entry.arguments.push_str(&input);
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_delta",
                    json!({
                        "type": "content_block_delta",
                        "index": tool_position,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": input,
                        }
                    }),
                ));
            }
            if stop {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_stop",
                    json!({ "type": "content_block_stop", "index": tool_position }),
                ));
            }
        }
        KiroEvent::ContextUsage {
            context_usage_percentage,
        } => {
            state.prompt_tokens =
                prompt_tokens_from_context(&state.model, context_usage_percentage)
                    .max(state.prompt_tokens);
            if context_usage_percentage >= 100.0 {
                state.context_overflow = true;
            }
        }
        KiroEvent::Error {
            error_code,
            error_message,
        } => {
            state.outputs.push_back(anthropic_event_bytes(
                "error",
                json!({
                    "type": "error",
                    "error": {
                        "type": "api_error",
                        "message": format!("{error_code}: {error_message}"),
                    }
                }),
            ));
        }
        KiroEvent::Exception {
            exception_type,
            message,
        } => {
            if exception_type == "ContentLengthExceededException" {
                state.context_overflow = true;
            } else {
                state.outputs.push_back(anthropic_event_bytes(
                    "error",
                    json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": format!("{exception_type}: {message}"),
                        }
                    }),
                ));
            }
        }
        KiroEvent::Unknown => {}
    }
}

fn push_anthropic_finish(state: &mut TranslatorState) {
    ensure_anthropic_message_started(state);
    if let Some(index) = state.open_text_index.take() {
        state.outputs.push_back(anthropic_event_bytes(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": index }),
        ));
    }
    let stop_reason = if state.tool_calls_seen {
        "tool_use"
    } else if state.context_overflow {
        "max_tokens"
    } else {
        "end_turn"
    };
    state.outputs.push_back(anthropic_event_bytes(
        "message_delta",
        json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": Value::Null,
            },
            "usage": {
                "input_tokens": state.prompt_tokens,
                "output_tokens": state.completion_tokens,
            }
        }),
    ));
    state.outputs.push_back(anthropic_event_bytes(
        "message_stop",
        json!({ "type": "message_stop" }),
    ));
}

fn ensure_anthropic_message_started(state: &mut TranslatorState) {
    if state.message_started {
        return;
    }
    state.message_started = true;
    state.outputs.push_back(anthropic_event_bytes(
        "message_start",
        json!({
            "type": "message_start",
            "message": {
                "id": state.anthropic_message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": state.model,
                "stop_reason": Value::Null,
                "stop_sequence": Value::Null,
                "usage": {
                    "input_tokens": state.prompt_tokens,
                    "output_tokens": 0,
                }
            }
        }),
    ));
}

fn open_or_create_text_block(state: &mut TranslatorState) -> usize {
    if let Some(index) = state.open_text_index {
        return index;
    }
    let index = state.next_block_index;
    state.next_block_index += 1;
    state.open_text_index = Some(index);
    state.outputs.push_back(anthropic_event_bytes(
        "content_block_start",
        json!({
            "type": "content_block_start",
            "index": index,
            "content_block": {
                "type": "text",
                "text": "",
            }
        }),
    ));
    index
}

fn next_openai_tool_index(state: &TranslatorState, tool_use_id: &str) -> usize {
    state
        .pending_tools
        .get(tool_use_id)
        .map(|entry| entry.index)
        .or_else(|| {
            state
                .tool_order
                .iter()
                .position(|existing| existing == tool_use_id)
        })
        .unwrap_or(state.tool_order.len())
}

fn next_anthropic_block_index(state: &mut TranslatorState, tool_use_id: &str) -> usize {
    if let Some(index) = state
        .pending_tools
        .get(tool_use_id)
        .map(|entry| entry.index)
    {
        return index;
    }
    let index = state.next_block_index;
    state.next_block_index += 1;
    index
}

fn anthropic_event_bytes(event: &str, data: Value) -> Vec<u8> {
    format!(
        "event: {event}\ndata: {}\n\n",
        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    )
    .into_bytes()
}

impl EventStreamParser {
    fn feed(&mut self, chunk: &[u8]) -> Result<Vec<KiroEvent>, GatewayError> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        loop {
            match parse_event_stream_frame(&self.buffer)? {
                None => break,
                Some((consumed, frame)) => {
                    self.buffer.drain(..consumed);
                    events.push(parse_kiro_event(frame));
                }
            }
        }
        Ok(events)
    }
}

fn parse_event_stream_frame(
    buffer: &[u8],
) -> Result<Option<(usize, EventStreamFrame)>, GatewayError> {
    if buffer.len() < 12 {
        return Ok(None);
    }

    let total_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let header_len = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
    let prelude_crc = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);

    if total_len < 16 || buffer.len() < total_len {
        return if buffer.len() < total_len {
            Ok(None)
        } else {
            Err(GatewayError::server_error("Invalid Kiro frame length")
                .with_code("kiro_invalid_frame_length"))
        };
    }

    if EVENT_STREAM_CRC.checksum(&buffer[..8]) != prelude_crc {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream prelude CRC")
                .with_code("kiro_invalid_prelude_crc"),
        );
    }

    let message_crc = u32::from_be_bytes([
        buffer[total_len - 4],
        buffer[total_len - 3],
        buffer[total_len - 2],
        buffer[total_len - 1],
    ]);
    if EVENT_STREAM_CRC.checksum(&buffer[..total_len - 4]) != message_crc {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream message CRC")
                .with_code("kiro_invalid_message_crc"),
        );
    }

    let headers_start = 12;
    let headers_end = headers_start + header_len;
    if headers_end > total_len - 4 {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream header length")
                .with_code("kiro_invalid_header_length"),
        );
    }

    Ok(Some((
        total_len,
        EventStreamFrame {
            headers: parse_headers(&buffer[headers_start..headers_end])?,
            payload: buffer[headers_end..total_len - 4].to_vec(),
        },
    )))
}

fn parse_headers(buffer: &[u8]) -> Result<HashMap<String, HeaderValue>, GatewayError> {
    let mut headers = HashMap::new();
    let mut offset = 0usize;

    while offset < buffer.len() {
        let name_len = *buffer.get(offset).ok_or_else(|| {
            GatewayError::server_error("Invalid Kiro header name length")
                .with_code("kiro_invalid_header_name")
        })? as usize;
        offset += 1;
        if offset + name_len > buffer.len() {
            return Err(
                GatewayError::server_error("Invalid Kiro header name bounds")
                    .with_code("kiro_invalid_header_bounds"),
            );
        }
        let name = String::from_utf8_lossy(&buffer[offset..offset + name_len]).to_string();
        offset += name_len;

        let value_type = *buffer.get(offset).ok_or_else(|| {
            GatewayError::server_error("Invalid Kiro header type")
                .with_code("kiro_invalid_header_type")
        })?;
        offset += 1;

        let value = match value_type {
            0 | 1 => HeaderValue::Bool,
            2 => {
                offset += 1;
                HeaderValue::Int
            }
            3 => {
                offset += 2;
                HeaderValue::Int
            }
            4 => {
                offset += 4;
                HeaderValue::Int
            }
            5 => {
                offset += 8;
                HeaderValue::Int
            }
            6 => {
                let len = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]) as usize;
                offset += 2 + len;
                HeaderValue::Bytes
            }
            7 => {
                let len = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]) as usize;
                let start = offset + 2;
                let end = start + len;
                if end > buffer.len() {
                    return Err(
                        GatewayError::server_error("Invalid Kiro string header bounds")
                            .with_code("kiro_invalid_header_string_bounds"),
                    );
                }
                offset = end;
                HeaderValue::String(String::from_utf8_lossy(&buffer[start..end]).to_string())
            }
            8 => {
                offset += 8;
                HeaderValue::Timestamp
            }
            9 => {
                offset += 16;
                HeaderValue::Uuid
            }
            _ => {
                return Err(GatewayError::server_error("Unsupported Kiro header type")
                    .with_code("kiro_unsupported_header_type"))
            }
        };

        if offset > buffer.len() {
            return Err(GatewayError::server_error("Invalid Kiro header overflow")
                .with_code("kiro_invalid_header_overflow"));
        }
        headers.insert(name, value);
    }
    Ok(headers)
}

fn parse_kiro_event(frame: EventStreamFrame) -> KiroEvent {
    let message_type = frame
        .headers
        .get(":message-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("event");
    match message_type {
        "event" => parse_kiro_event_payload(frame),
        "error" => KiroEvent::Error {
            error_code: frame
                .headers
                .get(":error-code")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownError")
                .to_string(),
            error_message: String::from_utf8_lossy(&frame.payload).to_string(),
        },
        "exception" => KiroEvent::Exception {
            exception_type: frame
                .headers
                .get(":exception-type")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownException")
                .to_string(),
            message: String::from_utf8_lossy(&frame.payload).to_string(),
        },
        _ => KiroEvent::Unknown,
    }
}

fn parse_kiro_event_payload(frame: EventStreamFrame) -> KiroEvent {
    let event_type = frame
        .headers
        .get(":event-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("");
    let payload: Value = serde_json::from_slice(&frame.payload).unwrap_or_else(|_| json!({}));
    match event_type {
        "assistantResponseEvent" => KiroEvent::AssistantResponse {
            content: payload
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "toolUseEvent" => KiroEvent::ToolUse {
            name: payload
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            tool_use_id: payload
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            input: payload
                .get("input")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            stop: payload
                .get("stop")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        },
        "contextUsageEvent" => KiroEvent::ContextUsage {
            context_usage_percentage: payload
                .get("contextUsagePercentage")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0),
        },
        _ => KiroEvent::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_message(role: MessageRole, text: &str) -> CanonicalMessage {
        CanonicalMessage {
            role,
            content: vec![ContentPart::Text {
                text: text.to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    fn make_request(messages: Vec<CanonicalMessage>) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ChatCompletions,
            requested_model: Some("claude-sonnet-4-20250514".to_string()),
            stream: false,
            messages,
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: Some("session-123".to_string()),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn map_model_uses_kiro_claude_ids() {
        assert_eq!(map_model("claude-sonnet-4-20250514"), "claude-sonnet-4.5");
        assert_eq!(map_model("claude-sonnet-4.6"), "claude-sonnet-4.6");
        assert_eq!(map_model("claude-opus-4.5"), "claude-opus-4.5");
        assert_eq!(map_model("claude-haiku-4-20250514"), "claude-haiku-4.5");
    }

    #[test]
    fn reserved_payload_extra_keys_skip_refresh_material() {
        assert!(is_reserved_payload_extra_key("kiroRefreshToken"));
        assert!(is_reserved_payload_extra_key("kiro-auth-region"));
        assert!(!is_reserved_payload_extra_key("profileArn"));
    }

    #[test]
    fn generate_machine_id_normalizes_uuid_and_hashes_refresh_token() {
        let normalized =
            generate_machine_id(Some("2582956e-cc88-4669-b546-07adbffcb894"), None).unwrap();
        assert_eq!(normalized.len(), 64);
        let hashed = generate_machine_id(None, Some("refresh-token-123")).unwrap();
        assert_eq!(hashed.len(), 64);
    }

    #[test]
    fn pack_kiro_builds_current_message_and_history() {
        let mut req = make_request(vec![
            make_text_message(MessageRole::System, "Be precise."),
            make_text_message(MessageRole::User, "Hello"),
            make_text_message(MessageRole::Assistant, "Hi"),
            make_text_message(MessageRole::User, "Read the file"),
        ]);
        req.tools.push(CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("read_file".to_string()),
            description: Some("Read a file".to_string()),
            input_schema: Some(json!({"type":"object","properties":{"path":{"type":"string"}}})),
            raw: HashMap::new(),
        });

        let packed = pack_kiro(&req, "claude-sonnet-4-20250514").unwrap();
        assert_eq!(
            packed["conversationState"]["currentMessage"]["userInputMessage"]["content"],
            "Read the file"
        );
        assert_eq!(
            packed["conversationState"]["history"][0]["userInputMessage"]["content"],
            "Be precise."
        );
        assert_eq!(
            packed["conversationState"]["currentMessage"]["userInputMessage"]
                ["userInputMessageContext"]["tools"][0]["toolSpecification"]["name"],
            "read_file"
        );
    }
}
