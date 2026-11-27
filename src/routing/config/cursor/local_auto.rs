//! Personal official CLI candidate, distinct from ACP and remote account import.
//! No process owner or runtime switch exists until isolation and billing are proven.
use crate::{
    error::{FallbackHint, GatewayError},
    protocol::canonical::{CanonicalRelayRequest, EndpointKind, MessageRole},
};

const PREFIX: &str = "cursor-local/";
const MODEL: &str = "cursor-local/auto";

pub(crate) fn reserved(model: &str) -> bool {
    model
        .trim()
        .get(..PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(PREFIX))
}

/// Returning an error here must not enter any other account, model or worker.
pub(crate) fn reject(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    let Some(model) = req.requested_model.as_deref().filter(|m| reserved(m)) else {
        return Ok(());
    };
    let unsupported =
        |message| GatewayError::bad_request(message).with_code("cursor_local_unsupported");
    let body = &req.raw_body;
    let mut error = if !model.trim().eq_ignore_ascii_case(MODEL) {
        unsupported("Only Auto is a personal Cursor CLI candidate; no substitute model is allowed.")
    } else if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Responses
    ) {
        unsupported("The personal Cursor CLI candidate does not support this protocol.")
    } else if req.previous_response_id.is_some()
        || !body["conversation"].is_null()
        || !body["session_id"].is_null()
        || body["store"] == true
    {
        unsupported(
            "The personal Cursor CLI candidate does not support stored or continued sessions.",
        )
    } else if ["tools", "functions"].iter().any(|k| {
        let field = &body[*k];
        !field.is_null() && !field.as_array().is_some_and(Vec::is_empty)
    }) || ["tool_choice", "function_call"].iter().any(|k| {
        let field = &body[*k];
        !field.is_null() && !matches!(field.as_str(), Some("auto" | "none"))
    }) || req
        .messages
        .iter()
        .any(|m| m.role == MessageRole::Tool || !m.tool_calls.is_empty())
    {
        unsupported("The personal Cursor CLI candidate does not support tools or tool results.")
    } else if req.explicit_session_key.is_some() {
        unsupported(
            "The personal Cursor CLI candidate does not support explicit user/session routing.",
        )
    } else if req.messages.len() > 1 || req.messages.iter().any(|m| m.role != MessageRole::User) {
        // A first turn can contain instructions and a user message without any predecessor.
        unsupported("The personal Cursor CLI candidate accepts only a single user message, without additional instructions or history.")
    } else {
        GatewayError::service_unavailable(
            "Personal Cursor Auto CLI dispatch is disabled. No other provider will be used.",
        )
        .with_code("cursor_local_auto_disabled")
    };
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "No substitute for a personal local CLI request".into(),
    };
    Err(error)
}
