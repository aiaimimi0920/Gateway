//! Suno selected image URLs and downloaded image materialization.

use crate::error::GatewayError;
use serde_json::Value;

pub(crate) fn resolve_suno_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    clips: &[crate::protocol::suno::SunoClip],
) -> Result<(Vec<String>, Option<Value>), GatewayError> {
    let image_urls = crate::protocol::suno::image_urls_for_requested_count(req, clips)?;
    let response = if crate::protocol::suno::prefers_url_response(req)? {
        Some(crate::protocol::suno::build_openai_images_response_from_urls(req, prompt, clips)?)
    } else {
        None
    };
    Ok((image_urls, response))
}

pub(crate) fn resolve_suno_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
    url: &str,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| crate::protocol::suno::infer_mime_type_from_url(url, "image/png"))
}

pub(crate) fn materialize_suno_downloaded_image(
    headers: &rquest::header::HeaderMap,
    url: &str,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    (
        resolve_suno_downloaded_image_mime_type(headers, url),
        bytes.to_vec(),
    )
}

pub(crate) fn build_suno_downloaded_images_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Result<Value, GatewayError> {
    crate::protocol::suno::build_openai_images_response_from_bytes(req, prompt, images)
}
