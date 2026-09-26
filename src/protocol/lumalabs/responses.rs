//! LumaLabs media MIME inference and caller-facing response envelopes.

use super::request_options::requested_image_count;
use crate::protocol::canonical::CanonicalRelayRequest;
use base64::Engine;
use serde_json::json;
use serde_json::Value;

pub fn infer_mime_type_from_url(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains(".mp4") {
        "video/mp4".to_string()
    } else if lower.contains(".webm") {
        "video/webm".to_string()
    } else if lower.contains(".mp3") {
        "audio/mpeg".to_string()
    } else if lower.contains(".wav") {
        "audio/wav".to_string()
    } else if lower.contains(".m4a") {
        "audio/mp4".to_string()
    } else if lower.contains(".ogg") {
        "audio/ogg".to_string()
    } else if lower.contains(".jpg") || lower.contains(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.contains(".webp") || lower.contains("format=webp") {
        "image/webp".to_string()
    } else if lower.contains(".gif") {
        "image/gif".to_string()
    } else {
        "image/png".to_string()
    }
}

pub fn build_openai_images_response_from_url(
    req: &CanonicalRelayRequest,
    prompt: &str,
    signed_url: &str,
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "created": created,
        "data": [{
            "url": signed_url,
            "revised_prompt": prompt,
        }],
        "n": requested_image_count(req),
    })
}

pub fn build_openai_images_response_from_bytes(
    prompt: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "created": created,
        "data": [{
            "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
            "revised_prompt": prompt,
            "mime_type": mime_type,
        }],
    })
}

pub fn build_video_generation_response(model: &str, prompt: &str, signed_url: &str) -> Value {
    build_media_generation_response("video.generation", "video", model, prompt, signed_url)
}

pub fn build_audio_generation_response(model: &str, prompt: &str, signed_url: &str) -> Value {
    build_media_generation_response("audio.generation", "audio", model, prompt, signed_url)
}

fn build_media_generation_response(
    object: &str,
    kind: &str,
    model: &str,
    prompt: &str,
    signed_url: &str,
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "object": object,
        "provider": "lumalabs",
        "created": created,
        "completed": true,
        "model": model,
        "prompt": prompt,
        "data": [{
            "url": signed_url,
            "mime_type": infer_mime_type_from_url(signed_url),
            "kind": kind,
        }],
    })
}
