//! Select text and decode inline audio from browser invocation payloads.

use base64::Engine;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::upstream::gemini::canvas_program_web_reverse as program;

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
