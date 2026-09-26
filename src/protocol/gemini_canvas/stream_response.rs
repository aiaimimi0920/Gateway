use serde_json::Value;
use std::collections::HashSet;

use super::{
    extract_stream_generate_locator,
    frames::{parse_locator_response_envelopes, parse_response_frames, strip_xssi_prefix},
    media_candidates::extract_media_assets_from_candidate_data,
    media_debug::summarize_stream_generate_media_debug_summary,
    types::*,
};
use crate::error::GatewayError;

pub fn extract_stream_generate_media_assets(
    body: &str,
    operation: GeminiCanvasMediaOperation,
) -> Result<Vec<GeminiCanvasMediaAsset>, GatewayError> {
    let blocked_error = |details: String| {
        let mut error = GatewayError::service_unavailable(
            "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_image_generation_unavailable");
        error.message = details;
        error
    };
    let preview = |text: &str, limit: usize, from_end: bool| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return "<empty>".to_string();
        }
        let chars: Vec<char> = trimmed.chars().collect();
        if from_end {
            let start = chars.len().saturating_sub(limit);
            chars[start..].iter().collect::<String>()
        } else {
            chars.into_iter().take(limit).collect::<String>()
        }
    };
    let normalized = strip_xssi_prefix(body);
    let (frames, _remainder) = parse_response_frames(normalized);
    if frames.is_empty() {
        if operation == GeminiCanvasMediaOperation::Image
            && response_indicates_image_generation_unavailable(200, None, normalized)
        {
            return Err(blocked_error(format!(
                "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.; body_head={}; body_tail={}",
                preview(normalized, 180, false),
                preview(normalized, 180, true)
            )));
        }
        let mut error = GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not contain any parseable frames.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_invalid_frame_response");
        error.message = format!(
            "{}; body_head={}; body_tail={}",
            error.message,
            preview(normalized, 180, false),
            preview(normalized, 180, true)
        );
        return Err(error);
    }

    let mut assets = Vec::new();
    let mut seen_urls = HashSet::new();
    for frame in &frames {
        collect_stream_generate_media_assets(frame, operation, &mut assets, &mut seen_urls);
    }
    if operation == GeminiCanvasMediaOperation::Music {
        prioritize_music_assets(&mut assets);
    }

    if assets.is_empty() {
        let debug_summary = summarize_stream_generate_media_debug_summary(&frames);
        if operation == GeminiCanvasMediaOperation::Image
            && response_indicates_image_generation_unavailable(200, None, normalized)
        {
            return Err(blocked_error(format!(
                "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.; frame_count={}; {}",
                frames.len(),
                debug_summary
            )));
        }
        let code = match operation {
            GeminiCanvasMediaOperation::Image => {
                "gemini_canvas_stream_generate_missing_image_asset"
            }
            GeminiCanvasMediaOperation::Music => {
                "gemini_canvas_stream_generate_missing_music_asset"
            }
            GeminiCanvasMediaOperation::Video => {
                "gemini_canvas_stream_generate_missing_video_asset"
            }
        };
        let frame_preview = frames
            .iter()
            .take(3)
            .map(|frame| {
                serde_json::to_string(frame)
                    .map(|json| preview(&json, 220, false))
                    .unwrap_or_else(|_| "<unserializable-frame>".to_string())
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let mut error = GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not include a usable media asset.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code(code);
        error.message = format!(
            "{}; frame_count={}; frame_preview={}; {}",
            error.message,
            frames.len(),
            frame_preview,
            debug_summary
        );
        return Err(error);
    }

    Ok(assets)
}

pub(super) fn prioritize_music_assets(assets: &mut [GeminiCanvasMediaAsset]) {
    assets.sort_by_key(|asset| {
        if asset.kind == "audio" || asset.mime_type.starts_with("audio/") {
            0u8
        } else if asset.kind == "video" || asset.mime_type.starts_with("video/") {
            1u8
        } else {
            2u8
        }
    });
}

pub fn response_indicates_image_generation_unavailable(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    if !(200..300).contains(&status) {
        return false;
    }

    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let text_like = normalized_content_type.is_empty()
        || normalized_content_type.contains("application/json")
        || normalized_content_type.contains("text/plain");
    let has_real_image_artifact = lower.contains("image_generation_content")
        && (lower.contains("googleusercontent")
            || lower.contains("gstatic")
            || lower.contains("blob:")
            || lower.contains("data:image/"));
    if has_real_image_artifact {
        return false;
    }

    let blocked_like = lower.contains("can't create it right now")
        || lower.contains("can't seem to create any")
        || lower.contains("can't create any for you")
        || (lower.contains("search for images") && lower.contains("can't create"))
        || lower.contains("are you signed in")
        || body.contains("您登录了吗")
        || body.contains("似乎无法为您创建任何图片")
        || body.contains("所在的地区尚未开通图片创建功能");
    text_like && blocked_like
}

pub fn stream_generate_indicates_image_edit_async_followup_ready(body: &str) -> bool {
    if extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image).is_ok() {
        return false;
    }
    if extract_stream_generate_locator(body).is_ok() {
        return true;
    }
    let normalized = strip_xssi_prefix(body);
    normalized.contains("\"37\":[")
        || normalized.contains("\\\"37\\\":[")
        || normalized.contains("\"18\":\"r_")
        || normalized.contains("\\\"18\\\":\\\"r_")
}

pub fn stream_generate_is_image_edit_short_ack(body: &str) -> bool {
    if extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image).is_ok() {
        return false;
    }
    let normalized = strip_xssi_prefix(body);
    normalized.contains("\"wrb.fr\",null,null,null,null,[13]")
        && normalized.contains("\"di\",")
        && normalized.contains("\"af.httprm\",")
}

pub fn stream_generate_parseable_frame_count(body: &str) -> usize {
    let normalized = strip_xssi_prefix(body);
    let (parsed_frames, _remainder) = parse_response_frames(normalized);
    if !parsed_frames.is_empty() {
        return parsed_frames.len();
    }
    parse_locator_response_envelopes(normalized).len()
}

fn collect_stream_generate_media_assets(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
    assets: &mut Vec<GeminiCanvasMediaAsset>,
    seen_urls: &mut HashSet<String>,
) {
    if let Some(mut extracted) = extract_media_assets_from_candidate_data(value, operation) {
        extracted.retain(|asset| seen_urls.insert(asset.url.clone()));
        assets.extend(extracted);
        return;
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_stream_generate_media_assets(item, operation, assets, seen_urls);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_stream_generate_media_assets(item, operation, assets, seen_urls);
            }
        }
        Value::String(text) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                collect_stream_generate_media_assets(&parsed, operation, assets, seen_urls);
            }
        }
        _ => {}
    }
}
