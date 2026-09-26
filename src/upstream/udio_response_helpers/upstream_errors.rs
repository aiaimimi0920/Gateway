//! Udio authentication, browser challenge and media HTTP status classification.

use crate::error::classify_upstream_error;
use crate::error::GatewayError;

pub(crate) fn classify_udio_upstream_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    let provider = "udio_compatible";
    if status == 401 {
        return GatewayError::server_error(
            "Udio session is not authenticated. Capture a fresh browser session cookie before routing requests through the gateway.",
        )
        .with_provider(provider)
        .with_code("udio_session_unauthorized");
    }

    let vercel_mitigated = headers
        .get("x-vercel-mitigated")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default();
    if status == 429 && vercel_mitigated.eq_ignore_ascii_case("challenge") {
        return GatewayError::server_error(
            "Udio requires a browser security check or captcha challenge before generation can continue.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }

    let body_lower = body_text.to_lowercase();
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if status == 403 && body_lower.contains("user disallowed") {
        return GatewayError::server_error(
            "Udio rejected the current challenge token or browser clearance. Refresh the challenge in the same browser context and retry.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }
    let is_vercel_security_checkpoint = body_lower.contains("vercel security checkpoint")
        || body_lower.contains("x-vercel-challenge-token")
        || body_lower.contains("x-vercel-mitigated")
        || body_lower.contains("data-astro-cid-nbv56vs3");
    if (status == 403 || status == 429)
        && (vercel_mitigated.eq_ignore_ascii_case("challenge")
            || (content_type.contains("text/html") && is_vercel_security_checkpoint))
    {
        return GatewayError::server_error(
            "Udio requires a browser security check or captcha challenge before generation can continue.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }

    classify_upstream_error(status, body_text, Some(provider))
}

pub(crate) fn classify_udio_media_fetch_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    classify_udio_upstream_error(status, headers, body_text)
}

pub(crate) fn ensure_successful_udio_media_fetch_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_udio_media_fetch_error(status, headers, body_text))
}
