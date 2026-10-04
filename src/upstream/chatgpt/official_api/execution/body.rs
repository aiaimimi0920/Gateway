//! Whole-body admission for official JSON responses and HTTP error diagnostics.

use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_body_with_provider;

pub(super) async fn read_json(
    response: rquest::Response,
    provider: &str,
) -> Result<Value, GatewayError> {
    let bytes = collect_bounded_upstream_body_with_provider(
        response,
        "ChatGPT official API JSON body",
        provider,
    )
    .await?;
    // Keep rquest's byte-based JSON decoder and error type; HTTP charset does not
    // alter JSON bytes. The in-memory response performs no second network read.
    rquest::Response::from(axum::http::Response::new(bytes))
        .json()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))
}
