//! Restore explicitly indexed public turns only while their whole native request prefix still matches.
use crate::{
    error::GatewayError,
    protocol::{
        bedrock_turn::{self, BedrockAssistantTurn},
        canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
    },
};
use serde_json::Value;
use std::collections::HashSet;

fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid or changed Bedrock public continuation.")
        .with_code("bedrock_invalid_signed_history")
}

pub(super) fn apply(
    req: &CanonicalRelayRequest,
    body: &mut Value,
    model: &str,
) -> Result<(), GatewayError> {
    if req
        .extra
        .keys()
        .any(|k| k == bedrock_turn::HISTORY_FIELD || bedrock_turn::response_field(k))
        || req
            .raw_body
            .as_object()
            .is_some_and(|v| v.keys().any(|k| bedrock_turn::response_field(k)))
    {
        return Err(invalid());
    }
    let Some(raw) = req.raw_body.get(bedrock_turn::HISTORY_FIELD) else {
        return Ok(());
    };
    if req
        .raw_body
        .get(crate::protocol::vertex_turn::HISTORY_FIELD)
        .is_some()
        || req.extra.contains_key("messages")
        || req.extra.contains_key("system")
    {
        return Err(invalid());
    }
    // Admit the envelope before re-normalizing (and cloning) the raw public request.
    bedrock_turn::wire::within_limit(raw, 8 * 1024 * 1024).map_err(|_| invalid())?;
    let entries = raw
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 64)
        .ok_or_else(invalid)?;
    // The raw public body must not restore messages that policy has changed after normalization.
    let original = match (req.protocol_family, req.endpoint_kind) {
        (ProtocolFamily::OpenAi, EndpointKind::ChatCompletions) => {
            crate::protocol::openai::normalize_chat_completions(req.raw_body.clone())
        }
        (ProtocolFamily::OpenAi, EndpointKind::Responses) => {
            crate::protocol::responses::normalize_responses(req.raw_body.clone())
        }
        (ProtocolFamily::Anthropic, EndpointKind::Messages) => {
            crate::protocol::anthropic::normalize_messages(req.raw_body.clone())
        }
        _ => return Err(invalid()),
    }?;
    if serde_json::to_value(&original.messages).map_err(|_| invalid())?
        != serde_json::to_value(&req.messages).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    // Count and serialized bytes are bounded before cloning each independently bounded turn.
    for call in req.messages.iter().flat_map(|m| &m.tool_calls) {
        if !matches!(call.call_type.as_str(), "function" | "function_call")
            || call.id.as_deref().is_none_or(|v| !valid_id(v))
            || call.name.as_deref().is_none_or(|v| v.is_empty())
            || call
                .arguments
                .as_deref()
                .and_then(|v| serde_json::from_str::<Value>(v).ok())
                .is_none()
        {
            return Err(invalid());
        }
    }
    let system = body.get("system").cloned();
    let messages = body
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .ok_or_else(invalid)?;
    let indices: Vec<_> = messages
        .iter()
        .enumerate()
        .filter(|(_, v)| v["role"] == "assistant")
        .map(|(i, _)| i)
        .collect();
    let mut previous = None;
    for entry in entries {
        if entry.as_object().is_none_or(|o| {
            o.len() != 2 || !o.contains_key("assistant_turn") || !o.contains_key("turn")
        }) {
            return Err(invalid());
        }
        let ordinal = entry["assistant_turn"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(invalid)?;
        if previous.is_some_and(|v| ordinal <= v) {
            return Err(invalid());
        }
        previous = Some(ordinal);
        let index = *indices.get(ordinal).ok_or_else(invalid)?;
        let turn = bedrock_turn::wire::read_turn(&entry["turn"]).map_err(|_| invalid())?;
        let continuation = turn.continuation.as_ref().ok_or_else(invalid)?;
        if !continuation
            .matches(&messages[..index], system.as_ref(), model)
            .map_err(|_| invalid())?
        {
            return Err(invalid());
        }
        matches_visible(&turn, &messages[index])?;
        messages[index]["content"] = turn.native_content();
    }
    validate_tools(messages)?;
    Ok(())
}

fn matches_visible(turn: &BedrockAssistantTurn, message: &Value) -> Result<(), GatewayError> {
    let content = message["content"].as_array().ok_or_else(invalid)?;
    if content.iter().any(|v| {
        v.as_object().is_none_or(|o| o.len() != 1)
            || !(v.get("text").is_some_and(Value::is_string) || v.get("toolUse").is_some())
    }) {
        return Err(invalid());
    }
    let (text, tools) = turn.projection();
    let visible: String = content.iter().filter_map(|v| v["text"].as_str()).collect();
    let expected: Vec<_> = content.iter().filter_map(|v| v.get("toolUse")).collect();
    if text != visible || tools.len() != expected.len() {
        return Err(invalid());
    }
    for (tool, expected) in tools.iter().zip(expected) {
        if tool.id.as_deref() != expected["toolUseId"].as_str()
            || tool.name.as_deref() != expected["name"].as_str()
            || tool
                .arguments
                .as_deref()
                .and_then(|v| serde_json::from_str::<Value>(v).ok())
                .as_ref()
                != expected.get("input")
        {
            return Err(invalid());
        }
    }
    Ok(())
}

// Every result must match the immediately preceding assistant turn, including parallel calls.
fn validate_tools(messages: &[Value]) -> Result<(), GatewayError> {
    let mut pending = HashSet::new();
    for message in messages {
        let content = message["content"].as_array().ok_or_else(invalid)?;
        if message["role"] == "assistant" {
            if !pending.is_empty() {
                return Err(invalid());
            }
            for call in content.iter().filter_map(|v| v.get("toolUse")) {
                let id = call["toolUseId"]
                    .as_str()
                    .filter(|id| valid_id(id))
                    .ok_or_else(invalid)?;
                if !pending.insert(id) {
                    return Err(invalid());
                }
            }
        } else {
            for result in content.iter().filter_map(|v| v.get("toolResult")) {
                let id = result["toolUseId"].as_str().ok_or_else(invalid)?;
                if !pending.remove(id) {
                    return Err(invalid());
                }
            }
            if !pending.is_empty() {
                return Err(invalid());
            }
        }
    }
    if !pending.is_empty() {
        return Err(invalid());
    }
    Ok(())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"_.:-".contains(&v))
}
