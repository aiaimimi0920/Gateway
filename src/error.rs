// ---------------------------------------------------------------------------
// GatewayError — structured error classification with FallbackHint
//
// Rust port of packages/ai-gateway-domain/src/modules/gateway/standard-error.ts
// ---------------------------------------------------------------------------

use axum::http::{header::RETRY_AFTER, HeaderValue, StatusCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use thiserror::Error;

/// Maximum number of characters retained in an upstream/provider error
/// message.  Error messages can cross API, log, and database boundaries, so
/// they must have a deterministic upper bound even when a provider returns a
/// very large response body.
pub const PROVIDER_ERROR_MESSAGE_MAX_CHARS: usize = 512;

const REDACTED_PROVIDER_VALUE: &str = "[REDACTED]";

fn provider_sensitive_header_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(authorization|proxy-authorization|cookie|set-cookie)\b["']?\s*[:=]\s*(?:"[^"]*"|'[^'\r\n]*'|[^\r\n,}]+)"#,
        )
        .expect("provider sensitive header regex must compile")
    })
}

fn provider_auth_scheme_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)\b(bearer|basic)\s+[^\s,;}\]\)"']+"#)
            .expect("provider auth scheme regex must compile")
    })
}

fn provider_jwt_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"\beyJ[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\b"#)
            .expect("provider JWT regex must compile")
    })
}

fn provider_prefixed_key_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)\bsk-[A-Za-z0-9_-]{6,}\b"#)
            .expect("provider prefixed key regex must compile")
    })
}

fn provider_assignment_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(x[-_ ]?api[-_ ]?key|api[-_ ]?key|apikey|access[-_ ]?token|refresh[-_ ]?token|id[-_ ]?token|auth[-_ ]?token|token|session(?:[-_ ]?(?:id|token|key))?|secret(?:[-_ ]?key)?)\b["']?\s*[:=]\s*(?:"[^"]*"|'[^'\r\n]*'|[^\s,;&}\)]+)"#,
        )
        .expect("provider assignment regex must compile")
    })
}

/// Sanitize a provider/upstream error message before it crosses an external
/// or durable boundary. Classification must happen on the raw body first;
/// this helper only changes the human-readable message and therefore keeps
/// the original error kind/code decisions intact.
pub fn sanitize_provider_error_message(message: &str) -> String {
    let mut sanitized = provider_sensitive_header_pattern()
        .replace_all(message, |captures: &regex::Captures<'_>| {
            format!("{}: {REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();
    sanitized = provider_auth_scheme_pattern()
        .replace_all(&sanitized, |captures: &regex::Captures<'_>| {
            format!("{} {REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();
    sanitized = provider_jwt_pattern()
        .replace_all(&sanitized, REDACTED_PROVIDER_VALUE)
        .into_owned();
    sanitized = provider_prefixed_key_pattern()
        .replace_all(&sanitized, REDACTED_PROVIDER_VALUE)
        .into_owned();
    sanitized = provider_assignment_pattern()
        .replace_all(&sanitized, |captures: &regex::Captures<'_>| {
            format!("{}={REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();

    // Remove control characters so a provider cannot inject additional log
    // lines or malformed response framing through an error message.
    let sanitized: String = sanitized
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim();
    if sanitized.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS {
        return sanitized.to_string();
    }

    let suffix = "...";
    let prefix_limit = PROVIDER_ERROR_MESSAGE_MAX_CHARS.saturating_sub(suffix.len());
    format!("{}{}", truncate(sanitized, prefix_limit).trim_end(), suffix)
}

// ---------------------------------------------------------------------------
// ErrorKind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Authentication,
    RateLimit,
    ContextLength,
    ContentFilter,
    ModelNotFound,
    ServerError,
    Network,
    BadRequest,
    InsufficientQuota,
    ServiceUnavailable,
    Timeout,
    Unknown,
}

// ---------------------------------------------------------------------------
// FallbackHint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FallbackHint {
    Retry {
        delay_ms: u64,
        reason: String,
    },
    FallbackProvider {
        reason: String,
    },
    DowngradeModel {
        suggested_model: Option<String>,
        reason: String,
    },
    Abort {
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// GatewayError
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
#[error("[{kind:?}] {message}")]
pub struct GatewayError {
    pub kind: ErrorKind,
    pub message: String,
    pub code: Option<String>,
    pub http_status: Option<u16>,
    pub retryable: bool,
    pub fallback_hint: FallbackHint,
    pub provider_name: Option<String>,
}

impl GatewayError {
    // ── constructor helpers ──────────────────────────────────────────────

    pub fn bad_request(message: impl Into<String>) -> Self {
        let kind = ErrorKind::BadRequest;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(400),
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        let kind = ErrorKind::Authentication;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(401),
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        let kind = ErrorKind::ModelNotFound;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(404),
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        let kind = ErrorKind::BadRequest;
        Self {
            kind,
            message: message.into(),
            code: Some("conflict".to_string()),
            http_status: Some(409),
            retryable: false,
            fallback_hint: FallbackHint::Abort {
                reason: "The requested state conflicts with the current resource state."
                    .to_string(),
            },
            provider_name: None,
        }
    }

    pub fn rate_limited(message: impl Into<String>, delay_ms: u64) -> Self {
        let kind = ErrorKind::RateLimit;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(429),
            retryable: is_kind_retryable(kind),
            fallback_hint: FallbackHint::Retry {
                delay_ms,
                reason: "Rate limit hit; back-off before retrying.".to_string(),
            },
            provider_name: None,
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        let kind = ErrorKind::ServiceUnavailable;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(503),
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn server_error(message: impl Into<String>) -> Self {
        let kind = ErrorKind::ServerError;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: Some(500),
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn content_filtered(message: impl Into<String>) -> Self {
        let kind = ErrorKind::ContentFilter;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: None,
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    pub fn quota_exceeded(message: impl Into<String>) -> Self {
        let kind = ErrorKind::InsufficientQuota;
        Self {
            kind,
            message: message.into(),
            code: None,
            http_status: None,
            retryable: is_kind_retryable(kind),
            fallback_hint: build_fallback_hint(kind),
            provider_name: None,
        }
    }

    // ── query helpers ────────────────────────────────────────────────────

    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    pub fn should_fallback(&self) -> bool {
        matches!(self.fallback_hint, FallbackHint::FallbackProvider { .. })
    }

    /// Attach a provider name (builder style).
    pub fn with_provider(mut self, name: impl Into<String>) -> Self {
        self.provider_name = Some(name.into());
        self
    }

    /// Attach an error code (builder style).
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

// ---------------------------------------------------------------------------
// axum IntoResponse
// ---------------------------------------------------------------------------

impl axum::response::IntoResponse for GatewayError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.http_status {
            Some(s) => StatusCode::from_u16(s).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            None => match self.kind {
                ErrorKind::Authentication => StatusCode::UNAUTHORIZED,
                ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
                ErrorKind::RateLimit => StatusCode::TOO_MANY_REQUESTS,
                ErrorKind::ModelNotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
        };

        let retry_after_seconds = match &self.fallback_hint {
            FallbackHint::Retry { delay_ms, .. } if self.kind == ErrorKind::RateLimit => {
                Some(delay_ms.saturating_add(999) / 1_000)
            }
            _ => None,
        };
        let body = serde_json::json!({
            "error": {
                "message": self.message,
                "type": format!("{:?}", self.kind),
                "code": self.code,
            }
        });

        let mut response = (status, axum::Json(body)).into_response();
        if let Some(retry_after_seconds) = retry_after_seconds {
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.max(1).to_string()) {
                response.headers_mut().insert(RETRY_AFTER, value);
            }
        }
        response
    }
}

// ---------------------------------------------------------------------------
// Internal helpers — kind determination
// ---------------------------------------------------------------------------

fn kind_from_http_status(status: u16) -> Option<ErrorKind> {
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
fn kind_from_body_keywords(body: &str) -> Option<ErrorKind> {
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

fn is_kind_retryable(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::RateLimit
            | ErrorKind::ServerError
            | ErrorKind::Network
            | ErrorKind::ServiceUnavailable
            | ErrorKind::Timeout
    )
}

fn build_fallback_hint(kind: ErrorKind) -> FallbackHint {
    match kind {
        ErrorKind::RateLimit => FallbackHint::Retry {
            delay_ms: 5_000,
            reason: "Rate limit hit; back-off before retrying.".to_string(),
        },
        ErrorKind::InsufficientQuota => FallbackHint::FallbackProvider {
            reason: "Account quota exhausted; switch to an alternative provider.".to_string(),
        },
        ErrorKind::ContextLength => FallbackHint::DowngradeModel {
            suggested_model: None,
            reason:
                "Prompt exceeds context window; retry with a model that supports a larger context."
                    .to_string(),
        },
        ErrorKind::Authentication => FallbackHint::Abort {
            reason: "Invalid or missing credentials; manual intervention required.".to_string(),
        },
        ErrorKind::ServerError => FallbackHint::FallbackProvider {
            reason: "Provider returned a server error; try an alternative provider.".to_string(),
        },
        ErrorKind::Network => FallbackHint::FallbackProvider {
            reason: "Network failure reaching provider; try an alternative provider.".to_string(),
        },
        ErrorKind::ContentFilter => FallbackHint::Abort {
            reason: "Request was blocked by content policy; the prompt must be revised."
                .to_string(),
        },
        ErrorKind::BadRequest => FallbackHint::Abort {
            reason: "Malformed request; fix the request before retrying.".to_string(),
        },
        ErrorKind::ModelNotFound => FallbackHint::DowngradeModel {
            suggested_model: None,
            reason: "Requested model does not exist on this provider; select an available model."
                .to_string(),
        },
        ErrorKind::ServiceUnavailable => FallbackHint::Retry {
            delay_ms: 2_000,
            reason: "Provider temporarily unavailable; retry after a brief delay.".to_string(),
        },
        ErrorKind::Timeout => FallbackHint::Retry {
            delay_ms: 1_000,
            reason: "Request timed out; retry with a shorter prompt or increased timeout."
                .to_string(),
        },
        ErrorKind::Unknown => FallbackHint::FallbackProvider {
            reason: "Unclassified error; try an alternative provider.".to_string(),
        },
    }
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
    let status_kind = kind_from_http_status(status);
    let body_kind = kind_from_body_keywords(body);

    // Most status mappings are authoritative, but provider-specific 400s are
    // often overloaded. If the body carries a more specific semantic signal,
    // prefer it over the generic bad-request bucket.
    let kind = match (status_kind, body_kind) {
        (Some(ErrorKind::BadRequest), Some(body_kind)) if body_kind != ErrorKind::BadRequest => {
            body_kind
        }
        (Some(status_kind), _) => status_kind,
        (None, Some(body_kind)) => body_kind,
        (None, None) => ErrorKind::Unknown,
    };

    // For rate-limit responses, try to parse a retry-after value from the body.
    let fallback_hint = if kind == ErrorKind::RateLimit {
        let delay_ms = parse_retry_after_from_body(body).unwrap_or(5_000);
        FallbackHint::Retry {
            delay_ms,
            reason: "Rate limit hit; back-off before retrying.".to_string(),
        }
    } else {
        build_fallback_hint(kind)
    };

    let message = extract_message_from_body(body)
        .map(|raw_message| sanitize_provider_error_message(&raw_message))
        .unwrap_or_else(|| truncate(&sanitize_provider_error_message(body), 200).to_string());

    GatewayError {
        kind,
        message,
        code: extract_code_from_body(body),
        http_status: Some(status),
        retryable: is_kind_retryable(kind),
        fallback_hint,
        provider_name: provider_name.map(str::to_string),
    }
}

/// Classify a [`rquest::Error`] (network / transport level) into a [`GatewayError`].
pub fn classify_network_error(err: &rquest::Error, provider_name: Option<&str>) -> GatewayError {
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
        http_status: err.status().map(|s| s.as_u16()),
        retryable: is_kind_retryable(kind),
        fallback_hint: build_fallback_hint(kind),
        provider_name: provider_name.map(str::to_string),
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
    let v: serde_json::Value = serde_json::from_str(body).ok()?;

    let candidate = v
        .get("retry_after")
        .or_else(|| v.get("retryAfter"))
        .or_else(|| v.get("error").and_then(|e| e.get("retry_after")))
        .or_else(|| v.get("error").and_then(|e| e.get("retryAfter")))?;

    let secs = candidate.as_f64()?;
    if secs >= 0.0 {
        Some((secs * 1_000.0).floor() as u64)
    } else {
        None
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzZWNyZXQtdXNlciJ9.c2lnbmF0dXJl";

    #[test]
    fn provider_error_message_sanitizer_redacts_secrets_and_bounds_output() {
        let raw = format!(
            "invalid api key\nAuthorization: Bearer bearer-secret-value\nBasic dXNlcjpwYXNzd29yZA==\napi_key=sk-proj-super-secret\ntoken='plain-token-secret'\ntoken=Bearer bearer-assignment-secret\nCookie: session=private-session; csrftoken=private-cookie\njwt={TEST_JWT}\n{}",
            "x".repeat(PROVIDER_ERROR_MESSAGE_MAX_CHARS * 2)
        );

        let sanitized = sanitize_provider_error_message(&raw);

        for secret in [
            "bearer-secret-value",
            "dXNlcjpwYXNzd29yZA==",
            "sk-proj-super-secret",
            "plain-token-secret",
            "bearer-assignment-secret",
            "private-session",
            "private-cookie",
            TEST_JWT,
        ] {
            assert!(
                !sanitized.contains(secret),
                "sanitized provider error leaked {secret}: {sanitized}"
            );
        }
        assert!(sanitized.contains("invalid api key"));
        assert!(sanitized.contains("[REDACTED]"));
        assert!(sanitized.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS);
    }

    // ── retryability ─────────────────────────────────────────────────────

    #[test]
    fn retryable_kinds_are_marked_retryable() {
        for kind in [
            ErrorKind::RateLimit,
            ErrorKind::ServerError,
            ErrorKind::Network,
            ErrorKind::ServiceUnavailable,
            ErrorKind::Timeout,
        ] {
            assert!(is_kind_retryable(kind), "{kind:?} should be retryable");
        }
    }

    #[test]
    fn non_retryable_kinds_are_not_retryable() {
        for kind in [
            ErrorKind::Authentication,
            ErrorKind::BadRequest,
            ErrorKind::ContentFilter,
            ErrorKind::ContextLength,
            ErrorKind::InsufficientQuota,
            ErrorKind::ModelNotFound,
            ErrorKind::Unknown,
        ] {
            assert!(!is_kind_retryable(kind), "{kind:?} should NOT be retryable");
        }
    }

    // ── kind_from_http_status ────────────────────────────────────────────

    #[test]
    fn http_status_mapping_covers_common_codes() {
        assert_eq!(kind_from_http_status(400), Some(ErrorKind::BadRequest));
        assert_eq!(kind_from_http_status(401), Some(ErrorKind::Authentication));
        assert_eq!(kind_from_http_status(403), Some(ErrorKind::Authentication));
        assert_eq!(kind_from_http_status(404), Some(ErrorKind::ModelNotFound));
        assert_eq!(kind_from_http_status(429), Some(ErrorKind::RateLimit));
        assert_eq!(kind_from_http_status(500), Some(ErrorKind::ServerError));
        assert_eq!(kind_from_http_status(502), Some(ErrorKind::ServerError));
        assert_eq!(kind_from_http_status(503), Some(ErrorKind::ServerError));
        assert_eq!(kind_from_http_status(504), Some(ErrorKind::Timeout));
    }

    #[test]
    fn http_status_returns_none_for_unmapped_codes() {
        assert_eq!(kind_from_http_status(200), None);
        assert_eq!(kind_from_http_status(418), None);
    }

    // ── keyword scanning ─────────────────────────────────────────────────

    #[test]
    fn keyword_detects_rate_limit() {
        assert_eq!(
            kind_from_body_keywords("You have exceeded your rate limit"),
            Some(ErrorKind::RateLimit)
        );
        assert_eq!(
            kind_from_body_keywords("rate_limit exceeded"),
            Some(ErrorKind::RateLimit)
        );
    }

    #[test]
    fn keyword_detects_context_length() {
        assert_eq!(
            kind_from_body_keywords("prompt too long for context length"),
            Some(ErrorKind::ContextLength)
        );
        assert_eq!(
            kind_from_body_keywords("tokens exceed the maximum"),
            Some(ErrorKind::ContextLength)
        );
    }

    #[test]
    fn keyword_detects_content_filter() {
        assert_eq!(
            kind_from_body_keywords("content filter triggered"),
            Some(ErrorKind::ContentFilter)
        );
        assert_eq!(
            kind_from_body_keywords("safety policy violation"),
            Some(ErrorKind::ContentFilter)
        );
    }

    #[test]
    fn keyword_returns_none_for_benign_text() {
        assert_eq!(kind_from_body_keywords("everything is fine"), None);
    }

    // ── constructor helpers ──────────────────────────────────────────────

    #[test]
    fn bad_request_constructor_sets_correct_fields() {
        let e = GatewayError::bad_request("missing field");
        assert_eq!(e.kind, ErrorKind::BadRequest);
        assert_eq!(e.http_status, Some(400));
        assert!(!e.retryable);
        assert!(matches!(e.fallback_hint, FallbackHint::Abort { .. }));
    }

    #[test]
    fn rate_limited_constructor_uses_supplied_delay() {
        let e = GatewayError::rate_limited("slow down", 3_000);
        assert_eq!(e.kind, ErrorKind::RateLimit);
        assert_eq!(e.http_status, Some(429));
        assert!(e.retryable);
        match &e.fallback_hint {
            FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 3_000),
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn server_error_is_retryable_and_triggers_fallback() {
        let e = GatewayError::server_error("internal failure");
        assert_eq!(e.kind, ErrorKind::ServerError);
        assert!(e.retryable);
        assert!(e.should_fallback());
    }

    #[test]
    fn unauthorized_is_not_retryable_and_aborts() {
        let e = GatewayError::unauthorized("bad key");
        assert!(!e.is_retryable());
        assert!(!e.should_fallback());
        assert!(matches!(e.fallback_hint, FallbackHint::Abort { .. }));
    }

    // ── classify_upstream_error ──────────────────────────────────────────

    #[test]
    fn classify_upstream_401_parses_openai_body() {
        let body = r#"{"error":{"message":"Invalid API key","code":"invalid_api_key"}}"#;
        let e = classify_upstream_error(401, body, Some("openai"));
        assert_eq!(e.kind, ErrorKind::Authentication);
        assert_eq!(e.provider_name.as_deref(), Some("openai"));
        assert_eq!(e.message, "Invalid API key");
        assert_eq!(e.code.as_deref(), Some("invalid_api_key"));
        assert!(!e.retryable);
    }

    #[test]
    fn classify_upstream_error_redacts_message_without_losing_classification() {
        let body = r#"{"error":{"message":"Invalid API key sk-proj-upstream-secret; token=secondary-secret","code":"invalid_api_key"}}"#;

        let e = classify_upstream_error(401, body, Some("openai"));

        assert_eq!(e.kind, ErrorKind::Authentication);
        assert_eq!(e.code.as_deref(), Some("invalid_api_key"));
        assert!(e.message.contains("Invalid API key"));
        assert!(!e.message.contains("sk-proj-upstream-secret"));
        assert!(!e.message.contains("secondary-secret"));
    }

    #[test]
    fn classify_upstream_sanitizes_before_truncating_a_long_jwt() {
        let body = format!(
            "eyJ{}.{}.{}",
            "a".repeat(100),
            "b".repeat(100),
            "signature-secret-value"
        );

        let e = classify_upstream_error(418, &body, None);

        assert!(!e.message.contains("eyJ"));
        assert!(!e.message.contains("signature-secret-value"));
        assert!(e.message.contains("[REDACTED]"));
    }

    #[test]
    fn classify_network_error_redacts_secrets_embedded_in_url() {
        let invalid_header = rquest::header::HeaderValue::from_bytes(b"\n")
            .expect_err("newline must be rejected as a header value");
        let error = rquest::Error::from(invalid_header).with_url(
            rquest::Url::parse("https://provider.invalid/models?token=network-secret-value")
                .expect("test URL must parse"),
        );

        let classified = classify_network_error(&error, Some("test-provider"));

        assert_eq!(classified.kind, ErrorKind::Unknown);
        assert!(!classified.message.contains("network-secret-value"));
        assert!(classified.message.contains("token=[REDACTED]"));
        assert!(classified.message.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS);
    }

    #[test]
    fn classify_upstream_429_uses_retry_after_from_body() {
        let body = r#"{"error":{"message":"Rate limited","retry_after":10}}"#;
        let e = classify_upstream_error(429, body, None);
        assert_eq!(e.kind, ErrorKind::RateLimit);
        assert!(e.retryable);
        match &e.fallback_hint {
            FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 10_000),
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn classify_upstream_429_falls_back_to_default_delay() {
        let body = r#"{"error":{"message":"Rate limited"}}"#;
        let e = classify_upstream_error(429, body, None);
        match &e.fallback_hint {
            FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 5_000),
            other => panic!("expected Retry, got {other:?}"),
        }
    }

    #[test]
    fn classify_upstream_500_triggers_provider_fallback() {
        let body = r#"{"message":"Internal server error"}"#;
        let e = classify_upstream_error(500, body, Some("anthropic"));
        assert_eq!(e.kind, ErrorKind::ServerError);
        assert!(e.retryable);
        assert!(e.should_fallback());
    }

    #[test]
    fn classify_upstream_falls_through_to_keyword_scan_on_unmapped_status() {
        // 418 has no status mapping — fall through to body keyword scan.
        let body = "quota exceeded for this billing cycle";
        let e = classify_upstream_error(418, body, None);
        assert_eq!(e.kind, ErrorKind::InsufficientQuota);
    }

    #[test]
    fn classify_upstream_fully_unknown_when_no_signals() {
        let e = classify_upstream_error(418, "I'm a teapot", None);
        assert_eq!(e.kind, ErrorKind::Unknown);
        assert!(!e.retryable);
    }

    #[test]
    fn classify_upstream_deactivated_workspace_as_authentication() {
        let e =
            classify_upstream_error(402, r#"{"detail":{"code":"deactivated_workspace"}}"#, None);
        assert_eq!(e.kind, ErrorKind::Authentication);
        assert!(!e.retryable);
    }

    #[test]
    fn classify_upstream_token_invalidated_as_authentication() {
        let e = classify_upstream_error(
            401,
            r#"{"error":{"message":"Your authentication token has been invalidated.","code":"token_invalidated"}}"#,
            None,
        );
        assert_eq!(e.kind, ErrorKind::Authentication);
        assert!(!e.retryable);
    }

    #[test]
    fn classify_upstream_appid_no_auth_error_as_authentication() {
        let e = classify_upstream_error(
            400,
            "AppIdNoAuthError: xop35qwen2b is not authorized for this appid",
            Some("xfyun"),
        );
        assert_eq!(e.kind, ErrorKind::Authentication);
        assert!(!e.retryable);
    }

    // ── builder extensions ───────────────────────────────────────────────

    #[test]
    fn builder_methods_attach_fields() {
        let e = GatewayError::server_error("boom")
            .with_provider("groq")
            .with_code("EINTERNAL");
        assert_eq!(e.provider_name.as_deref(), Some("groq"));
        assert_eq!(e.code.as_deref(), Some("EINTERNAL"));
    }

    // ── FallbackHint serialisation ───────────────────────────────────────

    #[test]
    fn fallback_hint_retry_serialises_action_tag() {
        let hint = FallbackHint::Retry {
            delay_ms: 500,
            reason: "test".to_string(),
        };
        let json = serde_json::to_value(&hint).unwrap();
        assert_eq!(json["action"], "retry");
        assert_eq!(json["delay_ms"], 500);
    }

    #[test]
    fn fallback_hint_fallback_provider_serialises_action_tag() {
        let hint = FallbackHint::FallbackProvider {
            reason: "provider down".to_string(),
        };
        let json = serde_json::to_value(&hint).unwrap();
        assert_eq!(json["action"], "fallback_provider");
    }

    #[test]
    fn fallback_hint_abort_serialises_action_tag() {
        let hint = FallbackHint::Abort {
            reason: "bad creds".to_string(),
        };
        let json = serde_json::to_value(&hint).unwrap();
        assert_eq!(json["action"], "abort");
    }
}
