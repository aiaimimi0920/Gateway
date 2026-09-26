//! Udio selected image URLs and downloaded image materialization.

use crate::error::GatewayError;
use crate::protocol::udio::UdioOutputKind;
use crate::protocol::udio::UdioSong;

pub(crate) fn resolve_udio_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    songs: &[UdioSong],
) -> Result<(Vec<String>, Option<serde_json::Value>), GatewayError> {
    let image_urls = crate::protocol::udio::collect_output_urls(songs, UdioOutputKind::Image)?
        .into_iter()
        .take(crate::protocol::udio::requested_output_count(req))
        .collect::<Vec<_>>();
    let response = if crate::protocol::udio::prefers_url_response(req)? {
        Some(
            crate::protocol::udio::build_openai_images_response_from_urls(
                req,
                prompt,
                &image_urls,
            )?,
        )
    } else {
        None
    };
    Ok((image_urls, response))
}

pub(crate) fn resolve_udio_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| "image/jpeg".to_string())
}

pub(crate) fn materialize_udio_downloaded_image(
    headers: &rquest::header::HeaderMap,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    (
        resolve_udio_downloaded_image_mime_type(headers),
        bytes.to_vec(),
    )
}

pub(crate) fn build_udio_downloaded_images_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> serde_json::Value {
    crate::protocol::udio::build_openai_images_response_from_bytes(req, prompt, images)
}
