//! Candidate selection and canonicalization for Gemini Web payloads.

use std::collections::HashMap;

use serde_json::Value;

use crate::protocol::canonical::{CanonicalRelayResponse, CanonicalToolCall, TokenUsage};

pub(super) fn extract_generate_response(
    frame: &Value,
    fallback_model: &str,
) -> Option<CanonicalRelayResponse> {
    let mut best_result = None;

    if let Some(result) = extract_direct_generate_response(frame, fallback_model) {
        best_result = select_more_complete_response(best_result, result);
    }

    if let Some(inner_payload) = frame
        .as_array()
        .and_then(|items| items.get(2))
        .and_then(Value::as_str)
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
    {
        if let Some(result) = extract_direct_generate_response(&inner_payload, fallback_model) {
            best_result = select_more_complete_response(best_result, result);
        }
        if let Some(text) = get_nested_value(&inner_payload, &[4, 0, 1, 0]).and_then(Value::as_str)
        {
            let model = get_nested_value(&inner_payload, &[83]).and_then(Value::as_str);
            let result = CanonicalRelayResponse {
                model: model.unwrap_or(fallback_model).to_string(),
                text: text.to_string(),
                usage: None,
                tool_calls: Vec::new(),
                upstream_status: Some(200),
                finish_reason: Some("stop".to_string()),
            };
            best_result = select_more_complete_response(best_result, result);
        }
    }

    if let Some(items) = frame.as_array() {
        for item in items {
            if let Some(result) = extract_generate_response(item, fallback_model) {
                best_result = select_more_complete_response(best_result, result);
            }
        }
    }

    best_result
}

fn extract_direct_generate_response(
    value: &Value,
    fallback_model: &str,
) -> Option<CanonicalRelayResponse> {
    let mut tool_calls = Vec::new();
    if let Some(text) = value
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        return Some(CanonicalRelayResponse {
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(fallback_model)
                .to_string(),
            text,
            usage: extract_usage(value.get("usage")),
            tool_calls,
            upstream_status: Some(200),
            finish_reason: Some("stop".to_string()),
        });
    }

    let candidates = value.get("candidates").and_then(Value::as_array)?;
    let candidate = candidates.first()?;
    if let Some(parts) = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
    {
        for part in parts {
            if let Some(function_call) = part
                .get("functionCall")
                .or_else(|| part.get("function_call"))
            {
                let tool_call = parse_direct_generate_tool_call(function_call);
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

    let text = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .and_then(|parts| {
            let mut fragments = Vec::new();
            for part in parts {
                if let Some(value) = part.get("text").and_then(Value::as_str) {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        fragments.push(trimmed.to_string());
                    }
                }
            }
            if fragments.is_empty() {
                None
            } else {
                Some(fragments.join(""))
            }
        })
        .or_else(|| {
            candidate
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            candidate
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    if text.is_none() && tool_calls.is_empty() {
        return None;
    }

    let finish_reason = candidate
        .get("finishReason")
        .or_else(|| candidate.get("finish_reason"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            Some(if tool_calls.is_empty() {
                "stop".to_string()
            } else {
                "tool_calls".to_string()
            })
        });

    Some(CanonicalRelayResponse {
        model: value
            .get("modelVersion")
            .or_else(|| value.get("model"))
            .and_then(Value::as_str)
            .unwrap_or(fallback_model)
            .to_string(),
        text: text.unwrap_or_default(),
        usage: extract_usage(value.get("usage")),
        tool_calls,
        upstream_status: Some(200),
        finish_reason,
    })
}

fn parse_direct_generate_tool_call(raw: &Value) -> CanonicalToolCall {
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

fn extract_usage(value: Option<&Value>) -> Option<TokenUsage> {
    let usage = value?;
    Some(TokenUsage {
        prompt_tokens: usage
            .get("prompt_tokens")
            .or_else(|| usage.get("inputTokens"))
            .and_then(Value::as_u64)?,
        completion_tokens: usage
            .get("completion_tokens")
            .or_else(|| usage.get("outputTokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: usage
            .get("total_tokens")
            .or_else(|| usage.get("totalTokens"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                usage
                    .get("prompt_tokens")
                    .or_else(|| usage.get("inputTokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    + usage
                        .get("completion_tokens")
                        .or_else(|| usage.get("outputTokens"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
            }),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

fn get_nested_value<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for index in path {
        current = current.as_array()?.get(*index)?;
    }
    Some(current)
}

pub(super) fn select_more_complete_response(
    current: Option<CanonicalRelayResponse>,
    candidate: CanonicalRelayResponse,
) -> Option<CanonicalRelayResponse> {
    match current {
        None => Some(candidate),
        Some(existing) => {
            let existing_has_tool_calls = !existing.tool_calls.is_empty();
            let candidate_has_tool_calls = !candidate.tool_calls.is_empty();
            if candidate_has_tool_calls && !existing_has_tool_calls {
                return Some(candidate);
            }
            if existing_has_tool_calls && !candidate_has_tool_calls {
                return Some(existing);
            }
            let existing_len = existing.text.trim().chars().count();
            let candidate_len = candidate.text.trim().chars().count();
            if candidate_len > existing_len {
                Some(candidate)
            } else if candidate_len == existing_len
                && candidate.usage.is_some()
                && existing.usage.is_none()
            {
                Some(candidate)
            } else if candidate_len == existing_len {
                Some(candidate)
            } else {
                Some(existing)
            }
        }
    }
}
