use serde_json::{json, Value};

use super::stream_support::map_openai_finish_reason_to_responses_status;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};

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
        "status": finish_reason.and_then(map_openai_finish_reason_to_responses_status)
            .unwrap_or_else(|| "completed".into()),
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

pub(super) fn normalize_arguments_value(value: &Value) -> String {
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
