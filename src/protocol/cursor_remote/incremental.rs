//! Only a delivered, same-socket live tool pause can accept output-only input.
//! This is not stored Responses, a new inference, or a replay/reconnect mechanism.
use super::{runtime::Continuation, tool_request::ToolOutput, tool_schema::invalid};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
};

#[derive(Clone, Debug)]
pub(crate) struct IncrementalRequest {
    pub continuation: Continuation,
    pub output: ToolOutput,
}

pub(crate) fn parse(
    req: &CanonicalRelayRequest,
    continuation: &Continuation,
) -> Result<IncrementalRequest, GatewayError> {
    let body = req.raw_body.as_object().ok_or_else(invalid)?;
    if req.protocol_family != ProtocolFamily::OpenAi
        || req.endpoint_kind != EndpointKind::Responses
        || req.requested_model.as_deref() != Some(crate::routing::config::cursor::MODEL)
        || req.raw_body.to_string().len() > 512 * 1024
    {
        return Err(invalid());
    }
    // Policy is already frozen in the live Run. Even an explicitly empty
    // tools/instructions value is not an instruction to change that policy.
    for (key, value) in body {
        let allowed = match key.as_str() {
            "model" | "input" | "previous_response_id" => true,
            "store" => value == false,
            "stream" => value == true && req.stream,
            _ => false,
        };
        if !allowed {
            return Err(invalid());
        }
    }
    let previous = body
        .get("previous_response_id")
        .and_then(|v| v.as_str())
        .ok_or_else(unavailable)?;
    if req.previous_response_id.as_deref() != Some(previous) || !continuation.references(previous) {
        return Err(unavailable());
    }
    let items = body
        .get("input")
        .and_then(|v| v.as_array())
        .filter(|items| items.len() == 1)
        .ok_or_else(invalid)?;
    let output = ToolOutput::from_item(&items[0])?;
    super::text_output::check_ipc(&output.call_id, &output.text)?;
    Ok(IncrementalRequest {
        continuation: continuation.clone(),
        output,
    })
}

pub(crate) fn unavailable() -> GatewayError {
    crate::routing::config::cursor::stop(
        GatewayError::bad_request(
            "Cursor previous_response_id requires the current delivered tool pause on this WebSocket",
        )
        .with_code("cursor_remote_previous_response_unavailable"),
    )
}
