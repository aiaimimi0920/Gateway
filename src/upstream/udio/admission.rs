//! Validate the raw worker snapshot before the legacy decoder can filter or coerce it.
use super::browser_worker::no_replay;
use crate::{
    error::GatewayError,
    protocol::udio::{self, UdioOutputKind, UdioSong},
    upstream::browser_worker_types::UdioBrowserWorkerSuccess,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn invalid() -> GatewayError {
    let mut error = GatewayError::server_error(
        "Udio returned an inconsistent or malformed task snapshot; details omitted.",
    )
    .with_code("udio_browser_result_invalid");
    error.http_status = Some(502);
    no_replay(error)
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.trim() == id && id.len() <= 128
}
fn asset(value: &Value, aliases: &[&str]) -> Option<String> {
    // Match the worker's nullish priority: a wrong-type first alias cannot be skipped.
    aliases
        .iter()
        .find_map(|key| value.get(*key).filter(|v| !v.is_null()))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn terminal(value: &Value) -> bool {
    value.get("finished").and_then(Value::as_bool) == Some(true)
        || value
            .get("status")
            .or_else(|| value.get("state"))
            .and_then(Value::as_str)
            .is_some_and(|status| {
                matches!(
                    status.to_ascii_lowercase().as_str(),
                    "finished" | "complete" | "completed"
                )
            })
}

pub(super) fn admit_songs(
    result: &UdioBrowserWorkerSuccess,
    target: UdioOutputKind,
    wait_completion: bool,
) -> Result<Vec<UdioSong>, GatewayError> {
    let ids: HashSet<_> = result.track_ids.iter().map(String::as_str).collect();
    if ids.is_empty()
        || ids.len() > 16
        || ids.len() != result.track_ids.len()
        || ids.iter().any(|id| !valid_id(id))
        || result.songs.len() > 16
    {
        return Err(invalid());
    }
    let mut seen = HashSet::new();
    for value in &result.songs {
        let id = value
            .as_object()
            .and_then(|_| value.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        if !valid_id(id) || !ids.contains(id) || !seen.insert(id) {
            return Err(invalid());
        }
        for key in ["status", "state"] {
            if let Some(raw) = value.get(key) {
                let status = raw.as_str().ok_or_else(invalid)?;
                if status.is_empty()
                    || status.trim() != status
                    || status.len() > 64
                    || matches!(
                        status.to_ascii_lowercase().as_str(),
                        "error" | "failed" | "canceled" | "cancelled"
                    )
                {
                    return Err(invalid());
                }
            }
        }
    }
    let mut decoded: HashMap<_, _> = if result.songs.is_empty() {
        HashMap::new()
    } else {
        udio::extract_songs_from_feed(&serde_json::json!({"songs": result.songs}))
            .map_err(|_| invalid())?
            .into_iter()
            .map(|song| (song.id.clone(), song))
            .collect()
    };
    let mut complete = seen.len() == ids.len();
    for raw in &result.songs {
        let id = raw["id"].as_str().ok_or_else(invalid)?;
        let song = decoded.get_mut(id).ok_or_else(invalid)?;
        song.audio_url = asset(raw, &["song_path", "songPath", "audio_url", "audioUrl"]);
        song.video_url = asset(raw, &["video_url", "videoUrl", "video_path", "videoPath"]);
        song.image_url = asset(
            raw,
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
        );
        // The legacy decoder accepts string booleans; only a raw boolean proves finished.
        song.finished = raw.get("finished").and_then(Value::as_bool) == Some(true);
        song.status = raw
            .get("status")
            .or_else(|| raw.get("state"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if song.finished {
                    "finished"
                } else if song.ready_to_stream {
                    "ready"
                } else {
                    "pending"
                }
                .into()
            });
        let has_target = match target {
            UdioOutputKind::Music => song.audio_url.is_some(),
            UdioOutputKind::Image => song.image_url.is_some(),
            UdioOutputKind::Video => song.video_url.is_some(),
        };
        complete &= has_target && terminal(raw);
    }
    if result.completed != complete || (wait_completion && !complete) {
        return Err(invalid());
    }
    // A no-wait partial snapshot must not lose any of the original task identities.
    Ok(udio::pending_songs(&result.track_ids)
        .into_iter()
        .map(|pending| decoded.remove(&pending.id).unwrap_or(pending))
        .collect())
}

#[cfg(test)]
mod tests;
