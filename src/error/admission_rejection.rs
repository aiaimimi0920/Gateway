//! Narrow upstream refusals eligible for candidate switching, never arbitrary 4xx/5xx replay.
use super::GatewayError;

pub(crate) fn is_account_balance_rejection(status: u16, message: &str) -> bool {
    let message = message.trim();
    // This is an admission refusal before generation, not an arbitrary 403.
    // Amounts and the optional request-id suffix vary between requests.
    matches!(status, 402 | 403)
        && (message.eq_ignore_ascii_case("Insufficient account balance")
            || message
                .strip_prefix("预扣费额度失败")
                .is_some_and(|details| {
                    details.contains("用户剩余额度:") && details.contains("需要预扣费额度:")
                })
            || message
                .strip_prefix("Not enough quota to pre-consume for this request:")
                .is_some_and(|details| {
                    details.contains(" remaining, ") && details.contains(" required")
                }))
}

pub(crate) fn is_explicit_admission_rejection(error: &GatewayError) -> bool {
    // Local authentication errors retain Abort and cannot acquire a replay slot.
    (error.http_status == Some(401)
        && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        || (error.http_status == Some(404)
            && error.code.as_deref() == Some("upstream_stream_model_not_found"))
        // A relay may wrap a structured pre-stream model refusal in HTTP 502.
        || (error.http_status == Some(502)
            && error.kind == super::ErrorKind::ModelNotFound
            && error.code.as_deref() == Some("upstream_stream_initialization_model_not_found")
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        // The recognized non-API diagnostic page is an upstream invocation failure.
        // Only its trusted classification may advance a tool conversation before handoff;
        // a JSON error borrowing the code, unknown HTML or policy denial remains closed.
        || (error.http_status == Some(403)
            && error.kind == super::ErrorKind::ServiceUnavailable
            && error.code.as_deref() == Some("upstream_html_diagnostic")
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. }))
        || (error.kind == super::ErrorKind::InsufficientQuota
            && matches!(error.fallback_hint, super::FallbackHint::FallbackProvider { .. })
            && error.http_status.is_some_and(|status| is_account_balance_rejection(status, &error.message)))
        // A structured overload admission refusal is safe to advance; a generic
        // 503 or a transport failure does not prove that execution never started.
        || (error.http_status == Some(503)
            && error.code.as_deref() == Some("system_cpu_overloaded"))
        || super::is_channel_unavailable(error)
}
