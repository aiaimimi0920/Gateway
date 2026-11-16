//! Bounded single-turn Messages proof; absent usage is unknown, not a zero-token success.
use super::*;
use crate::{
    protocol::{anthropic, canonical::CanonicalRelayResponse, openai},
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    model: &str,
    prompt: &str,
) -> Result<(u16, CanonicalRelayResponse), GatewayError> {
    let request = openai::normalize_chat_completions(json!({
        "model":model,"stream":false,"max_tokens":256,
        "messages":[{"role":"user","content":prompt}]
    }))?;
    let plan = UpstreamClient::build_request_plan(payload, &request, model, false)?;
    // Production plan/finalizer retain the configured regional target and exact account identity.
    let response = client
        .send_profiled_plan(payload, &plan, rquest::header::HeaderMap::new())?
        .send()
        .await?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(response_error());
    }
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "MiniMax model test",
            65_536,
            None,
        )
        .await
        .map_err(|_| response_error())?;
    let body: Value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    validate_json_response(payload, &body)?;
    if body["stop_reason"] != "end_turn"
        || body["content"].as_array().is_none_or(|items| {
            items.is_empty()
                || items.iter().any(|item| match item["type"].as_str() {
                    Some("text") => !item["text"].is_string(),
                    Some("thinking") => !item["thinking"].is_string(),
                    _ => true,
                })
        })
    {
        return Err(response_error());
    }
    // An absent usage object is valid unknown usage. A present but malformed counter is a failure.
    if let Some(usage) = body.get("usage").filter(|v| !v.is_null()) {
        let input = usage["input_tokens"].as_u64().ok_or_else(response_error)?;
        let output = usage["output_tokens"].as_u64().ok_or_else(response_error)?;
        input.checked_add(output).ok_or_else(response_error)?;
        for field in ["cache_read_input_tokens", "cache_creation_input_tokens"] {
            if usage
                .get(field)
                .is_some_and(|v| !v.is_null() && v.as_u64().is_none())
            {
                return Err(response_error());
            }
        }
    }
    let mut result = anthropic::unpack_anthropic_response(&body).map_err(|_| response_error())?;
    if result.text.trim().is_empty() || result.text.chars().count() > 8192 {
        return Err(response_error());
    }
    result.text = result.text.replace(&payload.api_key, "[redacted]");
    result.text = crate::error::sanitize_provider_error_message(&result.text);
    Ok((status, result))
}
