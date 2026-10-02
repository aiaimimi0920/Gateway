use std::collections::HashMap;

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalMessage, CanonicalRelayRequest, MessageRole};

mod accumulator;
mod accumulator_content;
mod anthropic_events;
pub use accumulator::accumulate_kiro_stream;
mod event_stream;
#[cfg(test)]
mod request_metadata_tests;
use anthropic_events::{anthropic_event_bytes, push_anthropic_events, push_anthropic_finish};
mod openai_events;
use openai_events::{push_openai_events, push_openai_finish};
mod event_stream_translate;
mod request_messages;
mod stream_state;

#[cfg(test)]
mod event_stream_tests;

#[cfg(test)]
mod event_stream_lifecycle_tests;

#[cfg(test)]
mod stream_contract_tests;

#[cfg(test)]
mod stream_limits_tests;

#[cfg(test)]
mod accumulator_tests;

#[cfg(test)]
mod accumulator_limits_tests;

#[cfg(test)]
mod request_messages_tests;

use event_stream::{EventStreamParser, KiroEvent};
use event_stream_translate::translate_kiro_stream_with_error;
use request_messages::{
    build_history_messages, build_short_tool_name_map, build_tool_name_map, build_user_message,
    ensure_history_tools_declared, pack_tool, resolve_conversation_id, validate_tool_pairing,
};
use stream_state::{build_translator_state, TranslatorState};

pub const KIRO_DEFAULT_MODEL: &str = "claude-sonnet-4.6";
pub const KIRO_GENERATE_ASSISTANT_RESPONSE_PATH: &str = "/generateAssistantResponse";
pub const KIRO_DEFAULT_VERSION: &str = "0.11.107";
pub const KIRO_DEFAULT_SYSTEM_VERSION: &str = "win32#10.0.22631";
pub const KIRO_DEFAULT_NODE_VERSION: &str = "22.22.0";

pub fn pack_kiro(req: &CanonicalRelayRequest, model: &str) -> Result<Value, GatewayError> {
    let requested_model = map_model(model);
    let tool_name_map = build_tool_name_map(req);
    let short_tool_name_map = build_short_tool_name_map(&tool_name_map);
    let mut tools = req
        .tools
        .iter()
        .map(|tool| pack_tool(tool, &short_tool_name_map))
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

    let mut history = build_history_messages(
        req,
        history_messages,
        &requested_model,
        &short_tool_name_map,
    )?;
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

pub fn translate_kiro_event_stream_to_openai_sse(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_kiro_event_stream_to_openai_sse_with_error(inner, model, req)
}

pub fn translate_kiro_event_stream_to_openai_sse_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    translate_kiro_stream_with_error(inner, build_translator_state(model, req), false)
}

pub fn translate_kiro_event_stream_to_anthropic_sse(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_kiro_event_stream_to_anthropic_sse_with_error(inner, model, req)
}

pub fn translate_kiro_event_stream_to_anthropic_sse_with_error<E: Send + 'static>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    req: CanonicalRelayRequest,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static {
    translate_kiro_stream_with_error(inner, build_translator_state(model, req), true)
}

// 保留旧状态注入测试入口的类型推断，生产入口使用同一泛型状态机。
#[cfg(test)]
fn translate_kiro_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    state: TranslatorState,
    anthropic: bool,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_kiro_stream_with_error(inner, state, anthropic)
}

fn push_kiro_stream_error(state: &mut TranslatorState, error: GatewayError, anthropic: bool) {
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
    }
}
