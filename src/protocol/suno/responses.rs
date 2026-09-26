//! Suno selected media assets and caller-facing response envelopes.

use super::request_options::requested_output_count;
use super::SunoClip;
use super::DEFAULT_IMAGE_MIME_TYPE;
use super::DEFAULT_VIDEO_MIME_TYPE;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use base64::Engine;
use serde_json::json;
use serde_json::Value;

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    clips: &[SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "suno",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": clips,
    })
}

pub fn image_urls_for_requested_count(
    req: &CanonicalRelayRequest,
    clips: &[SunoClip],
) -> Result<Vec<String>, GatewayError> {
    let urls = clips
        .iter()
        .filter_map(|clip| clip.image_url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(requested_output_count(req))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(
            GatewayError::server_error("Suno clips did not contain any image output.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_image_asset"),
        );
    }
    Ok(urls)
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    clips: &[SunoClip],
) -> Result<Value, GatewayError> {
    let urls = image_urls_for_requested_count(req, clips)?;
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": urls.into_iter().map(|url| json!({
            "url": url,
            "revised_prompt": prompt,
            "mime_type": infer_mime_type_from_url(&url, DEFAULT_IMAGE_MIME_TYPE),
        })).collect::<Vec<_>>(),
        "n": requested_output_count(req),
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Result<Value, GatewayError> {
    let data = images
        .iter()
        .take(requested_output_count(req))
        .map(|(mime_type, bytes)| {
            json!({
                "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
                "revised_prompt": prompt,
                "mime_type": mime_type,
            })
        })
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(
            GatewayError::server_error("Suno image download backfill returned no bytes.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_image_bytes"),
        );
    }
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": data,
        "n": requested_output_count(req),
    }))
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    clips: &[SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    let clip = first_clip_with_media_url(clips, |candidate| candidate.video_url.as_deref())
        .ok_or_else(|| {
            GatewayError::server_error("Suno clips did not contain any video output.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_video_asset")
        })?;
    let video_url = clip.video_url.as_deref().unwrap_or_default();
    Ok(json!({
        "object": "video.generation",
        "provider": "suno",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": [serialize_media_asset(
            clip,
            "video",
            video_url,
            &infer_mime_type_from_url(video_url, DEFAULT_VIDEO_MIME_TYPE),
        )],
    }))
}

pub fn infer_mime_type_from_url(url: &str, fallback: &str) -> String {
    let normalized = url
        .split('?')
        .next()
        .unwrap_or(url)
        .trim()
        .to_ascii_lowercase();
    if normalized.ends_with(".jpg") || normalized.ends_with(".jpeg") {
        return "image/jpeg".to_string();
    }
    if normalized.ends_with(".webp") {
        return "image/webp".to_string();
    }
    if normalized.ends_with(".gif") {
        return "image/gif".to_string();
    }
    if normalized.ends_with(".png") {
        return "image/png".to_string();
    }
    if normalized.ends_with(".mp4") {
        return "video/mp4".to_string();
    }
    if normalized.ends_with(".webm") {
        return "video/webm".to_string();
    }
    fallback.to_string()
}

fn first_clip_with_media_url<'a, F>(clips: &'a [SunoClip], picker: F) -> Option<&'a SunoClip>
where
    F: Fn(&'a SunoClip) -> Option<&'a str>,
{
    clips.iter().find(|clip| {
        picker(clip)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    })
}

fn serialize_media_asset(clip: &SunoClip, kind: &str, url: &str, mime_type: &str) -> Value {
    json!({
        "id": clip.id,
        "kind": kind,
        "url": url,
        "mime_type": mime_type,
        "title": clip.title,
        "status": clip.status,
        "image_url": clip.image_url,
        "audio_url": clip.audio_url,
        "video_url": clip.video_url,
        "created_at": clip.created_at,
        "model_name": clip.model_name,
        "prompt": clip.prompt,
        "lyric": clip.lyric,
        "tags": clip.tags,
        "negative_tags": clip.negative_tags,
        "duration": clip.duration,
        "error_message": clip.error_message,
    })
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
