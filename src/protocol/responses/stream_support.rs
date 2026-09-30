use serde_json::Value;

use crate::protocol::canonical::TokenUsage;

pub(super) fn extract_openai_stream_usage(chunk: &Value) -> Option<TokenUsage> {
    let usage = chunk.get("usage")?;
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|value| value.as_u64())
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|value| value.as_u64()),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|value| value.as_u64())
            .or_else(|| {
                usage
                    .get("prompt_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            })
            .or_else(|| {
                usage
                    .get("input_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            }),
    })
}

pub(super) fn merge_stream_usage(
    existing: Option<TokenUsage>,
    incoming: Option<TokenUsage>,
) -> Option<TokenUsage> {
    match (existing, incoming) {
        (None, None) => None,
        (Some(current), None) => Some(current),
        (None, Some(new_usage)) => Some(new_usage),
        (Some(current), Some(new_usage)) => Some(TokenUsage {
            prompt_tokens: current.prompt_tokens.max(new_usage.prompt_tokens),
            completion_tokens: current.completion_tokens.max(new_usage.completion_tokens),
            total_tokens: current.total_tokens.max(new_usage.total_tokens).max(
                current
                    .prompt_tokens
                    .max(new_usage.prompt_tokens)
                    .saturating_add(current.completion_tokens.max(new_usage.completion_tokens)),
            ),
            cache_creation_input_tokens: new_usage
                .cache_creation_input_tokens
                .or(current.cache_creation_input_tokens),
            cache_read_input_tokens: new_usage
                .cache_read_input_tokens
                .or(current.cache_read_input_tokens),
        }),
    }
}

pub(super) fn map_openai_finish_reason_to_responses_status(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(
        match value {
            "tool_calls" | "function_call" => "completed",
            "stop" | "end_turn" | "stop_sequence" | "completed" => "completed",
            "length" | "max_tokens" | "content_filter" | "incomplete" => "incomplete",
            _ => "failed",
        }
        .to_string(),
    )
}
