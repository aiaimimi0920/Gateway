//! Candidate pack ownership.

/// Re-pack a [`CanonicalRelayResponse`] into the wire format that the caller
/// expects, based on the protocol family of the original request.
pub(super) fn pack_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    resp: &crate::protocol::canonical::CanonicalRelayResponse,
) -> serde_json::Value {
    use crate::protocol::canonical::{EndpointKind, ProtocolFamily};

    let resp_id = format!("relay-{}", uuid::Uuid::new_v4());
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let reply_model = caller_visible_reply_model(req, &resp.model);

    match (req.protocol_family, req.endpoint_kind) {
        (ProtocolFamily::Anthropic, EndpointKind::Messages) => {
            crate::protocol::anthropic::build_messages_success(
                &resp_id,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
        (_, EndpointKind::Responses) => {
            // Responses API — use the proper Responses API shape.
            crate::protocol::responses::build_responses_success(
                &resp_id,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
        (_, EndpointKind::Completions) => {
            crate::protocol::openai::build_legacy_completions_success(
                &resp_id,
                created_at,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                resp.finish_reason.as_deref(),
            )
        }
        _ => {
            // OpenAI chat completions (default).
            crate::protocol::openai::build_chat_completions_success(
                &resp_id,
                created_at,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
    }
}

pub(super) fn caller_visible_reply_model(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    upstream_model: &str,
) -> String {
    req.requested_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(upstream_model)
        .to_string()
}
