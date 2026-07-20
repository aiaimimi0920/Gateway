use base64::Engine;
use serde_json::{json, Value};

use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::gemini_canvas;
use crate::upstream::browser_worker_runtime_helpers::{
    gemini_canvas_http_replay_worker_empty_output_error,
    gemini_canvas_http_replay_worker_output_parse_error,
};
use crate::upstream::browser_worker_types::{
    GeminiCanvasHttpReplayWorkerResult, GeminiCanvasHttpReplayWorkerSuccess,
};
use crate::upstream::gemini::canvas_program_web_reverse as program;

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedPoolResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub result: Option<Value>,
    pub error: Option<GeminiCanvasBrowserOwnedPoolError>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedPoolError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub status: Option<u16>,
    pub body: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedInvocationResult {
    pub operation: String,
    #[serde(rename = "shareUrl")]
    pub share_url: Option<String>,
    #[serde(rename = "shareId")]
    pub share_id: Option<String>,
    #[serde(rename = "shareFollowKind")]
    pub share_follow_kind: Option<String>,
    #[serde(rename = "beforeUrl")]
    pub before_url: Option<String>,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "stableProgramPair")]
    pub stable_program_pair: Option<Value>,
    #[serde(rename = "latestResponsePair")]
    pub latest_response_pair: Option<Value>,
    #[serde(rename = "aggregateHints")]
    pub aggregate_hints: Option<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "newChatClicked")]
    pub new_chat_clicked: Option<bool>,
    #[serde(rename = "modeSelected")]
    pub mode_selected: Option<bool>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub media: Vec<GeminiCanvasBrowserOwnedMediaAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedMediaAsset {
    pub kind: String,
    pub url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(rename = "durationSeconds")]
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct GeminiCanvasBrowserOwnedFetchInvocationResult {
    pub operation: String,
    pub status: u16,
    #[serde(rename = "finalUrl")]
    pub final_url: Option<String>,
    #[serde(rename = "pageUrl")]
    pub page_url: Option<String>,
    #[serde(rename = "canvasProgramUrl")]
    pub canvas_program_url: Option<String>,
    #[serde(rename = "appPath")]
    pub app_path: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "responseId")]
    pub response_id: Option<String>,
    #[serde(rename = "lastSeenConversationId")]
    pub last_seen_conversation_id: Option<String>,
    #[serde(rename = "lastSeenResponseId")]
    pub last_seen_response_id: Option<String>,
    #[serde(rename = "candidatePairs", default)]
    pub candidate_pairs: Vec<Value>,
    #[serde(rename = "capturedAt")]
    pub captured_at: Option<String>,
    #[serde(rename = "lastValidatedAt")]
    pub last_validated_at: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(rename = "bodyText")]
    pub body_text: Option<String>,
    #[serde(rename = "bodyBase64")]
    pub body_base64: Option<String>,
}

impl From<GeminiCanvasBrowserOwnedMediaAsset> for program::GeminiCanvasBrowserMediaAsset {
    fn from(value: GeminiCanvasBrowserOwnedMediaAsset) -> Self {
        Self {
            kind: value.kind,
            url: value.url,
            mime_type: value.mime_type,
            body_base64: value.body_base64,
            alt: value.alt,
            width: value.width,
            height: value.height,
            duration_seconds: value.duration_seconds,
        }
    }
}

impl From<GeminiCanvasBrowserOwnedInvocationResult>
    for program::GeminiCanvasBrowserInvocationResult
{
    fn from(value: GeminiCanvasBrowserOwnedInvocationResult) -> Self {
        Self {
            operation: value.operation,
            bootstrap_operation: None,
            share_url: value.share_url,
            share_id: value.share_id,
            share_follow_kind: value.share_follow_kind,
            before_url: value.before_url,
            final_url: value.final_url,
            page_url: value.page_url,
            canvas_program_url: value.canvas_program_url,
            app_path: value.app_path,
            conversation_id: value.conversation_id,
            response_id: value.response_id,
            invoke_base_url: None,
            music_ws_url: None,
            video_invoke_path: None,
            canvas_program_action: None,
            canvas_program_action_input: None,
            canvas_program_invoke_contract: None,
            last_seen_conversation_id: value.last_seen_conversation_id,
            last_seen_response_id: value.last_seen_response_id,
            candidate_pairs: value.candidate_pairs,
            stable_program_pair: value.stable_program_pair,
            latest_response_pair: value.latest_response_pair,
            aggregate_hints: value.aggregate_hints,
            captured_at: value.captured_at,
            last_validated_at: value.last_validated_at,
            new_chat_clicked: value.new_chat_clicked,
            mode_selected: value.mode_selected,
            body_text: value.body_text,
            body_base64: value.body_base64,
            mime_type: value.mime_type,
            text: value.text,
            media: value.media.into_iter().map(Into::into).collect(),
        }
    }
}

fn parse_browser_pool_result(
    provider: &str,
    body_text: &str,
    parse_error_code: &'static str,
    context_label: &str,
) -> Result<GeminiCanvasBrowserOwnedPoolResult, GatewayError> {
    serde_json::from_str(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to parse {context_label}: {error}. body: {body_text}"
        ))
        .with_provider(provider)
        .with_code(parse_error_code)
    })
}

pub(crate) fn parse_http_replay_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<GeminiCanvasHttpReplayWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(gemini_canvas_http_replay_worker_empty_output_error(stderr));
    }

    serde_json::from_str::<GeminiCanvasHttpReplayWorkerResult>(stdout).map_err(|error| {
        gemini_canvas_http_replay_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn extract_http_replay_worker_success(
    result: GeminiCanvasHttpReplayWorkerResult,
) -> Result<GeminiCanvasHttpReplayWorkerSuccess, GatewayError> {
    Ok(GeminiCanvasHttpReplayWorkerSuccess {
        status: result.status.unwrap_or(200),
        content_type: result.content_type,
        body_text: result.body_text.unwrap_or_default(),
    })
}

pub(crate) fn classify_http_replay_worker_failure(
    result: GeminiCanvasHttpReplayWorkerResult,
    stderr: &str,
    provider: &str,
) -> GatewayError {
    let status = result
        .error
        .as_ref()
        .and_then(|entry| entry.status)
        .or(result.status)
        .unwrap_or(500);
    let body_text = result
        .error
        .as_ref()
        .and_then(|entry| entry.body_text.as_deref())
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = result.error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

pub(crate) fn build_gemini_canvas_browser_executor_service_result(
    result: &program::GeminiCanvasBrowserInvocationResult,
) -> Value {
    json!({
        "operation": result.operation,
        "bodyText": result.body_text,
        "media": result
            .media
            .iter()
            .map(|asset| {
                json!({
                    "kind": asset.kind,
                    "url": asset.url,
                    "mimeType": asset.mime_type,
                    "alt": asset.alt,
                    "width": asset.width,
                    "height": asset.height,
                    "durationSeconds": asset.duration_seconds,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn classify_browser_pool_failure(
    provider: &str,
    http_status: u16,
    result: GeminiCanvasBrowserOwnedPoolResult,
) -> GatewayError {
    let mut gateway_error = classify_upstream_error(
        result
            .status
            .or_else(|| result.error.as_ref().and_then(|entry| entry.status))
            .unwrap_or(http_status),
        result
            .error
            .as_ref()
            .and_then(|entry| entry.body.as_deref())
            .unwrap_or(""),
        Some(provider),
    );
    if let Some(error) = result.error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            gateway_error.message = message;
        }
    }
    gateway_error
}

pub fn parse_browser_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
    expected_operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_browser_pool_output_parse_failed",
        "Gemini Canvas browser pool response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas browser pool reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserOwnedInvocationResult>(
        result_body,
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to decode Gemini Canvas browser pool media payload: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_result_decode_failed")
    })?;
    if invocation.operation != expected_operation {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas browser pool returned '{}' while '{}' was requested.",
            invocation.operation, expected_operation
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_operation_mismatch"));
    }
    Ok(invocation.into())
}

pub fn parse_remote_browser_invocation_value(
    provider: &str,
    result: Value,
    context_label: &str,
    error_code: &'static str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    serde_json::from_value::<GeminiCanvasBrowserOwnedInvocationResult>(result)
        .map(Into::into)
        .map_err(|error| {
            GatewayError::server_error(format!("Failed to parse {context_label}: {error}"))
                .with_provider(provider)
                .with_code(error_code)
        })
}

pub fn parse_remote_media_browser_invocation_value(
    provider: &str,
    result: Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    parse_remote_browser_invocation_value(
        provider,
        result,
        &format!("remote Gemini Canvas {operation} result"),
        "gemini_canvas_remote_result_parse_failed",
    )
}

pub fn parse_remote_modular_media_browser_invocation_value(
    provider: &str,
    result: Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    parse_remote_browser_invocation_value(
        provider,
        result,
        &format!("remote Gemini Canvas modular {operation} result"),
        "gemini_canvas_modular_remote_result_parse_failed",
    )
}

pub fn parse_connected_fetch_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserOwnedFetchInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_connected_fetch_output_parse_failed",
        "Gemini Canvas connected fetch response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas connected fetch reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_result")
    })?;
    let invocation =
        serde_json::from_value::<GeminiCanvasBrowserOwnedFetchInvocationResult>(result_body)
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "Failed to decode Gemini Canvas connected fetch payload: {error}"
                ))
                .with_provider(provider)
                .with_code("gemini_canvas_connected_fetch_decode_failed")
            })?;
    if !(200..300).contains(&invocation.status) {
        return Err(classify_upstream_error(
            invocation.status,
            invocation.body_text.as_deref().unwrap_or(""),
            Some(provider),
        ));
    }
    Ok(invocation)
}

pub fn parse_connected_fetch_json_body(
    provider: &str,
    body_text: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_text = body_text.ok_or_else(|| {
        GatewayError::server_error("Gemini Canvas connected fetch completed without a JSON body.")
            .with_provider(provider)
            .with_code("gemini_canvas_connected_fetch_missing_body")
    })?;
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas connected fetch did not return valid JSON: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_invalid_json")
    })
}

pub fn browser_request_retry_delay_ms(error: &GatewayError, attempts: usize) -> Option<u64> {
    let code = error.code.as_deref();
    let explicit_quota_gate = matches!(code, Some("gemini_canvas_video_quota_reached"));
    let retryable = matches!(
        code,
        Some("gemini_canvas_context_busy")
            | Some("gemini_canvas_auth_required")
            | Some("gemini_canvas_browser_worker_failed")
    ) || (error.http_status == Some(429) && !explicit_quota_gate)
        || (error.retryable && code.is_none());
    if !retryable || attempts >= 2 {
        return None;
    }
    Some(match code {
        Some("gemini_canvas_auth_required") => 2_000,
        Some("gemini_canvas_context_busy") => 3_000 + (attempts as u64 * 1_500),
        _ => 2_500,
    })
}

pub fn require_text_result(
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
    missing_message: &str,
    missing_code: &'static str,
) -> Result<String, GatewayError> {
    result.text.clone().ok_or_else(|| {
        GatewayError::server_error(missing_message)
            .with_provider(provider)
            .with_code(missing_code)
    })
}

pub fn extract_text_or_body_text(result: &program::GeminiCanvasBrowserInvocationResult) -> String {
    result
        .text
        .clone()
        .or(result.body_text.clone())
        .unwrap_or_default()
}

pub fn decode_inline_audio_payload(
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
    missing_message: &str,
    missing_code: &'static str,
    invalid_message_prefix: &str,
    invalid_code: &'static str,
    default_mime_type: &str,
) -> Result<gemini_canvas::GeminiCanvasAudio, GatewayError> {
    let encoded_audio = result.body_base64.as_deref().ok_or_else(|| {
        GatewayError::server_error(missing_message)
            .with_provider(provider)
            .with_code(missing_code)
    })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded_audio)
        .map_err(|error| {
            GatewayError::server_error(format!("{invalid_message_prefix}: {error}"))
                .with_provider(provider)
                .with_code(invalid_code)
        })?;
    let mime_type = result
        .mime_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_mime_type)
        .to_string();
    Ok(gemini_canvas::GeminiCanvasAudio { mime_type, bytes })
}

pub fn decode_modular_tts_audio_payload(
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<gemini_canvas::GeminiCanvasAudio, GatewayError> {
    decode_inline_audio_payload(
        result,
        provider,
        "Gemini Canvas modular browser relay completed without an audio payload.",
        "gemini_canvas_modular_missing_tts_audio_payload",
        "Gemini Canvas modular browser relay returned invalid inline audio bytes",
        "gemini_canvas_modular_invalid_tts_audio_payload",
        "audio/ogg",
    )
}

pub fn decode_browser_tts_audio_payload(
    result: &program::GeminiCanvasBrowserInvocationResult,
    provider: &str,
) -> Result<gemini_canvas::GeminiCanvasAudio, GatewayError> {
    decode_inline_audio_payload(
        result,
        provider,
        "Gemini Canvas browser-backed TTS completed without an audio payload.",
        "gemini_canvas_missing_tts_audio_payload",
        "Gemini Canvas browser-backed TTS returned invalid base64 audio bytes",
        "gemini_canvas_invalid_tts_audio_payload",
        "audio/wav",
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorKind, FallbackHint};

    #[test]
    fn browser_request_retry_delay_retries_retryable_network_error_without_code() {
        let error = GatewayError {
            kind: ErrorKind::Network,
            message: "Connection error: client error (SendRequest)".to_string(),
            code: None,
            http_status: None,
            retryable: true,
            fallback_hint: FallbackHint::Retry {
                delay_ms: 2_000,
                reason: "temporary network error".to_string(),
            },
            provider_name: Some("gemini_canvas_compatible".to_string()),
        };

        assert_eq!(browser_request_retry_delay_ms(&error, 0), Some(2_500));
        assert_eq!(browser_request_retry_delay_ms(&error, 1), Some(2_500));
        assert_eq!(browser_request_retry_delay_ms(&error, 2), None);
    }
}
