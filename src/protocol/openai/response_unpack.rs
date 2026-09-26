use serde_json::Value;

use super::tool_calls::parse_openai_message_tool_calls;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, TokenUsage};
use crate::protocol::tool_inject;

/// Unpack an OpenAI response JSON body into a [`CanonicalRelayResponse`].
pub fn unpack_openai_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    let model = body
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let choices = body
        .get("choices")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GatewayError::server_error("OpenAI response missing `choices`"))?;

    if choices.is_empty() {
        return Err(GatewayError::server_error(
            "OpenAI response `choices` is empty",
        ));
    }

    let first = &choices[0];
    let message = first
        .get("message")
        .ok_or_else(|| GatewayError::server_error("OpenAI response choice missing `message`"))?;

    let mut text = match message.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    };

    let mut finish_reason =
        map_openai_finish_reason(first.get("finish_reason").and_then(|v| v.as_str()));

    let mut tool_calls = parse_openai_message_tool_calls(message);
    if tool_calls.is_empty()
        && (text.contains("<tool_calls>")
            || text.contains("<function_calls>")
            || text.contains("<invoke "))
    {
        let parse_result = tool_inject::parse_tool_calls_from_text(&text);
        if parse_result.had_tool_calls {
            text = parse_result.clean_text;
            tool_calls = parse_result.tool_calls;
            finish_reason = Some("tool_calls".to_string());
        }
    }

    let usage = body.get("usage").map(|u| {
        let prompt_tokens = u
            .get("prompt_tokens")
            .or_else(|| u.get("input_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        let completion_tokens = u
            .get("completion_tokens")
            .or_else(|| u.get("output_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: u
                .get("total_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(prompt_tokens + completion_tokens),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: u
                .get("prompt_tokens_details")
                .and_then(|details| details.get("cached_tokens"))
                .and_then(|tokens| tokens.as_u64())
                .or_else(|| {
                    u.get("input_tokens_details")
                        .and_then(|details| details.get("cached_tokens"))
                        .and_then(|tokens| tokens.as_u64())
                }),
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

pub(super) fn map_openai_finish_reason(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "function_call" | "tool_use" => "tool_calls",
            other => other,
        }
        .to_string(),
    )
}
