//! Udio output readiness, selected media assets and response envelopes.

use super::request_options::requested_output_count;
use super::{UdioOutputKind, UdioSong};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use base64::Engine;
use serde_json::json;
use serde_json::Value;

pub fn songs_ready(songs: &[UdioSong]) -> bool {
    songs_ready_for_output(songs, UdioOutputKind::Music)
}

pub fn songs_ready_for_output(songs: &[UdioSong], output_kind: UdioOutputKind) -> bool {
    !songs.is_empty() && songs.iter().all(|song| song_has_output(song, output_kind))
}

pub fn collect_output_urls(
    songs: &[UdioSong],
    output_kind: UdioOutputKind,
) -> Result<Vec<String>, GatewayError> {
    let urls = songs
        .iter()
        .filter_map(|song| output_url(song, output_kind).map(str::to_string))
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(missing_output_error(output_kind));
    }
    Ok(urls)
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "udio",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": songs,
    })
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    urls: &[String],
) -> Result<Value, GatewayError> {
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": urls
            .iter()
            .take(requested_output_count(req))
            .map(|url| json!({
                "url": url,
                "revised_prompt": prompt,
            }))
            .collect::<Vec<_>>(),
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Value {
    json!({
        "created": current_unix_timestamp(),
        "data": images
            .iter()
            .take(requested_output_count(req))
            .map(|(mime_type, bytes)| {
                json!({
                    "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
                    "revised_prompt": prompt,
                    "mime_type": mime_type,
                })
            })
            .collect::<Vec<_>>(),
    })
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    let data = songs
        .iter()
        .filter_map(|song| serialize_media_asset(song, UdioOutputKind::Video))
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(missing_output_error(UdioOutputKind::Video));
    }

    Ok(json!({
        "object": UdioOutputKind::Video.object_name(),
        "provider": "udio",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": data,
    }))
}

fn song_has_output(song: &UdioSong, output_kind: UdioOutputKind) -> bool {
    output_url(song, output_kind).is_some()
}

fn output_url(song: &UdioSong, output_kind: UdioOutputKind) -> Option<&str> {
    match output_kind {
        UdioOutputKind::Image => song.image_url.as_deref(),
        UdioOutputKind::Music => song.audio_url.as_deref(),
        UdioOutputKind::Video => song.video_url.as_deref(),
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
}

fn missing_output_error(output_kind: UdioOutputKind) -> GatewayError {
    GatewayError::server_error(output_kind.missing_output_message())
        .with_provider("udio_compatible")
        .with_code(output_kind.missing_output_code())
}

fn serialize_media_asset(song: &UdioSong, output_kind: UdioOutputKind) -> Option<Value> {
    let url = output_url(song, output_kind)?;
    Some(json!({
        "kind": output_kind.media_kind(),
        "url": url,
        "track_id": song.id.clone(),
        "title": song.title.clone(),
        "status": song.status.clone(),
        "image_url": song.image_url.clone(),
        "audio_url": song.audio_url.clone(),
        "video_url": song.video_url.clone(),
        "duration_seconds": song.duration_seconds,
    }))
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
