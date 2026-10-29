//! Native Cohere output decoding; actual tokens are distinct from billed units.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage},
};
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn invalid() -> GatewayError {
    GatewayError::server_error("Invalid or incomplete Cohere response")
        .with_provider("cohere_compatible")
}

pub(super) fn finish(value: &Value) -> Result<&'static str, GatewayError> {
    match value.as_str() {
        Some("COMPLETE" | "STOP_SEQUENCE") => Ok("stop"),
        Some("MAX_TOKENS") => Ok("length"),
        Some("TOOL_CALL") => Ok("tool_calls"),
        _ => Err(invalid()),
    }
}

fn count(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && n.fract() == 0.0 && *n < u64::MAX as f64)
            .map(|n| n as u64)
    })
}

pub(super) fn usage(value: &Value) -> Option<TokenUsage> {
    let tokens = value.get("tokens")?;
    let prompt_tokens = count(&tokens["input_tokens"])?;
    let completion_tokens = count(&tokens["output_tokens"])?;
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens.checked_add(completion_tokens)?,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: count(&value["cached_tokens"]),
    })
}

pub fn unpack(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let finish_reason = finish(&body["finish_reason"])?;
    let message = body.get("message").ok_or_else(invalid)?;
    let text = match &message["content"] {
        Value::String(value) => value.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|v| v["text"].as_str())
            .collect::<String>(),
        Value::Null => String::new(),
        _ => return Err(invalid()),
    };
    let mut tool_calls = Vec::new();
    if let Some(calls) = message.get("tool_calls") {
        let calls = calls
            .as_array()
            .filter(|calls| calls.len() <= 128)
            .ok_or_else(invalid)?;
        for call in calls {
            let id = call["id"]
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 256)
                .ok_or_else(invalid)?;
            let name = call["function"]["name"]
                .as_str()
                .filter(|v| !v.is_empty() && v.len() <= 256)
                .ok_or_else(invalid)?;
            let arguments = match &call["function"]["arguments"] {
                Value::String(value) => value.clone(),
                Value::Object(_) => call["function"]["arguments"].to_string(),
                Value::Null => "{}".into(),
                _ => return Err(invalid()),
            };
            tool_calls.push(CanonicalToolCall {
                id: Some(id.into()),
                call_type: "function".into(),
                name: Some(name.into()),
                arguments: Some(arguments),
                raw: HashMap::new(),
            });
        }
    }
    if text.trim().is_empty() && tool_calls.is_empty() {
        return Err(invalid());
    }
    Ok(CanonicalRelayResponse {
        model_declaration: None,
        model: body["model"].as_str().unwrap_or("unknown").into(),
        text,
        usage: usage(&body["usage"]),
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(finish_reason.into()),
    })
}
