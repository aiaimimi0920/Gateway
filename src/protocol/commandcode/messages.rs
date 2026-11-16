//! Native Messages validates envelope/usage separately from lossless canonical conversion.
use super::*;
use crate::protocol::{
    anthropic,
    canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
};
use serde_json::{json, Value};

pub(crate) fn selected(payload: &ProviderAccountPayload) -> bool {
    owns_payload(payload) && payload.messages_path.as_deref() == Some("/messages")
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let body = super::native_request::pack(req, model, stream)?;
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/messages"),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::Messages,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}

pub(super) async fn read(
    response: rquest::Response,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "Command Code Messages",
    )
    .await
    .map_err(|_| response_error())?;
    let value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    unpack(&value)
}

pub(super) fn usage(value: &Value) -> Result<(), GatewayError> {
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
        return Err(response_error());
    }
    if let (Some(input), Some(output)) = (
        value["input_tokens"].as_u64(),
        value["output_tokens"].as_u64(),
    ) {
        input.checked_add(output).ok_or_else(response_error)?;
    }
    Ok(())
}

pub(super) fn unpack(value: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    validate_native(value)?;
    if !matches!(
        value["stop_reason"].as_str(),
        Some("end_turn" | "stop_sequence" | "max_tokens" | "tool_use")
    ) || value["content"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| !matches!(b["type"].as_str(), Some("text" | "tool_use")))
    {
        return Err(response_error());
    }
    anthropic::unpack_anthropic_response(value).map_err(|_| response_error())
}

pub(super) fn validate_native(value: &Value) -> Result<(), GatewayError> {
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
        return Err(response_error());
    }
    let blocks = value["content"].as_array().ok_or_else(response_error)?;
    if value["stop_reason"] == "tool_use" && !blocks.iter().any(|b| b["type"] == "tool_use") {
        return Err(response_error());
    }
    for block in blocks {
        match native_json::content_type(block)? {
            "text" if !block["text"].is_string() => return Err(response_error()),
            "tool_use" => {
                if !(block["id"].as_str().is_some_and(|s| !s.is_empty())
                    && block["name"].as_str().is_some_and(|s| !s.is_empty())
                    && block["input"].is_object())
                {
                    return Err(response_error());
                }
            }
            // Thinking, signatures, citations and server-tool blocks remain in the original JSON.
            _ => {}
        }
    }
    usage(&value["usage"])
}
