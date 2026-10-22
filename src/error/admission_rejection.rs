//! Narrow upstream admission refusals; never infer replay safety from arbitrary 4xx/5xx.
use super::GatewayError;

pub(crate) fn is_account_balance_rejection(status: u16, message: &str) -> bool {
    let message = message.trim();
    // This is an admission refusal before generation, not an arbitrary 403.
    // Amounts and the optional request-id suffix vary between requests.
    matches!(status, 402 | 403)
        && (message.eq_ignore_ascii_case("Insufficient account balance")
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
        || error
        .http_status
        .is_some_and(|status| is_account_balance_rejection(status, &error.message))
        // A structured overload admission refusal is safe to advance; a generic
        // 503 or a transport failure does not prove that execution never started.
        || (error.http_status == Some(503)
            && error.code.as_deref() == Some("system_cpu_overloaded"))
        || (error.http_status == Some(503)
            && error.code.as_deref() == Some("model_not_found")
            && error.message.starts_with("No available channel for model ")
            && error.message.contains(" (distributor)"))
}
