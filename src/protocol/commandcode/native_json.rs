//! Same-protocol JSON retains native content; validation does not execute opaque tools.
use super::*;
use crate::{protocol::canonical::CanonicalRelayRequest, upstream::client::UpstreamClient};
use serde_json::Value;

pub(crate) fn selected(payload: &ProviderAccountPayload, req: &CanonicalRelayRequest) -> bool {
    owns_payload(payload)
        && match req.endpoint_kind {
            EndpointKind::SystemOne => true,
            EndpointKind::Messages => payload.messages_path.as_deref() == Some("/messages"),
            EndpointKind::Responses => payload.responses_path.as_deref() == Some("/responses"),
            _ => false,
        }
}

pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    if !selected(payload, req) {
        return Err(invalid());
    }
    // Keep the product sender's fixed target, account identity, no redirects and redacted errors.
    let response = runtime::send(client, payload, req, model, false).await?;
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "Command Code native JSON",
    )
    .await
    .map_err(|_| response_error())?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    match req.endpoint_kind {
        EndpointKind::SystemOne => systemone::validate_response(&body, &req.raw_body)?,
        EndpointKind::Messages => messages::validate_native(&body)?,
        EndpointKind::Responses => responses::validate_native(&body)?,
        _ => return Err(invalid()),
    }
    Ok(body)
}

pub(super) fn content_type(value: &Value) -> Result<&str, GatewayError> {
    value["type"]
        .as_str()
        .filter(|kind| !kind.is_empty())
        .ok_or_else(response_error)
}

/// Observe only validated native usage and terminal fields. Opaque Gateway-looking extensions
/// must neither enter another product's turn decoder nor trigger the generic usage fallback.
pub(crate) fn observe(
    body: &Value,
    endpoint: EndpointKind,
) -> (Option<crate::protocol::canonical::TokenUsage>, &'static str) {
    if endpoint == EndpointKind::SystemOne {
        return (systemone::observe(body), "stop");
    }
    let usage = &body["usage"];
    let input = usage
        .get("input_tokens")
        .or_else(|| {
            (endpoint == EndpointKind::Responses)
                .then(|| usage.get("prompt_tokens"))
                .flatten()
        })
        .and_then(Value::as_u64);
    let output = usage
        .get("output_tokens")
        .or_else(|| {
            (endpoint == EndpointKind::Responses)
                .then(|| usage.get("completion_tokens"))
                .flatten()
        })
        .and_then(Value::as_u64);
    let observed = input
        .zip(output)
        .and_then(|(prompt_tokens, completion_tokens)| {
            Some(crate::protocol::canonical::TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens.checked_add(completion_tokens)?,
                cache_creation_input_tokens: usage["cache_creation_input_tokens"].as_u64(),
                cache_read_input_tokens: usage["cache_read_input_tokens"]
                    .as_u64()
                    .or_else(|| usage["input_tokens_details"]["cached_tokens"].as_u64())
                    .or_else(|| usage["prompt_tokens_details"]["cached_tokens"].as_u64()),
            })
        });
    let completion = if endpoint == EndpointKind::Messages {
        match body["stop_reason"].as_str() {
            Some("end_turn" | "stop_sequence") => "stop",
            Some("tool_use") => "tool_calls",
            Some("max_tokens" | "model_context_window_exceeded") => "length",
            Some("refusal") => "content_filter",
            _ => "other_provider_reason",
        }
    } else if body["status"] == "completed" {
        if body["output"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item["type"].as_str(),
                    Some("function_call" | "custom_tool_call")
                )
            })
        }) {
            "tool_calls"
        } else {
            "stop"
        }
    } else {
        match body["incomplete_details"]["reason"].as_str() {
            Some("max_output_tokens") => "length",
            Some("content_filter") => "content_filter",
            _ => "other_provider_reason",
        }
    };
    (observed, completion)
}
