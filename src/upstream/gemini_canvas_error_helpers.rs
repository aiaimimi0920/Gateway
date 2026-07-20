use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::gemini_canvas;
use crate::protocol::gemini_web;

pub(crate) fn gemini_canvas_body_indicates_context_busy(body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    normalized.contains("i couldn't do that because i'm getting a lot of requests right now")
        || normalized.contains("please try again later")
}

pub(crate) fn classify_gemini_canvas_pure_http_error(
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
) -> GatewayError {
    let provider = "gemini_canvas_compatible";
    if gemini_web::response_indicates_browser_challenge(status, content_type, body_text) {
        let mut error = GatewayError::service_unavailable(
            "Gemini Canvas pure HTTP replay hit an upstream browser challenge.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_pure_http_browser_challenge_required");
        error.http_status = Some(status);
        return error;
    }
    if gemini_web::response_indicates_session_invalid(status, content_type, body_text) {
        let mut error = GatewayError::unauthorized(
            "Gemini Canvas pure HTTP replay session is invalid or expired.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_pure_http_session_invalid");
        error.http_status = Some(status);
        return error;
    }
    if gemini_canvas::response_indicates_image_generation_unavailable(
        status,
        content_type,
        body_text,
    ) {
        let mut error = GatewayError::service_unavailable(
            "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_image_generation_unavailable");
        error.http_status = Some(status);
        return error;
    }
    classify_upstream_error(status, body_text, Some(provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_error_code(error: &GatewayError, expected: &str) {
        assert_eq!(error.code.as_deref(), Some(expected));
    }

    #[test]
    fn classify_gemini_canvas_pure_http_error_detects_browser_challenge() {
        let error = classify_gemini_canvas_pure_http_error(
            403,
            Some("text/html; charset=utf-8"),
            "<!DOCTYPE html><title>Security Check</title><body>Verify you are human</body>",
        );

        assert_error_code(&error, "gemini_canvas_pure_http_browser_challenge_required");
        assert_eq!(error.http_status, Some(403));
    }

    #[test]
    fn classify_gemini_canvas_pure_http_error_detects_session_invalid() {
        let error = classify_gemini_canvas_pure_http_error(
            401,
            Some("text/html; charset=utf-8"),
            "<!DOCTYPE html><a href=\"https://accounts.google.com\">sign in</a>",
        );

        assert_error_code(&error, "gemini_canvas_pure_http_session_invalid");
        assert_eq!(error.http_status, Some(401));
    }

    #[test]
    fn classify_gemini_canvas_pure_http_error_detects_image_unavailable() {
        let error = classify_gemini_canvas_pure_http_error(
            200,
            Some("application/json; charset=utf-8"),
            "Are you signed in? I can search images, but I can't seem to create any images for you right now.",
        );

        assert_error_code(&error, "gemini_canvas_image_generation_unavailable");
        assert_eq!(error.http_status, Some(200));
    }

    #[test]
    fn classify_gemini_canvas_pure_http_error_falls_back_to_generic_upstream_error() {
        let error = classify_gemini_canvas_pure_http_error(
            502,
            Some("application/json"),
            "{\"message\":\"temporary upstream failure\"}",
        );

        assert_eq!(error.http_status, Some(502));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.message, "temporary upstream failure");
    }

    #[test]
    fn gemini_canvas_body_indicates_context_busy_matches_retryable_copy() {
        assert!(gemini_canvas_body_indicates_context_busy(
            "I couldn't do that because I'm getting a lot of requests right now."
        ));
        assert!(gemini_canvas_body_indicates_context_busy(
            "PLEASE TRY AGAIN LATER"
        ));
        assert!(!gemini_canvas_body_indicates_context_busy(
            "generation finished successfully"
        ));
    }
}
