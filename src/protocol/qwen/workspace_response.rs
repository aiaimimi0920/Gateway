//! Ordinary workspace Chat JSON admission; never choose a wire decoder from the inbound endpoint.
use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayResponse, upstream_body},
};
use serde_json::Value;

const PROVIDER: &str = "openai_compatible";

pub(crate) async fn read(
    response: rquest::Response,
) -> Result<CanonicalRelayResponse, GatewayError> {
    // Use the shared 64 MiB decoded-body budget, not the 64 KiB short management-probe budget.
    let bytes = upstream_body::collect_bounded_upstream_body_with_provider(
        response,
        "Qwen workspace response body",
        PROVIDER,
    )
    .await
    .map_err(|mut error| {
        if matches!(
            error.code.as_deref(),
            Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
        ) {
            // Replaying an already accepted generation cannot repair a local admission failure.
            error.retryable = false;
        }
        error
    })?;
    let body = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    unpack(&body)
}

fn unpack(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    crate::protocol::openai::chat_admission::unpack(body).map_err(|_| invalid())
}

fn invalid() -> GatewayError {
    // Neither JSON parser diagnostics nor arbitrary upstream error/content fields are public diagnostics.
    let mut error = GatewayError::server_error("Invalid Qwen workspace Chat response.")
        .with_code("invalid_qwen_workspace_response")
        .with_provider(PROVIDER);
    error.retryable = false;
    error
}

#[cfg(test)]
#[path = "workspace_response_tests.rs"]
mod tests;
