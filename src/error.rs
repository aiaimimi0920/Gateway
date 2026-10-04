// ---------------------------------------------------------------------------
// GatewayError — structured error classification with FallbackHint
//
// Rust port of packages/ai-gateway-domain/src/modules/gateway/standard-error.ts
// ---------------------------------------------------------------------------

mod classification;
mod diagnostics;
mod retry_after;

pub use classification::{classify_network_error, classify_upstream_error};
pub use diagnostics::{sanitize_provider_error_message, PROVIDER_ERROR_MESSAGE_MAX_CHARS};
pub(crate) use retry_after::retry_after_from_headers;

use axum::http::{header::RETRY_AFTER, HeaderValue, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Clone, Error)]
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

    /// Only fixed Gateway reasons are eligible for the public budget marker.
    pub fn request_budget_stop_reason(&self) -> Option<&'static str> {
        let FallbackHint::Abort { reason } = &self.fallback_hint else {
            return None;
        };
        match reason.as_str() {
            "budget_exhausted:attempt_limit" => Some("attempt_limit"),
            "budget_exhausted:deadline" => Some("deadline"),
            "budget_exhausted:cancelled" => Some("cancelled"),
            "budget_exhausted:response_handed_off" => Some("response_handed_off"),
            _ => None,
        }
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

    /// A transport hint changes timing only, never retry eligibility or error identity.
    pub(crate) fn with_retry_after_ms(mut self, delay_ms: Option<u64>) -> Self {
        if let Some(delay_ms) = delay_ms.filter(|_| self.retryable) {
            self.fallback_hint = FallbackHint::Retry {
                delay_ms,
                reason: "Upstream requested a delay before retrying.".to_string(),
            };
        }
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
                ErrorKind::RateLimit | ErrorKind::InsufficientQuota => {
                    StatusCode::TOO_MANY_REQUESTS
                }
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
        let budget_stop_reason = self.request_budget_stop_reason();
        let body = serde_json::json!({
            "error": {
                "message": self.message,
                "type": format!("{:?}", self.kind),
                "code": self.code,
            }
        });

        let mut response = (status, axum::Json(body)).into_response();
        if let Some(reason) = budget_stop_reason {
            response.headers_mut().insert(
                "x-gateway-error-code",
                HeaderValue::from_static("budget_exhausted"),
            );
            response
                .headers_mut()
                .insert("x-gateway-stop-reason", HeaderValue::from_static(reason));
        }
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
