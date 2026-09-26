use serde_json::{json, Value};

use crate::protocol::canonical::{CanonicalToolCall, TokenUsage};

/// Build a complete, non-streaming OpenAI chat/completions success response.
pub fn build_chat_completions_success(
    response_id: &str,
    created_at: i64,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    let mut message = json!({
        "role": "assistant",
        "content": text,
    });

    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls
            .iter()
            .map(|tc| json!({
                "id": tc.id,
                "type": tc.call_type,
                "function": {
                    "name": tc.name,
                    "arguments": tc.arguments,
                }
            }))
            .collect::<Vec<_>>());
    }

    let mut response = json!({
        "id": response_id,
        "object": "chat.completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, !tool_calls.is_empty()),
        }],
    });

    if let Some(u) = usage {
        response["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    response
}

/// Build a streaming SSE delta chunk in OpenAI format.
pub fn build_chat_completions_delta(
    response_id: &str,
    created_at: i64,
    model: &str,
    delta_text: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": delta_text,
            },
            "finish_reason": null,
        }],
    })
}

/// Build a streaming stop/final chunk in OpenAI format.
pub fn build_chat_completions_stop(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": normalize_openai_wire_finish_reason(Some("stop"), false),
        }],
    });

    if let Some(u) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    chunk
}

pub fn build_legacy_completions_success(
    response_id: &str,
    created_at: i64,
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut response = json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": text,
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, false),
        }],
    });

    if let Some(u) = usage {
        response["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    response
}

pub fn build_legacy_completions_delta(
    response_id: &str,
    created_at: i64,
    model: &str,
    delta_text: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": delta_text,
            "finish_reason": null,
        }],
    })
}

pub fn build_legacy_completions_stop(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "text_completion",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "text": "",
            "finish_reason": normalize_openai_wire_finish_reason(finish_reason, false),
        }],
    });

    if let Some(u) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": u.prompt_tokens,
            "completion_tokens": u.completion_tokens,
            "total_tokens": u.total_tokens,
        });
    }

    chunk
}

fn normalize_openai_wire_finish_reason(value: Option<&str>, has_tool_calls: bool) -> &'static str {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("completed") => {
            if has_tool_calls {
                "tool_calls"
            } else {
                "stop"
            }
        }
        Some("incomplete") | Some("length") | Some("max_tokens") => "length",
        Some("tool_calls") | Some("function_call") | Some("tool_use") => "tool_calls",
        Some("stop") | Some("end_turn") => "stop",
        Some(_) => "stop",
        None => {
            if has_tool_calls {
                "tool_calls"
            } else {
                "stop"
            }
        }
    }
}
