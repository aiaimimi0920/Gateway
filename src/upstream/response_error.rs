//! Common HTTP error ownership: retain wait hints before consuming a bounded body.
use std::time::SystemTime;

use crate::error::{classify_upstream_error, retry_after_from_headers, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_charset_text_with_provider;

pub(crate) async fn classify_response_error(
    response: rquest::Response,
    provider: &str,
    body_label: &'static str,
) -> GatewayError {
    let status = response.status().as_u16();
    let delay = retry_after_from_headers(response.headers(), SystemTime::now());
    let body =
        match collect_bounded_upstream_charset_text_with_provider(response, body_label, provider)
            .await
        {
            Ok(body) => body,
            // Preserve resource-admission failures rather than disguising them as an
            // upstream HTTP status. Ordinary truncated bodies keep the legacy fallback.
            Err(error)
                if matches!(
                    error.code.as_deref(),
                    Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
                ) =>
            {
                return error
            }
            Err(_) => String::from("<unreadable body>"),
        };
    classify_upstream_error(status, &body, Some(provider)).with_retry_after_ms(delay)
}

#[cfg(test)]
mod tests;
