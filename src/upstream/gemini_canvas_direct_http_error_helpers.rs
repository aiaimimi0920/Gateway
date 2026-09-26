use super::contract_helpers::redact_gemini_canvas_url_for_logs;
use crate::error::GatewayError;
pub(crate) fn gemini_canvas_image_json_attempts_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP image replay exhausted all known JSON contracts.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_json_attempts_exhausted")
}

pub(crate) fn gemini_canvas_page_harvest_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest exhausted all request modes.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_exhausted")
}

pub(crate) fn gemini_canvas_page_harvest_redirect_loop_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest exceeded the redirect follow limit.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_redirect_loop")
}

pub(crate) fn gemini_canvas_page_harvest_redirect_missing_location_error(
    status: u16,
) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest returned a redirect without a usable Location header.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_redirect_missing_location");
    error.http_status = Some(status);
    error
}

pub(crate) fn gemini_canvas_page_harvest_unsafe_redirect_error(status: u16) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP page harvest refused a cross-origin, credential-bearing, or non-HTTP(S) redirect target.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_page_harvest_unsafe_redirect");
    error.http_status = Some(status);
    error
}

pub(crate) fn gemini_canvas_media_bootstrap_exhausted_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas media bootstrap exhausted all page harvest candidates.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_bootstrap_exhausted")
}

pub(crate) fn gemini_canvas_media_fetch_redirect_exhausted_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas direct HTTP media download exhausted redirect/handoff attempts.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_redirect_exhausted")
}

pub(crate) fn gemini_canvas_media_fetch_cookie_mismatch_redirect_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP media download redirected into Google account login/CookieMismatch instead of returning the requested asset.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_cookie_mismatch")
}

pub(crate) fn gemini_canvas_media_fetch_cookie_mismatch_html_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP media download resolved to a Google login/CookieMismatch HTML page instead of a binary asset.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_cookie_mismatch")
}

pub(crate) fn gemini_canvas_media_fetch_bad_redirect_error(next_url: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas direct HTTP media redirect URL was not usable: {}",
        redact_gemini_canvas_url_for_logs(next_url)
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_bad_redirect")
}

pub(crate) fn gemini_canvas_media_fetch_redirect_missing_location_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas direct HTTP media download returned a redirect without a usable Location header.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_redirect_missing_location")
}

pub(crate) fn gemini_canvas_media_fetch_bad_asset_url_error(asset_url: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas direct HTTP media asset URL was not usable: {}",
        redact_gemini_canvas_url_for_logs(asset_url)
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_media_fetch_bad_asset_url")
}

pub(crate) fn gemini_canvas_image_fetch_missing_inline_bytes_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas direct HTTP image materialization completed without inline bytes.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_fetch_missing_inline_bytes")
}

pub(crate) fn gemini_canvas_image_fetch_invalid_inline_bytes_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas direct HTTP image materialization returned invalid base64 bytes: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_image_fetch_invalid_inline_bytes")
}

pub(crate) fn gemini_canvas_pure_http_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas pure HTTP response did not return valid JSON: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_pure_http_invalid_json")
}
