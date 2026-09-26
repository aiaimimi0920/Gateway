//! Select media assets and preserve accepted/pending response contracts.

use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::upstream::gemini::canvas_program_web_reverse as program;

pub fn collect_image_media_assets(
    result: &program::GeminiCanvasBrowserInvocationResult,
) -> Vec<&program::GeminiCanvasBrowserMediaAsset> {
    collect_media_assets_by_kinds(result, &["image"])
}

pub fn convert_media_asset(
    asset: &program::GeminiCanvasBrowserMediaAsset,
) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: asset.kind.clone(),
        url: asset.url.clone(),
        mime_type: asset.mime_type.clone(),
        download_token: None,
        body_base64: asset.body_base64.clone(),
        alt: asset.alt.clone(),
        width: asset.width,
        height: asset.height,
        duration_seconds: asset.duration_seconds,
    }
}

pub fn collect_converted_image_media_assets(
    result: &program::GeminiCanvasBrowserInvocationResult,
) -> Vec<gemini_canvas::GeminiCanvasMediaAsset> {
    collect_image_media_assets(result)
        .into_iter()
        .map(convert_media_asset)
        .collect()
}

pub fn require_music_media_asset<'a>(
    result: &'a program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<&'a program::GeminiCanvasBrowserMediaAsset, GatewayError> {
    require_first_media_asset_by_kinds(
        result,
        &["video", "audio"],
        provider,
        "Gemini Canvas modular browser relay completed without a downloadable music asset.",
        "gemini_canvas_modular_no_music_asset",
    )
}

pub fn build_music_generation_response_from_invocation(
    model: &str,
    prompt: &str,
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<Value, GatewayError> {
    let accepted_body = result.body_text.as_deref();
    if music_body_indicates_pending_or_busy(accepted_body) {
        let has_audio_asset = result.media.iter().any(|asset| asset.kind == "audio");
        if !has_audio_asset {
            return Ok(gemini_canvas::build_music_generation_accepted_response(
                model,
                prompt,
                result
                    .conversation_id
                    .as_deref()
                    .or(result.last_seen_conversation_id.as_deref()),
                result
                    .response_id
                    .as_deref()
                    .or(result.last_seen_response_id.as_deref()),
                result.app_path.as_deref(),
                None,
                accepted_body,
            ));
        }
    }
    let asset = require_music_media_asset(result, provider).map(convert_media_asset)?;
    Ok(gemini_canvas::build_music_generation_response(
        model,
        prompt,
        &asset,
        accepted_body,
    ))
}

pub fn require_video_media_asset<'a>(
    result: &'a program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<&'a program::GeminiCanvasBrowserMediaAsset, GatewayError> {
    require_first_media_asset_by_kinds(
        result,
        &["video"],
        provider,
        "Gemini Canvas modular browser relay completed without a downloadable video asset.",
        "gemini_canvas_modular_no_video_asset",
    )
}

pub fn build_video_generation_response_from_invocation(
    model: &str,
    prompt: &str,
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<Value, GatewayError> {
    let accepted_body = result.body_text.as_deref();
    if let Some(body) = accepted_body {
        let pending = gemini_canvas::response_indicates_video_generation_pending(body)
            || gemini_canvas::response_indicates_video_generation_quota_reached(body);
        if pending {
            let has_video_asset = result.media.iter().any(|asset| asset.kind == "video");
            if !has_video_asset {
                let job_id_hint = gemini_canvas::extract_video_generation_job_id(body);
                return Ok(gemini_canvas::build_video_generation_accepted_response(
                    model,
                    prompt,
                    result
                        .conversation_id
                        .as_deref()
                        .or(result.last_seen_conversation_id.as_deref()),
                    result
                        .response_id
                        .as_deref()
                        .or(result.last_seen_response_id.as_deref()),
                    result.app_path.as_deref(),
                    job_id_hint.as_deref(),
                    accepted_body,
                ));
            }
        }
    }
    let asset = require_video_media_asset(result, provider).map(convert_media_asset)?;
    Ok(gemini_canvas::build_video_generation_response(
        model,
        prompt,
        &asset,
        accepted_body,
    ))
}

fn collect_media_assets_by_kinds<'a>(
    result: &'a program::GeminiCanvasBrowserInvocationResult,
    accepted_kinds: &[&str],
) -> Vec<&'a program::GeminiCanvasBrowserMediaAsset> {
    result
        .media
        .iter()
        .filter(|asset| accepted_kinds.iter().any(|kind| asset.kind == *kind))
        .collect()
}

fn require_first_media_asset_by_kinds<'a>(
    result: &'a program::GeminiCanvasBrowserInvocationResult,
    accepted_kinds: &[&str],
    provider: &str,
    missing_message: &str,
    missing_code: &'static str,
) -> Result<&'a program::GeminiCanvasBrowserMediaAsset, GatewayError> {
    collect_media_assets_by_kinds(result, accepted_kinds)
        .into_iter()
        .next()
        .ok_or_else(|| {
            GatewayError::server_error(missing_message)
                .with_provider(provider)
                .with_code(missing_code)
        })
}

fn music_body_indicates_pending_or_busy(body_text: Option<&str>) -> bool {
    let body_text = body_text.unwrap_or_default();
    if body_text.is_empty() {
        return false;
    }
    let normalized = body_text.to_ascii_lowercase();
    (body_text.contains("music_generation") && body_text.contains("action_input"))
        || normalized.contains("track details")
        || normalized.contains("generating your music")
        || normalized.contains("i've put together a 30-second electronic cue")
        || normalized.contains("i’ve put together a 30-second electronic cue")
        || normalized.contains("electronic cue for you")
        || body_text.contains("\"11\":[\"Electronic Music Cue Generation")
        || normalized.contains("i've hit a bit of a snag")
        || normalized.contains("i’ve hit a bit of a snag")
        || normalized.contains("getting a lot of requests right now")
        || normalized.contains("please try again later")
        || (body_text.contains("\"26\":\"") && body_text.contains("\"44\":true"))
        || body_text.contains("\"11\":[\"Electronic Cue")
}
