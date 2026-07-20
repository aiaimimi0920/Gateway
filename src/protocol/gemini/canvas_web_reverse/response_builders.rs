use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas as legacy;

use super::{prefers_url_response, requested_output_count};

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[legacy::GeminiCanvasMediaAsset],
) -> Result<Value, GatewayError> {
    let use_url = prefers_url_response(req)?;
    let created = current_unix_timestamp();

    let data = images
        .iter()
        .take(requested_output_count(req))
        .map(|image| {
            if use_url {
                json!({
                    "url": image.url,
                    "revised_prompt": prompt,
                    "mime_type": image.mime_type,
                })
            } else {
                json!({
                    "url": image.url,
                    "revised_prompt": prompt,
                    "mime_type": image.mime_type,
                })
            }
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "created": created,
        "data": data,
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[legacy::GeminiCanvasImage],
) -> Result<Value, GatewayError> {
    let use_url = prefers_url_response(req)?;
    let created = current_unix_timestamp();

    let data = images
        .iter()
        .take(requested_output_count(req))
        .map(|image| {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&image.bytes);
            if use_url {
                json!({
                    "url": format!("data:{};base64,{}", image.mime_type, encoded),
                    "revised_prompt": prompt,
                })
            } else {
                json!({
                    "b64_json": encoded,
                    "revised_prompt": prompt,
                    "mime_type": image.mime_type,
                })
            }
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "created": created,
        "data": data,
    }))
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    asset: &legacy::GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "gemini_canvas",
        "created": current_unix_timestamp(),
        "completed": true,
        "model": model,
        "prompt": prompt,
        "message": summarize_body_text(body_text),
        "data": [serialize_media_asset(asset)],
    })
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    asset: &legacy::GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "video.generation",
        "provider": "gemini_canvas",
        "created": current_unix_timestamp(),
        "completed": true,
        "model": model,
        "prompt": prompt,
        "message": summarize_body_text(body_text),
        "data": [serialize_media_asset(asset)],
    })
}

fn serialize_media_asset(asset: &legacy::GeminiCanvasMediaAsset) -> Value {
    json!({
        "url": asset.url,
        "mime_type": asset.mime_type,
        "kind": asset.kind,
        "alt": asset.alt,
        "width": asset.width,
        "height": asset.height,
        "duration_seconds": asset.duration_seconds,
        "body_base64": asset.body_base64,
    })
}

fn summarize_body_text(body_text: Option<&str>) -> Option<String> {
    body_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(600).collect::<String>())
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
