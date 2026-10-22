//! Narrow upstream admission refusals; never infer replay safety from arbitrary 4xx/5xx.
use super::GatewayError;

pub(crate) fn is_account_balance_rejection(status: u16, message: &str) -> bool {
    matches!(status, 402 | 403)
        && message
            .trim()
            .eq_ignore_ascii_case("Insufficient account balance")
}

pub(crate) fn is_explicit_admission_rejection(error: &GatewayError) -> bool {
    error
        .http_status
        .is_some_and(|status| is_account_balance_rejection(status, &error.message))
        || (error.http_status == Some(503)
            && error.code.as_deref() == Some("model_not_found")
            && error.message.starts_with("No available channel for model ")
            && error.message.contains(" (distributor)"))
}
