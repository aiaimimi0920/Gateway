//! Explicit fault taxonomy: client mistakes/cancellation never poison model availability.
use crate::error::{ErrorKind, GatewayError};

pub(crate) fn failure_outcome(error: &GatewayError) -> super::Outcome {
    // Budget termination can carry a previous upstream error; it is not a new failure.
    if error.request_budget_stop_reason().is_some() {
        return super::Outcome::Neutral;
    }
    if error.kind == ErrorKind::InsufficientQuota
        && matches!(
            error.fallback_hint,
            crate::error::FallbackHint::FallbackProvider { .. }
        )
        && error.http_status.is_some_and(|status| {
            crate::error::is_account_balance_rejection(status, &error.message)
        })
    {
        return super::Outcome::BalanceUnavailable(error.upstream_retry_after_ms.unwrap_or(0));
    }
    if crate::error::is_channel_unavailable(error) {
        return super::Outcome::ChannelUnavailable(error.upstream_retry_after_ms.unwrap_or(0));
    }
    failure_delay(error).map_or(super::Outcome::Neutral, super::Outcome::Failure)
}

pub(crate) fn failure_delay(error: &GatewayError) -> Option<u64> {
    if error.request_budget_stop_reason().is_some()
        || matches!(
            error.code.as_deref(),
            Some(
                "attempt_cancelled"
                    | "upstream_credential_unavailable"
                    | "rate_limit_admission_indeterminate"
                    | "model_circuit_open"
                    | "site_health_route_restricted"
            )
        )
        || matches!(
            error.kind,
            ErrorKind::BadRequest
                | ErrorKind::ContextLength
                | ErrorKind::ContentFilter
                | ErrorKind::Authentication
        )
    {
        return None;
    }
    if error.http_status == Some(429) || error.kind == ErrorKind::RateLimit {
        return Some(
            error
                .upstream_retry_after_ms
                .unwrap_or(120_000)
                .max(120_000),
        );
    }
    if error.code.as_deref() == Some("upstream_html_diagnostic") && error.http_status == Some(403) {
        return Some(0);
    }
    if matches!(error.http_status, Some(401 | 403)) {
        return None;
    }
    (error.http_status.is_some_and(|status| status >= 500)
        // The transport classifier preserves this trusted prefix for non-connect body/send errors.
        || (error.kind == ErrorKind::Unknown && error.http_status.is_none() && error.message.starts_with("Network error: "))
        || matches!(
            error.kind,
            ErrorKind::Timeout
                | ErrorKind::ServiceUnavailable
                | ErrorKind::Network
                | ErrorKind::ServerError
                | ErrorKind::ModelCapacity
        ))
    .then_some(0)
}
