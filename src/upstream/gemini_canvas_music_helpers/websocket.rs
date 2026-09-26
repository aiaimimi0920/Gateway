//! Music WebSocket frames, chunk decoding, and transport error contracts.

use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;

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
