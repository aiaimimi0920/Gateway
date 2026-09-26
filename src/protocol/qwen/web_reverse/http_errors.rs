use super::{QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, QWEN_WEB_SESSION_INVALID_CODE};
use crate::error::{classify_upstream_error, GatewayError};

pub fn classify_qwen_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    let provider = "qwen_web_compatible";
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "Qwen Web direct replay hit an upstream browser challenge and requires a fresh browser-backed session refresh.",
        )
        .with_provider(provider)
        .with_code(QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }

    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "Qwen Web browser session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(provider)
        .with_code(QWEN_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }

    classify_upstream_error(status, body, Some(provider))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("security checkpoint")
        || lower.contains("verify you are human")
        || lower.contains("verify that you are human")
        || lower.contains("bot verification")
        || lower.contains("bot check")
        || lower.contains("just a moment")
        || lower.contains("waf")
        || lower.contains("x5sec")
        || lower.contains("access denied");
    let rgv587_challenge = (lower.contains("rgv587") || lower.contains("fail_sys_user_validate"))
        && lower.contains("\"ret\"")
        && lower.contains("\"url\"");

    (status == 403 || status == 429 || status == 503 || status == 200)
        && ((html_like && challenge_like) || rgv587_challenge)
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("unauthorized")
        || lower.contains("token expired")
        || lower.contains("login")
        || lower.contains("log in")
        || lower.contains("sign in")
        || lower.contains("session expired")
        || lower.contains("invalid token")
        || lower.contains("\"code\":401")
        || lower.contains("\"status\":401");

    matches!(status, 401 | 403) && (auth_like || (html_like && auth_like))
}
