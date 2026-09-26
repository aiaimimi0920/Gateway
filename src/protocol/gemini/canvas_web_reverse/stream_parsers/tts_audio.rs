//! Recover and decode the TTS export audio payload without changing fallback order.

use base64::Engine;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::gemini_canvas as legacy;

use super::wire_frames::{parse_response_frames, strip_xssi_prefix};

pub fn extract_audio_from_tts_export_response(
    body: &str,
) -> Result<legacy::GeminiCanvasAudio, GatewayError> {
    let normalized = strip_xssi_prefix(body);
    let (frames, _remainder) = parse_response_frames(normalized);
    let mut best_base64 = None;
    for frame in &frames {
        collect_tts_export_audio_base64(frame, &mut best_base64);
    }
    if best_base64.is_none() {
        best_base64 = scan_tts_export_audio_base64_in_text(normalized);
    }

    let best_base64 = match best_base64 {
        Some(payload) => payload,
        None if frames.is_empty() => {
            return Err(GatewayError::server_error(
                "Gemini Canvas TTS export response did not contain any parseable frames.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_invalid_frame_response"));
        }
        None => {
            return Err(GatewayError::server_error(
                "Gemini Canvas TTS export response did not include an inline audio payload.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_missing_audio"));
        }
    };
    decode_tts_export_audio_base64(&best_base64)
}

fn looks_like_tts_export_audio_base64(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 8 {
        return false;
    }
    (trimmed.starts_with("T2dnUw") || trimmed.starts_with("UklGR") || trimmed.starts_with("SUQz"))
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '\r' | '\n'))
}

fn infer_tts_export_audio_mime_type(base64_payload: &str) -> String {
    let trimmed = base64_payload.trim();
    if trimmed.starts_with("UklGR") {
        "audio/wav".to_string()
    } else if trimmed.starts_with("SUQz") {
        "audio/mpeg".to_string()
    } else {
        "audio/ogg".to_string()
    }
}

fn decode_tts_export_audio_base64(
    base64_payload: &str,
) -> Result<legacy::GeminiCanvasAudio, GatewayError> {
    let normalized = normalize_tts_export_audio_base64(base64_payload);
    let mime_type = infer_tts_export_audio_mime_type(&normalized);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(normalized.as_bytes())
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to decode Gemini Canvas TTS export audio payload: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_tts_export_invalid_audio_base64")
        })?;
    Ok(legacy::GeminiCanvasAudio { mime_type, bytes })
}

fn normalize_tts_export_audio_base64(base64_payload: &str) -> String {
    let mut normalized = base64_payload
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n' | ' ' | '\t'))
        .collect::<String>();
    let remainder = normalized.len() % 4;
    if remainder != 0 {
        normalized.extend(std::iter::repeat_n('=', 4 - remainder));
    }
    normalized
}

fn collect_tts_export_audio_base64(value: &Value, best_base64: &mut Option<String>) {
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if looks_like_tts_export_audio_base64(trimmed) {
            let should_replace = best_base64
                .as_ref()
                .map(|current| trimmed.len() > current.len())
                .unwrap_or(true);
            if should_replace {
                *best_base64 = Some(trimmed.to_string());
            }
        } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            collect_tts_export_audio_base64(&parsed, best_base64);
        }
    }
    if let Some(items) = value.as_array() {
        for item in items {
            collect_tts_export_audio_base64(item, best_base64);
        }
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            collect_tts_export_audio_base64(item, best_base64);
        }
    }
}

fn scan_tts_export_audio_base64_in_text(body: &str) -> Option<String> {
    let search_body = body
        .split_once(legacy::GEMINI_CANVAS_TTS_EXPORT_RPCID)
        .map(|(_, suffix)| suffix)
        .unwrap_or(body);
    let mut best_base64 = None;
    for prefix in ["T2dnUw", "UklGR", "SUQz"] {
        let mut search_start = 0usize;
        while search_start < search_body.len() {
            let Some(relative_start) = search_body[search_start..].find(prefix) else {
                break;
            };
            let start = search_start + relative_start;
            let end = scan_tts_export_audio_base64_end(search_body, start);
            if end > start {
                let candidate = search_body[start..end].trim();
                let normalized_candidate = normalize_tts_export_audio_base64(candidate);
                if looks_like_tts_export_audio_base64(&normalized_candidate) {
                    let should_replace = best_base64
                        .as_ref()
                        .map(|current: &String| normalized_candidate.len() > current.len())
                        .unwrap_or(true);
                    if should_replace {
                        best_base64 = Some(normalized_candidate);
                    }
                }
            }
            search_start = start + prefix.len();
        }
    }
    best_base64
}

fn scan_tts_export_audio_base64_end(body: &str, start: usize) -> usize {
    let mut end = start;
    for (offset, ch) in body[start..].char_indices() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '\r' | '\n') {
            end = start + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}
