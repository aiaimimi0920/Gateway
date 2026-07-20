use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;

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

pub(crate) fn consume_gemini_canvas_music_server_content(
    value: &Value,
    mime_type: &mut String,
    audio_bytes: &mut Vec<u8>,
) -> Result<bool, GatewayError> {
    let Some(chunks) = value
        .get("serverContent")
        .or_else(|| value.get("server_content"))
        .and_then(|content| {
            content
                .get("audioChunks")
                .or_else(|| content.get("audio_chunks"))
        })
        .and_then(Value::as_array)
    else {
        return Ok(false);
    };

    let mut saw_audio = false;
    for chunk in chunks {
        let raw = chunk
            .get("data")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "Gemini Canvas music websocket returned an audio chunk without base64 data.",
                )
                .with_provider("gemini_canvas_compatible")
                .with_code("gemini_canvas_music_missing_audio")
            })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .map_err(|error| {
                GatewayError::service_unavailable(format!(
                    "Gemini Canvas music websocket returned invalid base64 audio data: {error}"
                ))
                .with_provider("gemini_canvas_compatible")
                .with_code("gemini_canvas_music_invalid_audio")
            })?;
        if let Some(value) = chunk
            .get("mimeType")
            .or_else(|| chunk.get("mime_type"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            *mime_type = value.to_string();
        }
        audio_bytes.extend_from_slice(&bytes);
        saw_audio = true;
    }

    Ok(saw_audio)
}

pub(crate) fn gemini_canvas_music_socket_url_with_api_key(
    socket_url: &str,
    api_key: &str,
) -> Result<url::Url, GatewayError> {
    let mut url = url::Url::parse(socket_url).map_err(|error| {
        GatewayError::server_error(format!(
            "failed to build Gemini Canvas music websocket url: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_request_failed")
    })?;
    url.query_pairs_mut().append_pair("key", api_key.trim());
    Ok(url)
}

pub(crate) fn build_gemini_canvas_music_ws_setup_frame(upstream_model: &str) -> String {
    json!({
        "setup": {
            "model": format!("models/{upstream_model}")
        }
    })
    .to_string()
}

pub(crate) fn build_gemini_canvas_music_ws_client_content_frame(prompt: &str) -> String {
    json!({
        "client_content": gemini_canvas::build_music_client_content(prompt)
    })
    .to_string()
}

pub(crate) fn build_gemini_canvas_music_ws_generation_config_frame(config: &Value) -> String {
    json!({
        "music_generation_config": config
    })
    .to_string()
}

pub(crate) fn build_gemini_canvas_music_ws_playback_control_frame() -> String {
    json!({
        "playback_control": "PLAY"
    })
    .to_string()
}

pub(crate) fn parse_gemini_canvas_music_ws_setup_text(text: &str) -> Result<Value, GatewayError> {
    serde_json::from_str::<Value>(text).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "Gemini Canvas music websocket returned invalid JSON during setup: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_invalid_json")
    })
}

pub(crate) fn parse_gemini_canvas_music_ws_setup_binary(
    bytes: &[u8],
) -> Result<Value, GatewayError> {
    serde_json::from_slice::<Value>(bytes).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "Gemini Canvas music websocket returned invalid binary JSON during setup: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_invalid_json")
    })
}

pub(crate) fn parse_gemini_canvas_music_ws_text(text: &str) -> Result<Value, GatewayError> {
    serde_json::from_str::<Value>(text).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "Gemini Canvas music websocket returned invalid JSON: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_invalid_json")
    })
}

pub(crate) fn parse_gemini_canvas_music_ws_binary(bytes: &[u8]) -> Result<Value, GatewayError> {
    serde_json::from_slice::<Value>(bytes).map_err(|error| {
        GatewayError::service_unavailable(format!(
            "Gemini Canvas music websocket returned invalid binary JSON: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_invalid_json")
    })
}

pub(crate) fn gemini_canvas_music_ws_pong_failed_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "failed to reply to Gemini Canvas music websocket ping: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_send_failed")
}

pub(crate) fn gemini_canvas_music_ws_request_failed_error(
    action: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!("failed to {action}: {error}"))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_music_ws_request_failed")
}

pub(crate) fn gemini_canvas_music_ws_connect_failed_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "failed to connect to Gemini Canvas music websocket: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_connect_failed")
}

pub(crate) fn gemini_canvas_music_ws_send_frame_error(
    frame_name: &str,
    error: &str,
) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "failed to send Gemini Canvas {frame_name} frame: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_send_failed")
}

pub(crate) fn gemini_canvas_music_ws_setup_transport_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas music websocket setup failed: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_transport_failed")
}

pub(crate) fn gemini_canvas_music_ws_stream_transport_error(error: &str) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "Gemini Canvas music websocket failed while streaming audio: {error}"
    ))
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_transport_failed")
}

pub(crate) fn gemini_canvas_music_ws_setup_timeout_error(
    last_setup_event: Option<String>,
) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Gemini Canvas music websocket did not acknowledge setup.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_setup_timeout");
    if let Some(event) = last_setup_event {
        error.message = format!("{}; last_setup_event={event}", error.message);
    }
    error
}

pub(crate) fn gemini_canvas_music_ws_authenticated_setup_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas music websocket could not establish an authenticated setup.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_ws_setup_timeout")
}

pub(crate) fn gemini_canvas_music_ws_missing_audio_error() -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas music websocket completed without audio chunks.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_music_missing_audio")
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
mod tests {
    use super::*;
    use serde_json::json;

    fn make_music_asset(
        kind: &str,
        url: &str,
        mime_type: &str,
    ) -> gemini_canvas::GeminiCanvasMediaAsset {
        gemini_canvas::GeminiCanvasMediaAsset {
            kind: kind.to_string(),
            url: url.to_string(),
            mime_type: mime_type.to_string(),
            download_token: None,
            body_base64: None,
            alt: None,
            width: None,
            height: None,
            duration_seconds: Some(30.0),
        }
    }

    #[test]
    fn parse_gemini_canvas_music_ws_setup_text_reports_invalid_json_contract() {
        let error = parse_gemini_canvas_music_ws_setup_text("{not-json")
            .expect_err("invalid setup JSON should fail");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_invalid_json")
        );
        assert!(error
            .message
            .starts_with("Gemini Canvas music websocket returned invalid JSON during setup:"));
    }

    #[test]
    fn parse_gemini_canvas_music_ws_setup_binary_reports_invalid_json_contract() {
        let error = parse_gemini_canvas_music_ws_setup_binary(b"{not-json")
            .expect_err("invalid setup binary JSON should fail");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_invalid_json")
        );
        assert!(error.message.starts_with(
            "Gemini Canvas music websocket returned invalid binary JSON during setup:"
        ));
    }

    #[test]
    fn parse_gemini_canvas_music_ws_text_reports_invalid_json_contract() {
        let error = parse_gemini_canvas_music_ws_text("{not-json")
            .expect_err("invalid runtime JSON should fail");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_invalid_json")
        );
        assert!(error
            .message
            .starts_with("Gemini Canvas music websocket returned invalid JSON:"));
    }

    #[test]
    fn parse_gemini_canvas_music_ws_binary_reports_invalid_json_contract() {
        let error = parse_gemini_canvas_music_ws_binary(b"{not-json")
            .expect_err("invalid runtime binary JSON should fail");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_invalid_json")
        );
        assert!(error
            .message
            .starts_with("Gemini Canvas music websocket returned invalid binary JSON:"));
    }

    #[test]
    fn gemini_canvas_music_ws_pong_failed_error_matches_contract() {
        let error = gemini_canvas_music_ws_pong_failed_error("broken pipe");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_send_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "failed to reply to Gemini Canvas music websocket ping: broken pipe"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_send_frame_error_matches_contract() {
        let error = gemini_canvas_music_ws_send_frame_error("music client_content", "broken pipe");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_send_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "failed to send Gemini Canvas music client_content frame: broken pipe"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_request_failed_error_matches_contract() {
        let error = gemini_canvas_music_ws_request_failed_error(
            "build Gemini Canvas music websocket request",
            "bad header",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_request_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "failed to build Gemini Canvas music websocket request: bad header"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_connect_failed_error_matches_contract() {
        let error = gemini_canvas_music_ws_connect_failed_error("connection reset");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_connect_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "failed to connect to Gemini Canvas music websocket: connection reset"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_setup_transport_error_matches_contract() {
        let error = gemini_canvas_music_ws_setup_transport_error("closed");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_transport_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music websocket setup failed: closed"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_stream_transport_error_matches_contract() {
        let error = gemini_canvas_music_ws_stream_transport_error("reset");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_transport_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music websocket failed while streaming audio: reset"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_setup_timeout_error_matches_contract() {
        let error = gemini_canvas_music_ws_setup_timeout_error(Some("close code=1000".to_string()));
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_setup_timeout")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music websocket did not acknowledge setup.; last_setup_event=close code=1000"
        );
    }

    #[test]
    fn gemini_canvas_music_ws_authenticated_setup_error_matches_contract() {
        let error = gemini_canvas_music_ws_authenticated_setup_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_ws_setup_timeout")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music websocket could not establish an authenticated setup."
        );
    }

    #[test]
    fn gemini_canvas_music_ws_missing_audio_error_matches_contract() {
        let error = gemini_canvas_music_ws_missing_audio_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_missing_audio")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music websocket completed without audio chunks."
        );
    }

    #[test]
    fn build_gemini_canvas_music_ws_setup_frame_preserves_model_contract() {
        let frame = build_gemini_canvas_music_ws_setup_frame("lyria-realtime-exp");
        let parsed: Value = serde_json::from_str(&frame).expect("setup frame JSON");
        assert_eq!(parsed["setup"]["model"], "models/lyria-realtime-exp");
    }

    #[test]
    fn build_gemini_canvas_music_ws_client_content_frame_wraps_prompt_contract() {
        let frame = build_gemini_canvas_music_ws_client_content_frame("write a synth cue");
        let parsed: Value = serde_json::from_str(&frame).expect("client content frame JSON");
        assert_eq!(
            parsed["client_content"],
            gemini_canvas::build_music_client_content("write a synth cue")
        );
    }

    #[test]
    fn build_gemini_canvas_music_ws_generation_config_frame_wraps_config_contract() {
        let config = json!({
            "durationSeconds": 30,
            "temperature": 0.8
        });
        let frame = build_gemini_canvas_music_ws_generation_config_frame(&config);
        let parsed: Value = serde_json::from_str(&frame).expect("generation config frame JSON");
        assert_eq!(parsed["music_generation_config"], config);
    }

    #[test]
    fn build_gemini_canvas_music_ws_playback_control_frame_preserves_play_contract() {
        let frame = build_gemini_canvas_music_ws_playback_control_frame();
        let parsed: Value = serde_json::from_str(&frame).expect("playback control frame JSON");
        assert_eq!(parsed["playback_control"], "PLAY");
    }

    #[test]
    fn gemini_canvas_music_filtered_prompt_error_matches_contract() {
        let error = gemini_canvas_music_filtered_prompt_error(&Value::String("policy".to_string()));
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_music_filtered_prompt")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music prompt was filtered: \"policy\""
        );
    }

    #[test]
    fn gemini_canvas_music_missing_asset_error_matches_contract() {
        let error = gemini_canvas_music_missing_asset_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_music_asset"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music generation completed without a downloadable media asset."
        );
    }

    #[test]
    fn gemini_canvas_music_missing_api_key_error_matches_contract() {
        let error = gemini_canvas_music_missing_api_key_error();
        assert_eq!(error.http_status, Some(401));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_canvas_missing_api_key"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas official music API requires api_key."
        );
    }

    #[test]
    fn gemini_canvas_program_music_no_key_contract_missing_error_matches_contract() {
        let error =
            gemini_canvas_program_music_no_key_contract_missing_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_contract_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas no-key music StreamGenerate did not expose a usable action contract or audio asset."
        );
    }

    #[test]
    fn gemini_canvas_program_music_no_key_browserless_stage_incomplete_error_matches_contract() {
        let error = gemini_canvas_program_music_no_key_browserless_stage_incomplete_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_stage_incomplete")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but the current browserless replay still lacks the required page-state to advance beyond BardErrorInfo [1060]."
        );
    }

    #[test]
    fn gemini_canvas_program_music_no_key_prelude_stage_incomplete_error_matches_contract() {
        let error = gemini_canvas_program_music_no_key_prelude_stage_incomplete_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_stage_incomplete")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but even the current-page minimal prelude still could not advance beyond BardErrorInfo [1060]."
        );
    }

    #[test]
    fn gemini_canvas_program_music_no_key_post_1060_contract_missing_error_matches_contract() {
        let error = gemini_canvas_program_music_no_key_post_1060_contract_missing_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_contract_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas no-key music StreamGenerate advanced past the old 1060 gate, but still did not expose a usable action contract or audio asset."
        );
    }

    #[test]
    fn gemini_canvas_program_music_page_poll_failed_error_matches_contract() {
        let error = gemini_canvas_program_music_page_poll_failed_error(
            "gemini_canvas_compatible",
            "https://example.com/app",
            "timeout",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_page_poll_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music follow-up still did not expose a usable music asset after PCck7e/page poll. bootstrap_page=https://example.com/app; upstream=timeout"
        );
    }

    #[test]
    fn gemini_canvas_program_music_recent_page_poll_failed_error_matches_contract() {
        let error = gemini_canvas_program_music_recent_page_poll_failed_error(
            "gemini_canvas_compatible",
            "https://example.com/bootstrap",
            "https://example.com/recovered",
            "timeout",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_recent_page_poll_failed")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas music aPya6c follow-up advanced to accepted state, but recent-conversation page polling still did not expose a usable music asset. bootstrap_page=https://example.com/bootstrap; recovered_page=https://example.com/recovered; upstream=timeout"
        );
    }

    #[test]
    fn gemini_canvas_program_music_invoke_target_missing_error_matches_contract() {
        let error = gemini_canvas_program_music_invoke_target_missing_error();
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_invoke_target_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned music lane is missing an explicit app invoke target."
        );
    }

    #[test]
    fn gemini_canvas_program_music_browser_fallback_forbidden_error_matches_contract() {
        let error = gemini_canvas_program_music_browser_fallback_forbidden_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_browser_fallback_forbidden")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned music did not expose a direct no-key contract. Browser execution fallback is disabled on the default path."
        );
    }

    #[test]
    fn gemini_canvas_program_music_no_key_request_contract_missing_error_matches_contract() {
        let error = gemini_canvas_program_music_no_key_request_contract_missing_error(
            "gemini_canvas_compatible",
        );
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_contract_missing")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas program-owned music StreamGenerate contract is missing requestUrl/requestBody."
        );
    }

    #[test]
    fn decode_gemini_canvas_program_music_no_key_audio_reports_missing_audio_contract() {
        let error = decode_gemini_canvas_program_music_no_key_audio(None, None)
            .expect_err("missing audio should fail");
        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_missing_audio")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas preview-frame no-key music websocket completed without audio bytes."
        );
    }

    #[test]
    fn decode_gemini_canvas_program_music_no_key_audio_reports_invalid_audio_contract() {
        let error =
            decode_gemini_canvas_program_music_no_key_audio(Some("not-base64"), Some("audio/ogg"))
                .expect_err("invalid audio should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_program_music_no_key_invalid_audio")
        );
        assert!(
            error
                .message
                .starts_with("Gemini Canvas preview-frame no-key music websocket returned invalid base64 audio bytes:")
        );
    }

    #[test]
    fn select_preferred_gemini_canvas_music_asset_prefers_audio_over_video_placeholder() {
        let assets = vec![
            make_music_asset("video", "https://example.invalid/fallback.mp4", "video/mp4"),
            make_music_asset("audio", "https://example.invalid/final.mp3", "audio/mpeg"),
        ];

        let selected = select_preferred_gemini_canvas_music_asset(&assets)
            .expect("music asset should be selected");
        assert_eq!(selected.kind, "audio");
        assert_eq!(selected.mime_type, "audio/mpeg");
        assert_eq!(selected.url, "https://example.invalid/final.mp3");
    }

    #[test]
    fn gemini_canvas_music_body_indicates_accepted_progress_for_generation_tokens() {
        let body = concat!(
            ")]}'\n\n",
            "135\n",
            "[[\"wrb.fr\",null,\"[null,[\\\"c_music\\\",\\\"r_music\\\"],{\\\"11\\\":[\\\"Electronic Music Cue Generation\\\"],\\\"44\\\":true}]\"]]\n",
            "133\n",
            "[[\"wrb.fr\",null,\"[null,[\\\"c_music\\\",\\\"r_music\\\"],{\\\"26\\\":\\\"AwAAAAAAAAAQwBHO-LzoF6Ltg6rx4Bk\\\",\\\"44\\\":true}]\"]]\n"
        );
        assert!(gemini_canvas_music_body_indicates_accepted_progress(body));
    }

    #[test]
    fn build_gemini_canvas_music_accepted_response_from_body_prefers_stream_locator() {
        let body = concat!(
            ")]}'\n\n",
            "177\n",
            "[[\"wrb.fr\",null,\"[null,[\\\"c_611d46744df5f83e\\\",\\\"r_5d8c944e0c56973b\\\"],{\\\"18\\\":\\\"r_5d8c944e0c56973b\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n",
            "135\n",
            "[[\"wrb.fr\",null,\"[null,[\\\"c_611d46744df5f83e\\\",\\\"r_5d8c944e0c56973b\\\"],{\\\"11\\\":[\\\"Electronic Music Cue Generation\\\"],\\\"44\\\":true}]\"]]\n"
        );

        let response = build_gemini_canvas_music_accepted_response_from_body(
            "gemini-3-flash-preview",
            "test prompt",
            Some(30.0),
            body,
            Some("c_fallback"),
            Some("r_fallback"),
            Some("/app/fallback"),
        );
        assert_eq!(response["accepted"], json!(true));
        assert_eq!(response["completed"], json!(false));
        assert_eq!(response["conversation_id"], json!("c_611d46744df5f83e"));
        assert_eq!(response["response_id"], json!("r_5d8c944e0c56973b"));
        assert_eq!(response["app_path"], json!("/app/611d46744df5f83e"));
    }

    #[test]
    fn gemini_canvas_music_response_requires_browser_followup_for_pending_acceptance() {
        let response = json!({
            "accepted": true,
            "completed": false,
            "data": [{
                "kind": "audio",
                "status": "pending",
                "url": null,
            }]
        });

        assert!(gemini_canvas_music_response_requires_browser_followup(
            &response
        ));
    }

    #[test]
    fn gemini_canvas_music_response_does_not_require_browser_followup_for_completed_asset() {
        let response = json!({
            "completed": true,
            "data": [{
                "kind": "audio",
                "status": "completed",
                "url": "https://example.invalid/final.mp3",
            }]
        });

        assert!(!gemini_canvas_music_response_requires_browser_followup(
            &response
        ));
    }
}
