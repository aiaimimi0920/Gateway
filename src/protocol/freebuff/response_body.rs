use crate::error::{ErrorKind, FallbackHint, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_body_with_provider;
use serde_json::Value;

pub(super) async fn read_chat_response(response: rquest::Response) -> Result<Value, GatewayError> {
    let body = collect_bounded_upstream_body_with_provider(
        response,
        "FreeBuff chat response",
        "freebuff_compatible",
    )
    .await?;
    serde_json::from_slice(&body).map_err(|error| GatewayError {
        // Preserve the former rquest JSON decode error's retry/fallback classification.
        kind: ErrorKind::Unknown,
        message: format!("Failed to decode FreeBuff chat response: {error}"),
        code: None,
        http_status: None,
        retryable: false,
        fallback_hint: FallbackHint::FallbackProvider {
            reason: "Unclassified error; try an alternative provider.".to_string(),
        },
        provider_name: Some("freebuff_compatible".to_string()),
    })
}
