//! LumaLabs image response planning, fetch failures and downloaded output materialization.

use crate::error::classify_upstream_error;
use crate::error::GatewayError;

pub(crate) fn resolve_lumalabs_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    signed_url: &str,
) -> Result<Option<serde_json::Value>, GatewayError> {
    if crate::protocol::lumalabs::prefers_url_response(req)? {
        return Ok(Some(
            crate::protocol::lumalabs::build_openai_images_response_from_url(
                req, prompt, signed_url,
            ),
        ));
    }
    Ok(None)
}

pub(crate) fn classify_lumalabs_media_fetch_error(status: u16, body_text: &str) -> GatewayError {
    classify_upstream_error(status, body_text, Some("lumalabs_compatible"))
}

pub(crate) fn ensure_successful_lumalabs_media_fetch_status(
    status: u16,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_lumalabs_media_fetch_error(status, body_text))
}

pub(crate) fn resolve_lumalabs_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
    signed_url: &str,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| crate::protocol::lumalabs::infer_mime_type_from_url(signed_url))
}

pub(crate) fn build_lumalabs_downloaded_image_response(
    prompt: &str,
    signed_url: &str,
    headers: &rquest::header::HeaderMap,
    bytes: &[u8],
) -> serde_json::Value {
    let mime_type = resolve_lumalabs_downloaded_image_mime_type(headers, signed_url);
    crate::protocol::lumalabs::build_openai_images_response_from_bytes(prompt, &mime_type, bytes)
}
