use std::collections::HashMap;

use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};

pub fn parse_response(body: &Value, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut total_tokens = 0u64;
    let mut reported_model = body
        .get("modelVersion")
        .or_else(|| body.get("model"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(model)
        .to_string();
    let mut finish_reason = None;

    if let Some(usage) = body.get("usageMetadata") {
        prompt_tokens = usage
            .get("promptTokenCount")
            .and_then(|value| value.as_u64())
            .unwrap_or(prompt_tokens);
        completion_tokens = usage
            .get("candidatesTokenCount")
            .and_then(|value| value.as_u64())
            .unwrap_or(completion_tokens);
        total_tokens = usage
            .get("totalTokenCount")
            .and_then(|value| value.as_u64())
            .unwrap_or(total_tokens);
    }

    let candidates = body
        .get("candidates")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini generateContent response did not include candidates.",
            )
            .with_code("gemini_missing_candidates")
        })?;

    for candidate in candidates {
        if let Some(value) = candidate
            .get("modelVersion")
            .or_else(|| candidate.get("model"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            reported_model = value.to_string();
        }
        if let Some(parts) = candidate
            .get("content")
            .and_then(|value| value.get("parts"))
            .and_then(|value| value.as_array())
        {
            for part in parts {
                if let Some(value) = part.get("text").and_then(|value| value.as_str()) {
                    text.push_str(value);
                }
                if let Some(function_call) = part
                    .get("functionCall")
                    .or_else(|| part.get("function_call"))
                {
                    let tool_call = parse_gemini_tool_call(function_call);
                    if !tool_calls.iter().any(|existing: &CanonicalToolCall| {
                        existing.id == tool_call.id
                            && existing.name == tool_call.name
                            && existing.arguments == tool_call.arguments
                    }) {
                        tool_calls.push(tool_call);
                    }
                }
            }
        }
        if let Some(reason) = candidate
            .get("finishReason")
            .and_then(|value| value.as_str())
        {
            finish_reason = Some(map_gemini_finish_reason_to_canonical(
                reason,
                !tool_calls.is_empty(),
            ));
        }
    }

    let usage = if total_tokens > 0 {
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        })
    } else {
        None
    };
    let resolved_finish_reason = finish_reason.unwrap_or_else(|| {
        if tool_calls.is_empty() {
            "stop".to_string()
        } else {
            "tool_calls".to_string()
        }
    });

    Ok(CanonicalRelayResponse {
        model: reported_model,
        text,
        usage,
        tool_calls,
        upstream_status: Some(200),
        finish_reason: Some(resolved_finish_reason),
    })
}

pub(crate) fn build_generate_content_success(
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut parts = Vec::new();
    if !text.is_empty() || tool_calls.is_empty() {
        parts.push(json!({"text": text}));
    }
    for tool_call in tool_calls {
        let args = tool_call
            .arguments
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .unwrap_or_else(|| json!({}));
        parts.push(json!({
            "functionCall": {
                "id": tool_call.id,
                "name": tool_call.name,
                "args": args,
            }
        }));
    }

    let mut body = json!({
        "candidates": [{
            "index": 0,
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": map_gemini_finish_reason(finish_reason, tool_calls),
        }],
        "modelVersion": model,
    });

    if let Some(usage) = usage {
        body["usageMetadata"] = json!({
            "promptTokenCount": usage.prompt_tokens,
            "candidatesTokenCount": usage.completion_tokens,
            "totalTokenCount": usage.total_tokens,
        });
    }

    body
}

pub(crate) fn parse_gemini_tool_call(raw: &Value) -> CanonicalToolCall {
    CanonicalToolCall {
        id: raw
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        call_type: "function".to_string(),
        name: raw
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        arguments: raw
            .get("args")
            .or_else(|| raw.get("arguments"))
            .map(|value| {
                if let Some(text) = value.as_str() {
                    text.to_string()
                } else {
                    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
                }
            }),
        raw: HashMap::new(),
    }
}

pub(crate) fn map_gemini_finish_reason(
    finish_reason: Option<&str>,
    tool_calls: &[CanonicalToolCall],
) -> &'static str {
    match finish_reason.unwrap_or(if tool_calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    }) {
        "length" => "MAX_TOKENS",
        "content_filter" => "SAFETY",
        _ => "STOP",
    }
}

pub(crate) fn map_gemini_finish_reason_to_canonical(reason: &str, has_tool_calls: bool) -> String {
    match reason {
        "MAX_TOKENS" => "length".to_string(),
        "SAFETY" => "content_filter".to_string(),
        "STOP" => {
            if has_tool_calls {
                "tool_calls".to_string()
            } else {
                "stop".to_string()
            }
        }
        other => other.to_ascii_lowercase(),
    }
}
