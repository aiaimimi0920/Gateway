//! Shared native Responses output checks and authoritative terminal accounting.
use crate::protocol::canonical::TokenUsage;
use serde_json::Value;

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.trim().is_empty())
}

pub(crate) fn valid_terminal(body: &Value) -> bool {
    nonempty(&body["id"])
        && matches!(body["status"].as_str(), Some("completed" | "incomplete"))
        && body["error"].is_null()
        && body.get("object").is_none_or(|v| v == "response")
        && body.get("output").is_none_or(|v| {
            v.as_array()
                .is_some_and(|items| items.iter().all(valid_output))
        })
        && (body["usage"].is_null() || usage(body).is_some())
}

fn valid_output(item: &Value) -> bool {
    match item["type"].as_str() {
        Some("message") => {
            item["role"] == "assistant"
                && item["content"].as_array().is_some_and(|parts| {
                    parts.iter().all(|part| match part["type"].as_str() {
                        Some("output_text") => part["text"].is_string(),
                        Some("refusal") => part["refusal"].is_string(),
                        Some(kind) => !kind.is_empty(),
                        None => false,
                    })
                })
        }
        Some("function_call") => {
            nonempty(&item["name"]) && nonempty(&item["call_id"]) && item["arguments"].is_string()
        }
        Some(kind) => !kind.is_empty(),
        None => false,
    }
}

fn usage(body: &Value) -> Option<TokenUsage> {
    let usage = &body["usage"];
    let input = usage["input_tokens"].as_u64()?;
    let output = usage["output_tokens"].as_u64()?;
    let total = input.checked_add(output)?;
    if usage
        .get("total_tokens")
        .is_some_and(|reported| reported.as_u64() != Some(total))
    {
        return None;
    }
    Some(TokenUsage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: total,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: usage["input_tokens_details"]["cached_tokens"]
            .as_u64()
            .filter(|cached| *cached <= input),
    })
}

pub(crate) fn observe(body: &Value) -> (Option<TokenUsage>, &'static str) {
    let finish = if body["status"] == "incomplete" {
        match body["incomplete_details"]["reason"].as_str() {
            Some("max_output_tokens") => "length",
            Some("content_filter") => "content_filter",
            _ => "other_provider_reason",
        }
    } else if body["output"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["type"] == "function_call"))
    {
        "tool_calls"
    } else {
        "stop"
    };
    (usage(body), finish)
}
