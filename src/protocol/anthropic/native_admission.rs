//! Validate Messages prior to lossless JSON/SSE publication.
use crate::protocol::canonical::TokenUsage;
use serde_json::Value;

pub(super) fn usage(value: &Value) -> Result<(), ()> {
    if !value.is_null()
        && (!value.is_object()
            || [
                "input_tokens",
                "output_tokens",
                "cache_read_input_tokens",
                "cache_creation_input_tokens",
            ]
            .iter()
            .any(|key| value.get(key).is_some_and(|n| n.as_u64().is_none())))
    {
        return Err(());
    }
    if let (Some(input), Some(output)) = (
        value["input_tokens"].as_u64(),
        value["output_tokens"].as_u64(),
    ) {
        input.checked_add(output).ok_or(())?;
    }
    Ok(())
}

pub(crate) fn validate(value: &Value) -> Result<(), ()> {
    if value["type"] != "message"
        || value["role"] != "assistant"
        || !value["error"].is_null()
        || !value["id"].as_str().is_some_and(|s| !s.is_empty())
        || !value["model"].as_str().is_some_and(|s| !s.is_empty())
        || !matches!(
            value["stop_reason"].as_str(),
            Some(
                "end_turn"
                    | "stop_sequence"
                    | "max_tokens"
                    | "tool_use"
                    | "pause_turn"
                    | "refusal"
                    | "model_context_window_exceeded"
            )
        )
    {
        return Err(());
    }
    let blocks = value["content"].as_array().ok_or(())?;
    if value["stop_reason"] == "tool_use" && !blocks.iter().any(|b| b["type"] == "tool_use") {
        return Err(());
    }
    for block in blocks {
        match block["type"]
            .as_str()
            .filter(|kind| !kind.is_empty())
            .ok_or(())?
        {
            "text" if !block["text"].is_string() => return Err(()),
            "tool_use" => {
                if !(block["id"].as_str().is_some_and(|s| !s.is_empty())
                    && block["name"].as_str().is_some_and(|s| !s.is_empty())
                    && block["input"].is_object())
                {
                    return Err(());
                }
            }
            // Thinking, signatures, citations and server-tool blocks remain in the original JSON.
            _ => {}
        }
    }
    usage(&value["usage"])
}

/// Read only the authoritative fields, without interpreting opaque content as Gateway metadata.
pub(crate) fn observe(body: &Value) -> (Option<TokenUsage>, &'static str) {
    let usage = &body["usage"];
    let tokens = || {
        let prompt_tokens = usage["input_tokens"].as_u64()?;
        let completion_tokens = usage["output_tokens"].as_u64()?;
        Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.checked_add(completion_tokens)?,
            cache_creation_input_tokens: usage["cache_creation_input_tokens"].as_u64(),
            cache_read_input_tokens: usage["cache_read_input_tokens"].as_u64(),
        })
    };
    let finish = match body["stop_reason"].as_str() {
        Some("end_turn" | "stop_sequence") => "stop",
        Some("tool_use") => "tool_calls",
        Some("max_tokens" | "model_context_window_exceeded") => "length",
        Some("refusal") => "content_filter",
        _ => "other_provider_reason",
    };
    (tokens(), finish)
}
