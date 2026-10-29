//! Explicit fault taxonomy: client mistakes/cancellation never poison model availability.
use crate::error::{ErrorKind, GatewayError};

pub(crate) fn failure_outcome(error: &GatewayError) -> super::Outcome {
    // Budget termination can carry a previous upstream error; it is not a new failure.
    if error.request_budget_stop_reason().is_some() || error.kind == ErrorKind::ModelCapacity {
        return super::Outcome::Neutral;
    }
    if crate::error::is_provider_ip_block(error) {
        return super::Outcome::ProviderUnavailable(error.upstream_retry_after_ms.unwrap_or(0));
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
    failure_delay(error).map_or(super::Outcome::Neutral, |delay| {
        if matches!(error.kind, ErrorKind::Network | ErrorKind::Timeout)
            || super::health_control::is_transport_message(&error.message)
        {
            super::Outcome::TransportFailure(delay)
        } else {
            super::Outcome::Failure(delay)
        }
    })
}

pub(crate) fn failure_delay(error: &GatewayError) -> Option<u64> {
    if error.request_budget_stop_reason().is_some()
        || error.kind == ErrorKind::ModelCapacity
        || matches!(
            error.code.as_deref(),
            Some(
                "attempt_cancelled"
                    | "upstream_credential_unavailable"
                    | "rate_limit_admission_indeterminate"
                    | "model_circuit_open"
                    | "gateway_network_unstable"
                    | "site_health_route_restricted"
                    | "capacity_backoff_registry_full"
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
    if crate::error::is_provider_ip_block(error) {
        return Some(error.upstream_retry_after_ms.unwrap_or(0));
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
        ))
    .then_some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_is_neutral_to_hard_circuits_but_real_rate_limits_still_trip() {
        for status in [400, 429, 503] {
            let error = crate::error::classify_upstream_error(
                status,
                r#"{"error":{"code":"model_capacity_exhausted","message":"Model is at capacity"}}"#,
                None,
            );
            assert!(matches!(
                failure_outcome(&error),
                super::super::Outcome::Neutral
            ));
            assert_eq!(failure_delay(&error), None);
        }
        let rate_limit = crate::error::classify_upstream_error(429, "Too many requests", None);
        assert_eq!(failure_delay(&rate_limit), Some(120_000));
    }

    #[test]
    fn provider_ip_block_receives_provider_scoped_circuit_feedback() {
        let error = crate::error::classify_upstream_error(
            403,
            r#"{"error":{"message":"该ip已被封禁，原因：bulk probe guard: ip 27.47.35.168 requested 5 distinct models in 60s (last_user 11552, offense 1)"}}"#,
            Some("fixture"),
        );
        assert!(matches!(
            failure_outcome(&error),
            super::super::Outcome::ProviderUnavailable(0)
        ));
        assert_eq!(failure_delay(&error), Some(0));
    }
}
