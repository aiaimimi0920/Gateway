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
    let body = crate::protocol::native_text_request::pack(req, model, stream, invalid)?;
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
    anthropic::native_admission::validate(value).map_err(|_| response_error())
}
