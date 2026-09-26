//! Udio track identity extraction, pending records and feed decoding.

use super::field_values::{read_value_bool, read_value_number, read_value_string};
use super::UdioSong;
use crate::error::GatewayError;
use serde_json::Value;

pub fn extract_track_ids(body: &Value) -> Result<Vec<String>, GatewayError> {
    let ids = body
        .get("track_ids")
        .or_else(|| body.get("trackIds"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|ids| !ids.is_empty())
        .or_else(|| {
            body.get("songs")
                .and_then(|value| value.as_array())
                .map(|songs| {
                    songs
                        .iter()
                        .filter_map(|song| song.get("id").and_then(|value| value.as_str()))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
        })
        .filter(|ids| !ids.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error("Udio generation response missing track ids.")
                .with_provider("udio_compatible")
                .with_code("udio_missing_track_ids")
        })?;

    Ok(ids)
}

pub fn extract_songs_from_feed(body: &Value) -> Result<Vec<UdioSong>, GatewayError> {
    let songs = body
        .get("songs")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Udio songs response missing songs array.")
                .with_provider("udio_compatible")
                .with_code("udio_missing_songs")
        })?;

    let parsed = songs.iter().filter_map(parse_song).collect::<Vec<_>>();
    if parsed.is_empty() {
        return Err(GatewayError::server_error(
            "Udio songs response did not contain any valid songs.",
        )
        .with_provider("udio_compatible")
        .with_code("udio_invalid_songs"));
    }

    Ok(parsed)
}

pub fn pending_songs(track_ids: &[String]) -> Vec<UdioSong> {
    track_ids
        .iter()
        .map(|id| UdioSong {
            id: id.clone(),
            title: None,
            image_url: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: false,
            ready_to_stream: false,
            estimated_duration_seconds: None,
            status: "pending".to_string(),
            error_message: None,
        })
        .collect()
}

fn parse_song(value: &Value) -> Option<UdioSong> {
    let id = read_value_string(value, &["id"])?;
    let finished = read_value_bool(value, &["finished"]).unwrap_or(false);
    let ready_to_stream =
        read_value_bool(value, &["readyToStream", "ready_to_stream"]).unwrap_or(false);
    let status = read_value_string(value, &["status", "state"]).unwrap_or_else(|| {
        if finished {
            "finished".to_string()
        } else if ready_to_stream {
            "ready".to_string()
        } else {
            "pending".to_string()
        }
    });
    let prompt = read_value_string(value, &["prompt", "description"]);
    let lyric_input = read_value_string(value, &["lyricInput", "lyric_input", "lyrics"]);
    let audio_url = read_value_string(value, &["song_path", "songPath", "audio_url", "audioUrl"]);

    Some(UdioSong {
        id,
        title: read_value_string(value, &["title"]),
        image_url: read_value_string(
            value,
            &[
                "image_url",
                "imageUrl",
                "image_path",
                "imagePath",
                "cover_image_url",
                "coverImageUrl",
                "cover_art_url",
                "coverArtUrl",
            ],
        ),
        audio_url,
        video_url: read_value_string(value, &["video_url", "videoUrl", "video_path", "videoPath"]),
        created_at: read_value_string(value, &["created_at", "createdAt"]),
        duration_seconds: read_value_number(
            value,
            &["duration_seconds", "durationSeconds", "duration"],
        ),
        prompt,
        lyrics: read_value_string(value, &["lyrics"]),
        lyric_input,
        finished,
        ready_to_stream,
        estimated_duration_seconds: read_value_number(
            value,
            &["estimatedDuration", "estimated_duration"],
        ),
        status,
        error_message: read_value_string(
            value,
            &["error_message", "errorMessage", "error_detail", "error"],
        ),
    })
}
