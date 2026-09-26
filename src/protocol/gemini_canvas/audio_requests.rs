use serde_json::Value;

use super::{batchexecute::build_text_batchexecute_request, defaults::*};
use crate::error::GatewayError;
use crate::protocol::gemini_web;

pub fn build_tts_trigger_request(
    response_id: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let response_id = response_id.trim();
    if response_id.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS trigger requires a non-empty StreamGenerate response id.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_response_id"));
    }
    build_text_batchexecute_request(
        GEMINI_CANVAS_TTS_TRIGGER_RPCID,
        Value::Array(vec![Value::String(response_id.to_string())]),
        bootstrap,
        source_path,
    )
}

pub fn build_music_trigger_request(
    response_id: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_tts_trigger_request(response_id, bootstrap, source_path)
}

pub fn infer_tts_export_locale(text: &str, fallback_locale: &str) -> String {
    let fallback = fallback_locale.trim();
    let fallback = if fallback.is_empty() {
        "en-US"
    } else {
        fallback
    };
    let mut segments = fallback.split(['-', '_']);
    let fallback_language = segments
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en");
    let region = segments
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("US")
        .to_ascii_uppercase();
    let detected_language = if text.chars().any(is_hangul) {
        "ko"
    } else if text.chars().any(is_hiragana_or_katakana) {
        "ja"
    } else if text.chars().any(is_cjk_unified_ideograph) {
        "zh"
    } else if text.chars().any(is_cyrillic) {
        "ru"
    } else if text.chars().any(|ch| ch.is_ascii_alphabetic()) {
        "en"
    } else {
        fallback_language
    };
    format!("{detected_language}-{region}")
}

pub fn build_tts_audio_export_request(
    text: &str,
    locale: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS export requires a non-empty response text payload.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_export_text"));
    }
    let locale = locale.trim();
    if locale.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS export requires a non-empty locale tag.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_export_locale"));
    }
    build_text_batchexecute_request(
        GEMINI_CANVAS_TTS_EXPORT_RPCID,
        Value::Array(vec![
            Value::Null,
            Value::String(text.to_string()),
            Value::String(locale.to_string()),
            Value::Null,
            Value::from(2),
        ]),
        bootstrap,
        source_path,
    )
}

fn is_cjk_unified_ideograph(ch: char) -> bool {
    matches!(ch as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF)
}

fn is_hiragana_or_katakana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF)
}

fn is_hangul(ch: char) -> bool {
    matches!(ch as u32, 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF)
}

fn is_cyrillic(ch: char) -> bool {
    matches!(ch as u32, 0x0400..=0x04FF | 0x0500..=0x052F)
}
