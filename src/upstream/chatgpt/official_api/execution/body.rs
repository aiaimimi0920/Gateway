//! Whole-body admission for official JSON responses and HTTP error diagnostics.

use serde_json::Value;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::upstream_body::{
    collect_bounded_upstream_body_with_provider,
    collect_bounded_upstream_charset_text_with_provider,
};

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

pub(super) async fn read_error(
    response: rquest::Response,
    provider: &str,
) -> Result<String, GatewayError> {
    match collect_bounded_upstream_charset_text_with_provider(
        response,
        "ChatGPT official API error body",
        provider,
    )
    .await
    {
        Ok(body) => Ok(body),
        // Resource admission failures must not disappear into the legacy
        // transport-read fallback and then be classified as an upstream status.
        Err(error)
            if matches!(
                error.code.as_deref(),
                Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
            ) =>
        {
            Err(error)
        }
        Err(_) => Ok(String::from("<unreadable body>")),
    }
}
