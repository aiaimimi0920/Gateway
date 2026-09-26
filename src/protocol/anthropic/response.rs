use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};

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
