//! Accepted music results, no-key audio decoding, and follow-up error contracts.

use base64::Engine;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;

mod websocket;

pub(crate) use websocket::{
    build_gemini_canvas_music_ws_client_content_frame,
    build_gemini_canvas_music_ws_generation_config_frame,
    build_gemini_canvas_music_ws_playback_control_frame, build_gemini_canvas_music_ws_setup_frame,
    consume_gemini_canvas_music_server_content, gemini_canvas_music_socket_url_with_api_key,
    gemini_canvas_music_ws_authenticated_setup_error, gemini_canvas_music_ws_connect_failed_error,
    gemini_canvas_music_ws_missing_audio_error, gemini_canvas_music_ws_pong_failed_error,
    gemini_canvas_music_ws_request_failed_error, gemini_canvas_music_ws_send_frame_error,
    gemini_canvas_music_ws_setup_timeout_error, gemini_canvas_music_ws_setup_transport_error,
    gemini_canvas_music_ws_stream_transport_error, parse_gemini_canvas_music_ws_binary,
    parse_gemini_canvas_music_ws_setup_binary, parse_gemini_canvas_music_ws_setup_text,
    parse_gemini_canvas_music_ws_text,
};

pub(crate) fn gemini_canvas_music_body_indicates_accepted_progress(body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    (body.contains("music_generation") && body.contains("action_input"))
        || normalized.contains("track details")
        || normalized.contains("generating your music")
        || normalized.contains("i've put together a 30-second electronic cue")
        || normalized.contains("i’ve put together a 30-second electronic cue")
        || normalized.contains("electronic cue for you")
        || body.contains("\"11\":[\"Electronic Music Cue Generation")
        || body.contains("\"11\":[\"Electronic Cue")
        || body.contains("\\\"11\\\":[\\\"Electronic Music Cue Generation")
        || body.contains("\\\"11\\\":[\\\"Electronic Cue")
        || (body.contains("\"26\":\"") && body.contains("\"44\":true"))
        || (body.contains("\\\"26\\\":\\\"") && body.contains("\\\"44\\\":true"))
}

pub(crate) fn select_preferred_gemini_canvas_music_asset<'a>(
    assets: &'a [gemini_canvas::GeminiCanvasMediaAsset],
) -> Option<&'a gemini_canvas::GeminiCanvasMediaAsset> {
    assets
        .iter()
        .find(|asset| asset.kind == "audio" || asset.mime_type.starts_with("audio/"))
        .or_else(|| {
            assets
                .iter()
                .find(|asset| asset.kind == "video" || asset.mime_type.starts_with("video/"))
        })
        .or_else(|| assets.first())
}

pub(crate) fn build_gemini_canvas_music_accepted_response_from_body(
    model: &str,
    prompt: &str,
    duration_seconds: Option<f64>,
    body_text: &str,
    fallback_conversation_id: Option<&str>,
    fallback_response_id: Option<&str>,
    fallback_app_path: Option<&str>,
) -> Value {
    let locator_hint = gemini_canvas::extract_stream_generate_locator(body_text).ok();
    let conversation_id_hint = locator_hint
        .as_ref()
        .map(|locator| locator.conversation_id.as_str())
        .or(fallback_conversation_id);
    let response_id_hint = locator_hint
        .as_ref()
        .map(|locator| locator.response_id.as_str())
        .or(fallback_response_id);
    let app_path_hint = locator_hint
        .as_ref()
        .map(|locator| locator.app_path.as_str())
        .or(fallback_app_path);
    gemini_canvas::build_music_generation_accepted_response(
        model,
        prompt,
        conversation_id_hint,
        response_id_hint,
        app_path_hint,
        duration_seconds,
        Some(body_text),
    )
}

pub(crate) fn gemini_canvas_music_response_requires_browser_followup(response: &Value) -> bool {
    if response
        .get("accepted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !response
            .get("completed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return true;
    }

    response
        .get("data")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("status"))
        .and_then(Value::as_str)
        .map(|status| status.eq_ignore_ascii_case("pending"))
        .unwrap_or(false)
}

pub(crate) fn gemini_canvas_music_filtered_prompt_error(filtered_prompt: &Value) -> GatewayError {
    GatewayError::bad_request(format!(
        "Gemini Canvas music prompt was filtered: {filtered_prompt}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_filtered_prompt")
}

pub(crate) fn gemini_canvas_music_missing_asset_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Gemini Canvas music generation completed without a downloadable media asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_no_music_asset")
}

pub(crate) fn gemini_canvas_music_missing_api_key_error() -> GatewayError {
    GatewayError::unauthorized("Gemini Canvas official music API requires api_key.")
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_missing_api_key")
}

pub(crate) fn gemini_canvas_program_music_no_key_contract_missing_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas no-key music StreamGenerate did not expose a usable action contract or audio asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_no_key_contract_missing")
}

pub(crate) fn gemini_canvas_program_music_no_key_browserless_stage_incomplete_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but the current browserless replay still lacks the required page-state to advance beyond BardErrorInfo [1060].",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_no_key_stage_incomplete")
}

pub(crate) fn gemini_canvas_program_music_no_key_prelude_stage_incomplete_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but even the current-page minimal prelude still could not advance beyond BardErrorInfo [1060].",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_no_key_stage_incomplete")
}

pub(crate) fn gemini_canvas_program_music_no_key_post_1060_contract_missing_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas no-key music StreamGenerate advanced past the old 1060 gate, but still did not expose a usable action contract or audio asset.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_no_key_contract_missing")
}

pub(crate) fn gemini_canvas_program_music_page_poll_failed_error(
    provider: &str,
    bootstrap_page_url: &str,
    upstream_summary: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas music follow-up still did not expose a usable music asset after PCck7e/page poll. bootstrap_page={bootstrap_page_url}; upstream={upstream_summary}",
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_page_poll_failed")
}

pub(crate) fn gemini_canvas_program_music_recent_page_poll_failed_error(
    provider: &str,
    bootstrap_page_url: &str,
    app_page_url: &str,
    upstream_summary: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas music aPya6c follow-up advanced to accepted state, but recent-conversation page polling still did not expose a usable music asset. bootstrap_page={bootstrap_page_url}; recovered_page={app_page_url}; upstream={upstream_summary}",
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_recent_page_poll_failed")
}

pub(crate) fn gemini_canvas_program_music_browser_fallback_forbidden_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned music did not expose a direct no-key contract. Browser execution fallback is disabled on the default path.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_browser_fallback_forbidden")
}

pub(crate) fn gemini_canvas_program_music_no_key_request_contract_missing_error(
    provider: &str,
) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned music StreamGenerate contract is missing requestUrl/requestBody.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_music_no_key_contract_missing")
}

pub(crate) fn gemini_canvas_program_music_invoke_target_missing_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program-owned music lane is missing an explicit app invoke target.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_program_music_invoke_target_missing")
}

pub(crate) fn decode_gemini_canvas_program_music_no_key_audio(
    body_base64: Option<&str>,
    content_type: Option<&str>,
) -> Result<gemini_canvas::GeminiCanvasAudio, GatewayError> {
    let body_base64 = body_base64.ok_or_else(|| {
        GatewayError::service_unavailable(
            "Gemini Canvas preview-frame no-key music websocket completed without audio bytes.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_program_music_no_key_missing_audio")
    })?;
    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(body_base64)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Gemini Canvas preview-frame no-key music websocket returned invalid base64 audio bytes: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_program_music_no_key_invalid_audio")
        })?;

    Ok(gemini_canvas::GeminiCanvasAudio {
        mime_type: content_type
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("audio/L16;codec=pcm;rate=48000;channels=2")
            .to_string(),
        bytes: audio_bytes,
    })
}

#[cfg(test)]
mod tests;
