use super::super::{
    CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, CHATGPT_WEB_REVERSE_ADAPTER,
    CHATGPT_WEB_SESSION_INVALID_CODE,
};
use crate::error::{classify_upstream_error, GatewayError};

pub fn classify_chatgpt_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "ChatGPT Web reverse hit an upstream browser challenge and requires a refreshed browser session or different network path.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code(CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }
    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "ChatGPT Web reverse session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code(CHATGPT_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }
    classify_upstream_error(status, body, Some(CHATGPT_WEB_REVERSE_ADAPTER))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let lower = body.to_ascii_lowercase();
    let html_like = content_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|media_type| {
            media_type
                .trim_matches([' ', '\t'])
                .eq_ignore_ascii_case("text/html")
        })
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("cloudflare")
        || lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("verify you are human")
        || lower.contains("just a moment")
        || lower.contains("attention required");
    matches!(status, 403 | 429 | 503 | 200) && html_like && challenge_like
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("unauthorized")
        || lower.contains("token expired")
        || lower.contains("log in")
        || lower.contains("sign in")
        || lower.contains("session expired")
        || lower.contains("invalid token")
        || lower.contains("not logged in");
    matches!(status, 401 | 403) && (auth_like || (html_like && auth_like))
}
