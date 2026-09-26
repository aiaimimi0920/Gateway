//! Suno feed polling, clip decoding and endpoint readiness.

use super::field_values::read_value_string;
use super::SunoClip;
use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;
use serde_json::json;
use serde_json::Value;

pub fn build_feed_poll_request(clip_ids: &[String]) -> Value {
    json!({
        "filters": {
            "ids": {
                "presence": "True",
                "clipIds": clip_ids,
            }
        },
        "limit": clip_ids.len(),
    })
}

pub fn extract_clip_ids(body: &Value) -> Result<Vec<String>, GatewayError> {
    let clips = body
        .get("clips")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Suno generation response missing clips array.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_generation_clips")
        })?;

    let ids = clips
        .iter()
        .filter_map(|clip| clip.get("id").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    if ids.is_empty() {
        return Err(GatewayError::server_error(
            "Suno generation response clips array did not contain ids.",
        )
        .with_provider("suno_compatible")
        .with_code("suno_missing_clip_ids"));
    }

    Ok(ids)
}

pub fn extract_clips_from_feed(body: &Value) -> Result<Vec<SunoClip>, GatewayError> {
    let clips = body
        .get("clips")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Suno feed response missing clips array.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_feed_clips")
        })?;

    let parsed = clips.iter().filter_map(parse_clip).collect::<Vec<_>>();

    if parsed.is_empty() {
        return Err(GatewayError::server_error(
            "Suno feed response did not contain any valid clips.",
        )
        .with_provider("suno_compatible")
        .with_code("suno_invalid_feed_clips"));
    }

    Ok(parsed)
}

pub fn clips_ready_for_endpoint(endpoint_kind: EndpointKind, clips: &[SunoClip]) -> bool {
    !clips.is_empty()
        && clips.iter().all(|clip| {
            matches!(clip.status.as_str(), "complete" | "completed")
                && match endpoint_kind {
                    EndpointKind::ImagesGenerations => has_media_url(clip.image_url.as_deref()),
                    EndpointKind::MusicGenerations => has_media_url(clip.audio_url.as_deref()),
                    EndpointKind::VideosGenerations => has_media_url(clip.video_url.as_deref()),
                    _ => false,
                }
        })
}

fn has_media_url(value: Option<&str>) -> bool {
    value.is_some_and(|candidate| !candidate.trim().is_empty())
}

fn parse_clip(value: &Value) -> Option<SunoClip> {
    let metadata = value.get("metadata");
    let id = value.get("id")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let status = value
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("pending")
        .trim()
        .to_string();
    Some(SunoClip {
        id,
        title: read_value_string(value, &["title"]),
        image_url: read_value_string(value, &["image_url", "imageUrl"]),
        lyric: metadata.and_then(|value| read_value_string(value, &["prompt"])),
        audio_url: read_value_string(value, &["audio_url", "audioUrl"]),
        video_url: read_value_string(value, &["video_url", "videoUrl"]),
        created_at: read_value_string(value, &["created_at", "createdAt"]),
        model_name: read_value_string(value, &["model_name", "modelName"]),
        prompt: metadata.and_then(|value| read_value_string(value, &["prompt"])),
        gpt_description_prompt: metadata.and_then(|value| {
            read_value_string(value, &["gpt_description_prompt", "gptDescriptionPrompt"])
        }),
        status,
        clip_type: metadata.and_then(|value| read_value_string(value, &["type"])),
        tags: metadata.and_then(|value| read_value_string(value, &["tags"])),
        negative_tags: metadata
            .and_then(|value| read_value_string(value, &["negative_tags", "negativeTags"])),
        duration: metadata.and_then(|value| read_value_string(value, &["duration"])),
        error_message: metadata
            .and_then(|value| read_value_string(value, &["error_message", "errorMessage"])),
    })
}
