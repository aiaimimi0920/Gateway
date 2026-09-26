//! Caller-visible image, music, and video result envelopes.

use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas as legacy;

use super::{prefers_url_response, requested_output_count};

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
        "provider": "google_gemini_api",
        "created": current_unix_timestamp(),
        "completed": true,
        "model": model,
        "prompt": prompt,
        "message": summarize_body_text(body_text),
        "data": [serialize_media_asset(asset)],
    })
}

pub fn build_music_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    duration_seconds: Option<f64>,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "google_gemini_api",
        "created": current_unix_timestamp(),
        "accepted": true,
        "completed": false,
        "model": model,
        "prompt": prompt,
        "conversation_id": conversation_id,
        "response_id": response_id,
        "app_path": app_path,
        "duration_seconds": duration_seconds,
        "message": summarize_body_text(body_text),
        "data": [{
            "kind": "audio",
            "status": "pending",
            "url": Value::Null,
            "mime_type": Value::Null,
            "alt": prompt,
            "width": Value::Null,
            "height": Value::Null,
            "duration_seconds": duration_seconds,
            "body_base64": Value::Null,
        }],
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
        "provider": "google_gemini_api",
        "created": current_unix_timestamp(),
        "completed": true,
        "model": model,
        "prompt": prompt,
        "message": summarize_body_text(body_text),
        "data": [serialize_media_asset(asset)],
    })
}

pub fn build_video_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    job_id: Option<&str>,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "video.generation",
        "provider": "google_gemini_api",
        "created": current_unix_timestamp(),
        "accepted": true,
        "completed": false,
        "model": model,
        "prompt": prompt,
        "conversation_id": conversation_id,
        "response_id": response_id,
        "app_path": app_path,
        "job_id": job_id,
        "message": summarize_body_text(body_text),
        "data": [{
            "kind": "video",
            "status": "pending",
            "url": Value::Null,
            "mime_type": Value::Null,
            "alt": prompt,
            "width": Value::Null,
            "height": Value::Null,
            "duration_seconds": Value::Null,
            "body_base64": Value::Null,
        }],
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
