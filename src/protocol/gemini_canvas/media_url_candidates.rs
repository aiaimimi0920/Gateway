use serde_json::Value;
use std::collections::HashSet;

use super::{
    media_mime::{infer_image_mime_type, infer_mime_type},
    types::*,
};

pub(super) fn extract_media_asset_from_value_url_scan(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mut urls = Vec::new();
    let mut seen = HashSet::new();
    collect_candidate_media_urls(value, &mut urls, &mut seen);
    select_best_media_asset_from_urls(&urls, operation)
}

fn collect_candidate_media_urls(value: &Value, urls: &mut Vec<String>, seen: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_candidate_media_urls(item, urls, seen);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_candidate_media_urls(item, urls, seen);
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.starts_with("https://") && seen.insert(trimmed.to_string()) {
                urls.push(trimmed.to_string());
            } else if trimmed.starts_with('[') || trimmed.starts_with('{') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    collect_candidate_media_urls(&parsed, urls, seen);
                }
            }
        }
        _ => {}
    }
}

fn select_best_media_asset_from_urls(
    urls: &[String],
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mut best: Option<(i32, GeminiCanvasMediaAsset)> = None;
    for url in urls {
        let Some((rank, asset)) = rank_media_asset_candidate(url, operation) else {
            continue;
        };
        match &best {
            Some((best_rank, _)) if rank <= *best_rank => {}
            _ => best = Some((rank, asset)),
        }
    }
    best.map(|(_, asset)| asset)
}

fn rank_media_asset_candidate(
    url: &str,
    operation: GeminiCanvasMediaOperation,
) -> Option<(i32, GeminiCanvasMediaAsset)> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("blob:")
        || trimmed.starts_with("data:")
        || trimmed.contains("video_gen_chip")
        || trimmed.contains("generated_video_content")
    {
        return None;
    }

    let mime_type = infer_mime_type(trimmed, "");
    let lower = trimmed.to_ascii_lowercase();
    let host_bonus = if lower.contains("contribution.usercontent.google.com") {
        40
    } else if lower.contains("work.fife.usercontent.google.com/rd-gg-dl/") {
        30
    } else if lower.contains("lh3.googleusercontent.com/rd-ogw/") {
        28
    } else if lower.contains("lh3.googleusercontent.com/rd-gg-dl/") {
        25
    } else if lower.contains("lh3.googleusercontent.com/gg-dl/") {
        22
    } else if lower.contains("lh3.googleusercontent.com/gg/") {
        18
    } else {
        0
    };

    match operation {
        GeminiCanvasMediaOperation::Image => {
            let image_like = mime_type.starts_with("image/")
                || lower.contains("lh3.googleusercontent.com/rd-ogw/")
                || lower.contains("lh3.googleusercontent.com/gg-dl/")
                || lower.contains("lh3.googleusercontent.com/gg/")
                || lower.contains("contribution.usercontent.google.com/download?");
            if !image_like {
                return None;
            }
            Some((
                100 + host_bonus,
                GeminiCanvasMediaAsset {
                    kind: "image".to_string(),
                    mime_type: if mime_type.is_empty() {
                        infer_image_mime_type(trimmed)
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
        GeminiCanvasMediaOperation::Music => {
            let (kind, base_rank) = if mime_type.starts_with("audio/") {
                ("audio".to_string(), 100)
            } else if mime_type.starts_with("video/") {
                ("video".to_string(), 80)
            } else {
                return None;
            };
            Some((
                base_rank + host_bonus,
                GeminiCanvasMediaAsset {
                    kind,
                    mime_type: if mime_type.is_empty() {
                        if lower.contains(".mp3") {
                            "audio/mpeg".to_string()
                        } else {
                            "video/mp4".to_string()
                        }
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
        GeminiCanvasMediaOperation::Video => {
            if !mime_type.starts_with("video/") {
                return None;
            }
            Some((
                100 + host_bonus,
                GeminiCanvasMediaAsset {
                    kind: "video".to_string(),
                    mime_type: if mime_type.is_empty() {
                        "video/mp4".to_string()
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
    }
}
