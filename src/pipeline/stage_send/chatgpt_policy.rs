//! Candidate chatgpt policy ownership.
use super::*;

pub(super) fn should_refresh_chatgpt_web_after_failure(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE
                | crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_SESSION_INVALID_CODE
        )
    )
}

pub(super) fn should_escalate_chatgpt_web_to_browser_relay(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    error: &GatewayError,
) -> bool {
    if should_refresh_chatgpt_web_after_failure(error) {
        return true;
    }
    let has_browser_recovery_seed = payload
        .extra_body
        .as_ref()
        .map(|body| {
            [
                "authSeed",
                "chatgptAuthUrl",
                "mailboxRef",
                "mailboxSessionId",
            ]
            .iter()
            .any(|key| body.get(*key).is_some())
        })
        .unwrap_or(false);
    let has_runtime_bearer = !payload.api_key.trim().is_empty()
        || payload
            .auth_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
    let has_runtime_cookie = payload
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.trim())
        .is_some_and(|value| !value.is_empty());
    if !(has_browser_recovery_seed || has_runtime_bearer || has_runtime_cookie) {
        return false;
    }
    let message = error.message.as_str();
    matches!(
        error.kind,
        crate::error::ErrorKind::ServerError | crate::error::ErrorKind::ServiceUnavailable
    ) && (message.contains("Internal Server Error") || message.contains("\"detail\""))
}

pub(super) fn chatgpt_web_request_time_browser_allowed(
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> bool {
    if let Ok(value) = std::env::var("CHATGPT_WEB_REVERSE_REQUEST_TIME_BROWSER") {
        let normalized = value.trim().to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "0" | "false" | "no" | "off" | "disabled" | "never" | "pure_http_only"
        ) {
            return false;
        }
    }

    let Some(extra_body) = payload.extra_body.as_ref() else {
        return true;
    };
    if extra_body
        .get("requestTimeBrowserAllowed")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|allowed| !allowed)
    {
        return false;
    }
    let mode = extra_body
        .get("requestTimeBrowserMode")
        .or_else(|| extra_body.get("browserFallbackMode"))
        .or_else(|| extra_body.get("fallbackMode"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase);
    if let Some(mode) = mode {
        if matches!(
            mode.as_str(),
            "disabled" | "never" | "off" | "pure_http_only" | "browserless" | "fail_fast"
        ) {
            return false;
        }
    }
    true
}

pub(super) fn chatgpt_web_request_time_browser_path_needed(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    error: &GatewayError,
) -> bool {
    should_refresh_chatgpt_web_after_failure(error)
        || should_escalate_chatgpt_web_to_browser_relay(payload, error)
}

pub(super) fn chatgpt_web_browser_fallback_forbidden_error(error: &GatewayError) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "ChatGPT Web reverse pure HTTP replay failed and request-time browser fallback is disabled. original_code={} original_message={}",
        error.code.as_deref().unwrap_or("none"),
        error.message
    ))
    .with_provider("chatgpt_web_reverse_compatible")
    .with_code("chatgpt_web_request_time_browser_forbidden")
}
