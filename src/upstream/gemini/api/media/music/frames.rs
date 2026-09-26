//! Official music server-frame decoding and bounded diagnostic previews.

use base64::Engine;
use serde_json::Value;

use crate::error::GatewayError;
use crate::upstream::gemini::api::execution::GEMINI_API_LEGACY_ADAPTER;

pub(super) fn compact_response_preview(body_text: &str, max_chars: usize) -> String {
    let normalized = body_text
        .chars()
        .filter(|value| !value.is_control() || matches!(*value, '\n' | '\r' | '\t'))
        .collect::<String>()
        .trim()
        .replace('\r', " ")
        .replace('\n', " ");
    if normalized.len() <= max_chars {
        normalized
    } else {
        // Keep the existing byte budget without slicing through a UTF-8 character.
        let mut end = max_chars;
        while !normalized.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &normalized[..end])
    }
}

pub(super) fn consume_music_server_content(
    value: &Value,
    audio_mime_type: &mut String,
    audio_bytes: &mut Vec<u8>,
) -> Result<bool, GatewayError> {
    let Some(server_content) = value
        .get("serverContent")
        .or_else(|| value.get("server_content"))
    else {
        return Ok(false);
    };
    let mut saw_audio = false;
    if let Some(chunks) = server_content
        .get("audioChunks")
        .or_else(|| server_content.get("audio_chunks"))
        .and_then(Value::as_array)
    {
        for chunk in chunks {
            let Some(raw_base64) = chunk.get("data").and_then(Value::as_str) else {
                continue;
            };
            let mime_type = chunk
                .get("mimeType")
                .or_else(|| chunk.get("mime_type"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("audio/l16;rate=48000;channels=2");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw_base64.as_bytes())
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Gemini official music websocket returned invalid audio base64: {error}"
                    ))
                    .with_provider(GEMINI_API_LEGACY_ADAPTER)
                    .with_code("gemini_official_music_ws_invalid_audio")
                })?;
            if !bytes.is_empty() {
                *audio_mime_type = mime_type.to_string();
                audio_bytes.extend_from_slice(&bytes);
                saw_audio = true;
            }
        }
        if saw_audio {
            return Ok(true);
        }
    }

    let Some(parts) = server_content
        .get("modelTurn")
        .or_else(|| server_content.get("model_turn"))
        .and_then(|turn| turn.get("parts"))
        .and_then(Value::as_array)
    else {
        return Ok(false);
    };
    for part in parts {
        let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) else {
            continue;
        };
        let Some(raw_base64) = inline_data.get("data").and_then(Value::as_str) else {
            continue;
        };
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("audio/l16;rate=48000;channels=2");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw_base64.as_bytes())
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "Gemini official music websocket returned invalid audio base64: {error}"
                ))
                .with_provider(GEMINI_API_LEGACY_ADAPTER)
                .with_code("gemini_official_music_ws_invalid_audio")
            })?;
        if !bytes.is_empty() {
            *audio_mime_type = mime_type.to_string();
            audio_bytes.extend_from_slice(&bytes);
            saw_audio = true;
        }
    }
    Ok(saw_audio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_response_preview_preserves_utf8_and_byte_limit() {
        for (input, limit, expected) in [
            ("a\u{e9}tail", 2, "a..."),
            ("\u{1f600}tail", 3, "..."),
            ("\u{1f600}tail", 4, "\u{1f600}..."),
        ] {
            assert_eq!(compact_response_preview(input, limit), expected);
        }
    }

    #[test]
    fn compact_response_preview_preserves_normalization_and_ascii_budget() {
        assert_eq!(compact_response_preview("abcdef", 3), "abc...");
        assert_eq!(compact_response_preview("abc", 0), "...");
        assert_eq!(
            compact_response_preview(" hi\r\nthere\u{0} ", 99),
            "hi  there"
        );
        assert_eq!(compact_response_preview("plain", 5), "plain");
    }

    #[test]
    fn consume_music_server_content_accepts_audio_chunks_shape() {
        let mut mime_type = String::from("audio/l16;rate=48000;channels=2");
        let mut audio_bytes = Vec::new();
        let payload = json!({
            "serverContent": {
                "audioChunks": [
                    {
                        "mimeType": "audio/pcm",
                        "data": "AQIDBA=="
                    }
                ]
            }
        });

        let saw_audio =
            consume_music_server_content(&payload, &mut mime_type, &mut audio_bytes).unwrap();

        assert!(saw_audio);
        assert_eq!(mime_type, "audio/pcm");
        assert_eq!(audio_bytes, vec![1, 2, 3, 4]);
    }
}
