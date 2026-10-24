//! HTTP/network classification and provider body interpretation.

use super::diagnostics::{sanitize_provider_error_message, truncate};
use super::{build_fallback_hint, is_kind_retryable, ErrorKind, FallbackHint, GatewayError};

pub(super) fn kind_from_http_status(status: u16) -> Option<ErrorKind> {
    match status {
        400 => Some(ErrorKind::BadRequest),
        401 | 403 => Some(ErrorKind::Authentication),
        404 => Some(ErrorKind::ModelNotFound),
        429 => Some(ErrorKind::RateLimit),
        500 | 502 | 503 => Some(ErrorKind::ServerError),
        504 => Some(ErrorKind::Timeout),
        _ => None,
    }
}

/// Scan a body string for provider-level keyword signals.
///
/// Priority mirrors the TypeScript KEYWORD_KIND_MAP — more specific patterns
/// first so that "rate limit" beats "service unavailable" when both appear.
pub(super) fn kind_from_body_keywords(body: &str) -> Option<ErrorKind> {
    let lower = body.to_lowercase();

    if lower.contains("rate limit") || lower.contains("rate_limit") || lower.contains("ratelimit") {
        return Some(ErrorKind::RateLimit);
    }
    if lower.contains("context length")
        || lower.contains("too long")
        || lower.contains("maximum context")
        || lower.contains("token limit")
        || lower.contains("tokens exceed")
        || lower.contains("token exceed")
    {
        return Some(ErrorKind::ContextLength);
    }
    if lower.contains("content filter")
        || lower.contains("content_filter")
        || lower.contains("safety")
        || lower.contains("policy violation")
        || lower.contains("moderat")
    {
        return Some(ErrorKind::ContentFilter);
    }
    if lower.contains("quota") || lower.contains("credit") {
        return Some(ErrorKind::InsufficientQuota);
    }
    if lower.contains("service unavailable")
        || lower.contains("overloaded")
        || lower.contains("temporarily unavailable")
    {
        return Some(ErrorKind::ServiceUnavailable);
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return Some(ErrorKind::Timeout);
    }
    if lower.contains("authentication")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("appidnoautherror")
        || lower.contains("token_invalidated")
        || lower.contains("token_revoked")
        || lower.contains("deactivated_workspace")
        || lower.contains("no auth")
    {
        return Some(ErrorKind::Authentication);
    }
    if lower.contains("model not found")
        || lower.contains("does not exist")
        || lower.contains("unknown model")
    {
        return Some(ErrorKind::ModelNotFound);
    }
    None
}

// ---------------------------------------------------------------------------
// Public classification functions
// ---------------------------------------------------------------------------

/// Classify a raw upstream HTTP response into a [`GatewayError`].
///
/// Resolution order: HTTP status → body keyword scan → `Unknown`.
/// For 429 responses the body is also scanned for a numeric `retry_after`
/// (seconds) so the `FallbackHint::Retry.delay_ms` can be more precise.
pub fn classify_upstream_error(
    status: u16,
    body: &str,
    provider_name: Option<&str>,
) -> GatewayError {
    if let Some(error) = super::html_rejection::classify(status, body, provider_name) {
        return error;
    }
    if let Some(error) =
        super::stream_initialization_rejection::classify(status, body, provider_name)
    {
        return error;
    }
    let status_kind = kind_from_http_status(status);
    let body_kind = kind_from_body_keywords(body);
    let code = extract_code_from_body(body);
    let raw_message = extract_message_from_body(body);

    // Most status mappings are authoritative, but provider-specific 400s are
    // often overloaded. If the body carries a more specific semantic signal,
    // prefer it over the generic bad-request bucket.
    let kind =
        if super::is_account_balance_rejection(status, raw_message.as_deref().unwrap_or(body)) {
            ErrorKind::InsufficientQuota
        } else if super::is_model_capacity(status, code.as_deref(), raw_message.as_deref()) {
            ErrorKind::ModelCapacity
        } else {
            match (status_kind, body_kind) {
                (Some(ErrorKind::BadRequest), Some(body_kind))
                    if body_kind != ErrorKind::BadRequest =>
                {
                    body_kind
                }
                (Some(status_kind), _) => status_kind,
                (None, Some(body_kind)) => body_kind,
                (None, None) => ErrorKind::Unknown,
            }
        };

    // For rate-limit responses, try to parse a retry-after value from the body.
    let upstream_retry_after_ms = matches!(kind, ErrorKind::RateLimit | ErrorKind::ModelCapacity)
        .then(|| parse_retry_after_from_body(body))
        .flatten();
    // An upstream 401 rejects this credential, not the caller's Gateway identity.
    // Switch candidates without retrying the same rejected credential.
    let fallback_hint = if status == 401 {
        FallbackHint::FallbackProvider {
            reason: "Upstream rejected authentication; try another candidate.".to_string(),
        }
    } else if matches!(kind, ErrorKind::RateLimit | ErrorKind::ModelCapacity) {
        let delay_ms = upstream_retry_after_ms.unwrap_or(if kind == ErrorKind::ModelCapacity {
            2_000
        } else {
            5_000
        });
        FallbackHint::Retry {
            delay_ms,
            reason: if kind == ErrorKind::ModelCapacity {
                "Model capacity exhausted; back-off before retrying."
            } else {
                "Rate limit hit; back-off before retrying."
            }
            .to_string(),
        }
    } else {
        build_fallback_hint(kind)
    };

    let message = raw_message
        .map(|raw_message| sanitize_provider_error_message(&raw_message))
        .unwrap_or_else(|| truncate(&sanitize_provider_error_message(body), 200).to_string());

    let mut error = GatewayError {
        kind,
        message,
        code,
        http_status: Some(status),
        retryable: is_kind_retryable(kind),
        fallback_hint,
        provider_name: provider_name.map(str::to_string),
        upstream_retry_after_ms,
    };
    if super::is_channel_unavailable(&error) {
        error.retryable = false;
        error.fallback_hint = FallbackHint::FallbackProvider {
            reason: "No available model channel; cool this candidate and try another.".into(),
        };
    }
    error
}

/// Classify a [`rquest::Error`] (network / transport level) into a [`GatewayError`].
pub fn classify_network_error(err: &rquest::Error, provider_name: Option<&str>) -> GatewayError {
    #[cfg(feature = "cpa-rust-transport")]
    if gateway_cpa_http_transport::is_body_limit_error(err) {
        let mut error =
            GatewayError::server_error("CPA upstream body exceeded the configured limit")
                .with_code("upstream_body_too_large");
        error.provider_name = provider_name.map(str::to_owned);
        return error;
    }
    let (kind, raw_message) = if err.is_timeout() {
        (ErrorKind::Timeout, format!("Request timed out: {err}"))
    } else if err.is_connect() {
        (ErrorKind::Network, format!("Connection error: {err}"))
    } else {
        (ErrorKind::Unknown, format!("Network error: {err}"))
    };
    let message = sanitize_provider_error_message(&raw_message);

    GatewayError {
        kind,
        message,
        code: None,
        http_status: err
            .status()
            .map(|s| s.as_u16())
            .or_else(|| err.is_timeout().then_some(504)),
        retryable: is_kind_retryable(kind),
        fallback_hint: build_fallback_hint(kind),
        provider_name: provider_name.map(str::to_string),
        upstream_retry_after_ms: None,
    }
}

/// Keep native transport classification and strip URLs before diagnostics can expose query secrets.
pub(crate) fn classify_native_network_error(err: reqwest::Error, provider: &str) -> GatewayError {
    let kind = if err.is_timeout() {
        ErrorKind::Timeout
    } else if err.is_connect() {
        ErrorKind::Network
    } else {
        ErrorKind::Unknown
    };
    let status = err
        .status()
        .map(|s| s.as_u16())
        .or_else(|| err.is_timeout().then_some(504));
    GatewayError {
        kind,
        message: sanitize_provider_error_message(&format!(
            "Native transport error: {}",
            err.without_url()
        )),
        code: None,
        http_status: status,
        retryable: is_kind_retryable(kind),
        fallback_hint: build_fallback_hint(kind),
        provider_name: Some(provider.to_owned()),
        upstream_retry_after_ms: None,
    }
}

// ---------------------------------------------------------------------------
// Body parsing helpers
// ---------------------------------------------------------------------------

/// Try to extract a human-readable message from a JSON response body.
///
/// Handles the two most common shapes:
/// - `{ "error": { "message": "..." } }` (OpenAI-compatible)
/// - `{ "message": "..." }`
fn extract_message_from_body(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;

    // Nested error.message first (OpenAI / Anthropic style).
    if let Some(msg) = v
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Top-level message.
    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

/// Try to extract an error code from a JSON response body.
fn extract_code_from_body(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;

    for key in &["code", "error_code", "errorCode"] {
        // Top-level.
        if let Some(code) = v.get(key).and_then(|c| c.as_str()) {
            let trimmed = code.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        // Nested inside `error` object.
        if let Some(code) = v
            .get("error")
            .and_then(|e| e.get(key))
            .and_then(|c| c.as_str())
        {
            let trimmed = code.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

/// Try to parse a `retry_after` (seconds) from the body and convert to ms.
///
/// Handles `{ "retry_after": 5 }` and `{ "error": { "retry_after": 5 } }`.
fn parse_retry_after_from_body(body: &str) -> Option<u64> {
    super::retry_after::parse_body(body)
}
/// Transport-owned static messages cannot accidentally include proxy or URL credentials.
pub(crate) fn classify_native_transport_failure(timeout: bool, provider: &str) -> GatewayError {
    let kind = if timeout {
        ErrorKind::Timeout
    } else {
        ErrorKind::Network
    };
    GatewayError {
        kind,
        message: if timeout {
            "Native WebSocket connection timed out"
        } else {
            "Native WebSocket connection failed"
        }
        .into(),
        code: None,
        http_status: timeout.then_some(504),
        retryable: is_kind_retryable(kind),
        fallback_hint: build_fallback_hint(kind),
        provider_name: Some(provider.into()),
        upstream_retry_after_ms: None,
    }
}
