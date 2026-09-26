use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::{sleep, timeout};

use super::{ensure_successful_producer_http_status, PRODUCER_MUSIC_CLIP_POLL_INTERVAL};
use crate::error::{classify_network_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider as read_body;

#[cfg(test)]
#[path = "producer_music_clips_tests.rs"]
mod hardening_tests;

async fn fetch_producer_music_library(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let response = http
        .request(
            Method::GET,
            crate::protocol::producer::build_clips_library_url(base_url),
        )
        .headers(headers.clone())
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let body_text = read_body(response, "Producer.ai music library response", provider).await?;
    ensure_successful_producer_http_status(
        status,
        &body_text,
        provider,
        Some("producer_music_clip_poll_failed"),
    )?;
    serde_json::from_str(&body_text).map_err(|_| {
        GatewayError::server_error("Producer music clip library response was not valid JSON.")
            .with_provider(provider)
            .with_code("producer_music_clip_poll_invalid_response")
    })
}

pub(super) async fn poll_producer_music_clip_assets(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    stream_summary: Value,
    poll_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let clip_ids = producer_music_clip_ids(&stream_summary);
    if clip_ids.is_empty() {
        return Err(GatewayError::server_error(
            "Producer music stream completed without returning a clip ID.",
        )
        .with_provider(provider)
        .with_code("producer_music_missing_clip_id"));
    }

    // One budget owns requests, body reads and interval sleeps; dropping it cancels in-flight I/O.
    timeout(poll_timeout, async {
        loop {
            let library = fetch_producer_music_library(http, base_url, headers).await?;
            let clips = producer_music_clip_assets(&library, &clip_ids);
            if producer_music_clip_assets_complete(&clips, &clip_ids) {
                return build_producer_music_completed_response(stream_summary, clips);
            }
            sleep(PRODUCER_MUSIC_CLIP_POLL_INTERVAL).await;
        }
    })
    .await
    .map_err(|_| {
        GatewayError::service_unavailable(
            "Producer music clip media did not become available before the poll timeout.",
        )
        .with_provider(provider)
        .with_code("producer_music_clip_poll_timeout")
    })?
}

fn producer_music_clip_ids(value: &Value) -> Vec<String> {
    fn visit<'a>(value: &'a Value, seen: &mut HashSet<&'a str>, output: &mut Vec<String>) {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, seen, output);
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "clip_id" | "clipId" | "song_id" | "songId") {
                        if let Some(value) = value
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            if seen.insert(value) {
                                output.push(value.to_string());
                            }
                        }
                    }
                    visit(value, seen, output);
                }
            }
            _ => {}
        }
    }

    let mut output = Vec::new();
    // Keep traversal order in the output; borrowed membership avoids quadratic duplicate scans.
    visit(value, &mut HashSet::new(), &mut output);
    output
}

fn producer_music_clip_assets(library: &Value, clip_ids: &[String]) -> Vec<Value> {
    fn visit(
        value: &Value,
        targets: &HashSet<&str>,
        seen: &mut HashSet<String>,
        output: &mut Vec<Value>,
    ) {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, targets, seen, output);
                }
            }
            Value::Object(object) => {
                let candidate_id = ["id", "clip_id", "clipId", "song_id", "songId"]
                    .iter()
                    .find_map(|key| object.get(*key).and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if let Some(candidate_id) = candidate_id {
                    if targets.contains(candidate_id) && seen.insert(candidate_id.to_string()) {
                        let mut clip = Map::new();
                        clip.insert(
                            "clip_id".to_string(),
                            Value::String(candidate_id.to_string()),
                        );
                        for field in [
                            "title",
                            "duration",
                            "audio_url",
                            "wav_url",
                            "image_url",
                            "video_url",
                            "status",
                            "state",
                        ] {
                            if let Some(value) = object.get(field) {
                                clip.insert(field.to_string(), value.clone());
                            }
                        }
                        output.push(Value::Object(clip));
                    }
                }
                for value in object.values() {
                    visit(value, targets, seen, output);
                }
            }
            _ => {}
        }
    }

    let targets = clip_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    visit(library, &targets, &mut seen, &mut output);
    output
}

fn producer_music_clip_assets_complete(clips: &[Value], clip_ids: &[String]) -> bool {
    let available: HashSet<&str> = clips
        .iter()
        .filter(|clip| downloadable_music_url(clip).is_some())
        .filter_map(|clip| clip.get("clip_id").and_then(Value::as_str))
        .collect();
    clip_ids.iter().all(|id| available.contains(id.as_str()))
}

fn downloadable_music_url(clip: &Value) -> Option<&str> {
    ["audio_url", "wav_url"].iter().find_map(|field| {
        clip.get(*field)
            .and_then(Value::as_str)
            .filter(|url| !url.trim().is_empty())
    })
}

fn build_producer_music_completed_response(
    mut stream_summary: Value,
    clips: Vec<Value>,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let object = stream_summary.as_object_mut().ok_or_else(|| {
        GatewayError::server_error("Producer music stream summary must be a JSON object.")
            .with_provider(provider)
            .with_code("producer_music_invalid_stream_summary")
    })?;
    let data = clips
        .iter()
        .filter_map(|clip| {
            let clip_id = clip.get("clip_id")?.as_str()?;
            let url = downloadable_music_url(clip)?;
            let mut item = Map::new();
            item.insert("clip_id".to_string(), Value::String(clip_id.to_string()));
            item.insert("url".to_string(), Value::String(url.to_string()));
            for field in ["wav_url", "image_url", "title", "duration"] {
                if let Some(value) = clip.get(field) {
                    item.insert(field.to_string(), value.clone());
                }
            }
            Some(Value::Object(item))
        })
        .collect::<Vec<_>>();
    object.insert("clips".to_string(), Value::Array(clips));
    object.insert("data".to_string(), Value::Array(data));
    object.insert("completed".to_string(), Value::Bool(true));
    object.insert("media_completed".to_string(), Value::Bool(true));
    Ok(stream_summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn producer_music_clip_completion_builds_downloadable_media_contract() {
        let summary = json!({
            "object": "music.generation",
            "completed": true,
            "parts": [
                {"part": {"content": {"clip_id": "clip-a"}}},
                {"part": {"content": {"clip_id": "clip-b"}}}
            ]
        });
        let clip_ids = producer_music_clip_ids(&summary);
        assert_eq!(clip_ids, vec!["clip-a", "clip-b"]);

        let library = json!({
            "data": [
                {
                    "id": "clip-b",
                    "title": "Paris B",
                    "audio_url": "https://storage.example/clips/b.m4a",
                    "wav_url": "https://storage.example/clips/b.wav"
                },
                {
                    "id": "clip-a",
                    "title": "Paris A",
                    "audio_url": "https://storage.example/clips/a.m4a"
                }
            ]
        });
        let clips = producer_music_clip_assets(&library, &clip_ids);
        assert!(producer_music_clip_assets_complete(&clips, &clip_ids));

        let completed = build_producer_music_completed_response(summary, clips)
            .expect("music completion response");
        assert_eq!(completed["completed"], true);
        assert_eq!(completed["media_completed"], true);
        assert_eq!(completed["data"].as_array().map(Vec::len), Some(2));
        assert!(completed["data"].as_array().is_some_and(|items| {
            items.iter().all(|item| {
                item.get("clip_id").and_then(Value::as_str).is_some()
                    && item.get("url").and_then(Value::as_str).is_some()
            })
        }));
    }
}
