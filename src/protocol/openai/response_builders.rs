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
            .map(crate::protocol::openai_tool_wire::chat_call)
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
        response["usage"] = chat_usage(u);
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
        chunk["usage"] = chat_usage(u);
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

fn chat_usage(usage: &TokenUsage) -> Value {
    let mut value = json!({"prompt_tokens":usage.prompt_tokens,
        "completion_tokens":usage.completion_tokens,"total_tokens":usage.total_tokens});
    // Unknown cache usage remains absent; an explicit upstream zero is still meaningful.
    if let Some(cached) = usage.cache_read_input_tokens {
        value["prompt_tokens_details"] = json!({"cached_tokens":cached});
    }
    value
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
        Some("content_filter") => "content_filter",
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_usage_builders_preserve_known_cache_without_inventing_unknown_usage() {
        for cached in [None, Some(0), Some(3)] {
            let usage = TokenUsage {
                prompt_tokens: 8,
                completion_tokens: 2,
                total_tokens: 10,
                cache_read_input_tokens: cached,
                cache_creation_input_tokens: None,
            };
            for body in [
                build_chat_completions_success(
                    "id",
                    0,
                    "model",
                    "OK",
                    Some(&usage),
                    &[],
                    Some("stop"),
                ),
                build_chat_completions_stop("id", 0, "model", Some(&usage)),
            ] {
                assert_eq!(body["usage"]["total_tokens"], 10);
                assert_eq!(
                    body["usage"]
                        .pointer("/prompt_tokens_details/cached_tokens")
                        .and_then(Value::as_u64),
                    cached
                );
                assert_eq!(
                    body["usage"].get("prompt_tokens_details").is_some(),
                    cached.is_some()
                );
            }
        }
        assert!(build_chat_completions_stop("id", 0, "model", None)
            .get("usage")
            .is_none());
    }
    #[test]
    fn openai_wire_builders_preserve_content_filter_instead_of_claiming_stop() {
        let chat =
            build_chat_completions_success("id", 0, "model", "", None, &[], Some("content_filter"));
        let legacy =
            build_legacy_completions_success("id", 0, "model", "", None, Some("content_filter"));
        let stop = build_legacy_completions_stop("id", 0, "model", None, Some("content_filter"));
        for body in [chat, legacy, stop] {
            assert_eq!(body["choices"][0]["finish_reason"], "content_filter");
        }
        for (input, expected) in [
            ("stop", "stop"),
            ("length", "length"),
            ("tool_calls", "tool_calls"),
            ("completed", "stop"),
        ] {
            assert_eq!(
                normalize_openai_wire_finish_reason(Some(input), false),
                expected
            );
        }
    }
}
