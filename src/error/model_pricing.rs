//! Known relay pricing compilation failures occur at admission, before inference.
use super::{ErrorKind, FallbackHint, GatewayError};

pub(crate) fn is_model_pricing_failure(
    status: u16,
    code: Option<&str>,
    message: Option<&str>,
) -> bool {
    if !matches!(status, 400 | 500 | 502 | 503)
        || !matches!(code.map(str::trim), None | Some("" | "model_price_error"))
    {
        return false;
    }
    // Only primary error text is supplied by the HTTP/SSE owners. Never scan
    // echoed prompts or broaden an explicit auth/validation error into replay.
    let Some(message) = message.filter(|text| text.len() <= 4096) else {
        return false;
    };
    let Some((model, detail)) = message
        .trim()
        .strip_prefix("model ")
        .and_then(|text| text.split_once(" tiered expr run failed: expr compile error:"))
    else {
        return false;
    };
    !model.is_empty()
        && model.len() <= 256
        && model.bytes().all(|byte| byte.is_ascii_graphic())
        && !detail.trim().is_empty()
}

pub(super) fn classify(
    status: u16,
    code: Option<&str>,
    message: &str,
    provider: Option<&str>,
) -> Option<GatewayError> {
    if !is_model_pricing_failure(status, code, Some(message)) {
        return None;
    }
    Some(GatewayError {
        kind: ErrorKind::ServerError,
        message: super::sanitize_provider_error_message(message),
        code: code.map(str::to_owned),
        http_status: Some(status),
        // A configuration fault cannot recover by retrying the same candidate.
        retryable: false,
        fallback_hint: FallbackHint::FallbackProvider {
            reason: "Upstream model pricing compilation failed; try another eligible candidate."
                .into(),
        },
        provider_name: provider.map(str::to_owned),
        upstream_retry_after_ms: None,
        native_transport: None,
    })
}

pub(super) fn is_admission_rejection(error: &GatewayError) -> bool {
    // A self-reported code alone must not grant a tool-bearing request a send slot.
    error.kind == ErrorKind::ServerError
        && !error.retryable
        && error.should_fallback()
        && error.http_status.is_some_and(|status| {
            is_model_pricing_failure(status, error.code.as_deref(), Some(&error.message))
        })
}
