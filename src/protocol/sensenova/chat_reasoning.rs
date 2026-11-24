//! Preserve provider-owned Chat reasoning without restoring raw messages over Gateway policy.
use super::*;
use crate::protocol::{
    canonical::{MessageRole, TokenUsage},
    openai,
};
use serde_json::Value;

pub(crate) fn restore_history(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    plan: &mut RequestPlan,
) -> Result<(), GatewayError> {
    if !owns_payload(payload) || req.endpoint_kind != EndpointKind::ChatCompletions {
        return Ok(());
    }
    let Some(raw) = req.raw_body["messages"].as_array() else {
        return Ok(());
    };
    if !raw.iter().any(|item| !item["reasoning_content"].is_null()) {
        return Ok(());
    }
    if raw.iter().any(|item| {
        !item["reasoning_content"].is_null()
            && (item["role"] != "assistant" || !item["reasoning_content"].is_string())
    }) {
        return Err(invalid_history());
    }
    let original = openai::normalize_chat_completions(req.raw_body.clone())?;
    let mut original = original
        .messages
        .iter()
        .filter(|item| item.role != MessageRole::System);
    let Some(messages) = plan
        .body
        .as_mut()
        .and_then(|body| body["messages"].as_array_mut())
    else {
        return Err(invalid_history());
    };
    let mut wire = messages.iter_mut().filter(|item| item["role"] != "system");
    // Admin/system changes may move the wire positions. Refuse to reattach opaque reasoning
    // if any non-system turn was rewritten or reordered rather than guessing its new owner.
    for item in raw.iter().filter(|item| item["role"] != "system") {
        let expected = original.next().ok_or_else(invalid_history)?;
        let target = wire.next().ok_or_else(invalid_history)?;
        if *target != openai::pack_openai_message(expected) {
            return Err(invalid_history());
        }
        if let Some(reasoning) = item["reasoning_content"].as_str() {
            target["reasoning_content"] = Value::String(reasoning.to_owned());
        }
    }
    if original.next().is_some() || wire.next().is_some() {
        return Err(invalid_history());
    }
    Ok(())
}

fn invalid_history() -> GatewayError {
    GatewayError::bad_request(
        "Cannot preserve SenseNova Chat reasoning for invalid or rewritten history.",
    )
    .with_code("sensenova_chat_reasoning_history_invalid")
}

pub(super) async fn read(response: rquest::Response) -> Result<Value, GatewayError> {
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_body(response, "SenseNova Chat")
            .await
            .map_err(|_| response_error())?;
    let value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    openai::chat_admission::validate(&value).map_err(|_| response_error())?;
    if value["choices"].as_array().unwrap().iter().any(|choice| {
        let reasoning = &choice["message"]["reasoning_content"];
        !reasoning.is_null() && !reasoning.is_string()
    }) {
        return Err(response_error());
    }
    Ok(value)
}

pub(crate) fn observe(body: &Value) -> (Option<TokenUsage>, &'static str) {
    let usage = &body["usage"];
    let tokens = usage["prompt_tokens"]
        .as_u64()
        .zip(usage["completion_tokens"].as_u64())
        .and_then(|(prompt_tokens, completion_tokens)| {
            Some(TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens.checked_add(completion_tokens)?,
                cache_read_input_tokens: usage["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .or_else(|| usage["input_tokens_details"]["cached_tokens"].as_u64()),
                cache_creation_input_tokens: None,
            })
        });
    let finish = match body["choices"][0]["finish_reason"].as_str() {
        Some("stop") => "stop",
        Some("tool_calls" | "function_call") => "tool_calls",
        Some("content_filter") => "content_filter",
        Some("length") => "length",
        _ => "other_provider_reason",
    };
    (tokens, finish)
}
