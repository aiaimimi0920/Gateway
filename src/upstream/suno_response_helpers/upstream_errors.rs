//! Suno upstream authentication, challenge and HTTP status classification.

use crate::error::classify_upstream_error;
use crate::error::GatewayError;

pub(crate) fn suno_challenge_required_error() -> GatewayError {
    GatewayError::server_error(
        "Suno requires an active browser challenge token before generation can continue.",
    )
    .with_provider("suno_compatible")
    .with_code("suno_challenge_required")
}

pub(crate) fn classify_suno_upstream_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> GatewayError {
    let provider = "suno_compatible";
    if status == 401 {
        return GatewayError::server_error(
            "Suno session is not authenticated. Capture a fresh browser session cookie before routing requests through the gateway.",
        )
        .with_provider(provider)
        .with_code("suno_session_unauthorized");
    }

    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let body_lower = body_text.to_ascii_lowercase();
    let looks_like_html = content_type.contains("text/html")
        || body_lower.contains("<!doctype html")
        || body_lower.contains("<html");
    let looks_like_challenge = body_lower.contains("captcha")
        || body_lower.contains("turnstile")
        || body_lower.contains("challenge")
        || body_lower.contains("cloudflare");
    let body_is_empty = body_text.trim().is_empty();

    if missing_challenge_token
        && matches!(status, 400 | 403 | 422 | 429)
        && (body_is_empty || looks_like_html || looks_like_challenge)
    {
        return suno_challenge_required_error();
    }

    classify_upstream_error(status, body_text, Some(provider))
}

pub(crate) fn classify_suno_media_fetch_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    classify_suno_upstream_error(status, headers, body_text, false)
}

pub(crate) fn ensure_successful_suno_media_fetch_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_suno_media_fetch_error(status, headers, body_text))
}

pub(crate) fn ensure_successful_suno_upstream_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_suno_upstream_error(
        status,
        headers,
        body_text,
        missing_challenge_token,
    ))
}
