//! Official inline audio decoding and PCM, WAV, and Opus response formatting.

use base64::Engine;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas as legacy;

pub fn extract_audio_from_generate_content_response(
    body: &Value,
) -> Result<legacy::GeminiCanvasAudio, GatewayError> {
    let Some(parts) = body
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
    else {
        return Err(GatewayError::server_error(
            "Gemini official TTS response did not contain candidates[0].content.parts.",
        )
        .with_code("gemini_official_missing_audio_parts"));
    };

    for part in parts {
        if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
            let mime_type = inline_data
                .get("mimeType")
                .or_else(|| inline_data.get("mime_type"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("audio/L16;codec=pcm;rate=24000")
                .to_string();
            let raw = inline_data
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GatewayError::server_error(
                        "Gemini official TTS response inlineData was missing base64 audio bytes.",
                    )
                    .with_code("gemini_official_missing_audio_data")
                })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Failed to decode Gemini official TTS audio payload: {error}"
                    ))
                    .with_code("gemini_official_invalid_audio_base64")
                })?;
            return Ok(legacy::GeminiCanvasAudio { mime_type, bytes });
        }
    }

    Err(GatewayError::server_error(
        "Gemini official TTS response did not include an inlineData audio part.",
    )
    .with_code("gemini_official_audio_part_not_found"))
}

pub fn build_audio_binary_response(
    req: &CanonicalRelayRequest,
    audio: &legacy::GeminiCanvasAudio,
) -> Result<(Vec<u8>, String), GatewayError> {
    match requested_tts_response_format(req)?.as_str() {
        "pcm" => Ok((audio.bytes.clone(), audio.mime_type.clone())),
        "wav" => {
            let normalized_mime = audio.mime_type.trim().to_ascii_lowercase();
            if normalized_mime.starts_with("audio/wav")
                || normalized_mime.starts_with("audio/x-wav")
            {
                Ok((audio.bytes.clone(), "audio/wav".to_string()))
            } else if is_pcm_audio_mime_type(&audio.mime_type) {
                Ok((
                    pcm_audio_to_wav_bytes(&audio.bytes, &audio.mime_type),
                    "audio/wav".to_string(),
                ))
            } else {
                Err(GatewayError::server_error(format!(
                    "Gemini official API returned `{}` audio which cannot satisfy response_format=wav.",
                    audio.mime_type
                ))
                .with_code("gemini_official_tts_wav_unavailable"))
            }
        }
        "opus" => {
            if is_ogg_opus_mime_type(&audio.mime_type) {
                Ok((audio.bytes.clone(), "audio/ogg".to_string()))
            } else {
                Err(GatewayError::server_error(format!(
                    "Gemini official API returned `{}` audio which cannot satisfy response_format=opus.",
                    audio.mime_type
                ))
                .with_code("gemini_official_tts_opus_unavailable"))
            }
        }
        _ => unreachable!("validated TTS response format"),
    }
}

fn requested_tts_response_format(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    let Some(format) = req
        .raw_body
        .get("response_format")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok("wav".to_string());
    };

    match format.as_str() {
        "wav" | "pcm" | "opus" | "ogg" => Ok(match format.as_str() {
            "ogg" => "opus".to_string(),
            _ => format,
        }),
        _ => Err(GatewayError::bad_request(
            "Gemini official TTS currently supports response_format=wav, pcm, or opus.",
        )
        .with_code("unsupported_gemini_official_tts_response_format")),
    }
}

fn is_pcm_audio_mime_type(mime_type: &str) -> bool {
    let normalized = mime_type.trim().to_ascii_lowercase();
    normalized.starts_with("audio/l16")
        || normalized.starts_with("audio/pcm")
        || normalized.starts_with("audio/raw")
}

fn is_ogg_opus_mime_type(mime_type: &str) -> bool {
    let normalized = mime_type.trim().to_ascii_lowercase();
    normalized.starts_with("audio/ogg") || normalized.starts_with("audio/opus")
}

fn parse_pcm_sample_rate(mime_type: &str) -> u32 {
    for segment in mime_type.split(';') {
        let trimmed = segment.trim();
        if let Some(rate) = trimmed.strip_prefix("rate=") {
            if let Ok(parsed) = rate.parse::<u32>() {
                return parsed.max(1);
            }
        }
    }
    24_000
}

fn parse_pcm_channels(mime_type: &str) -> u16 {
    for segment in mime_type.split(';') {
        let trimmed = segment.trim();
        if let Some(channels) = trimmed.strip_prefix("channels=") {
            if let Ok(parsed) = channels.parse::<u16>() {
                return parsed.max(1);
            }
        }
    }
    1
}

fn pcm_audio_to_wav_bytes(raw_pcm: &[u8], mime_type: &str) -> Vec<u8> {
    let sample_rate = parse_pcm_sample_rate(mime_type);
    let channels = parse_pcm_channels(mime_type);
    let bits_per_sample = 16u16;
    let mut pcm_le = raw_pcm.to_vec();
    if mime_type.to_ascii_lowercase().starts_with("audio/l16") {
        for chunk in pcm_le.chunks_exact_mut(2) {
            chunk.swap(0, 1);
        }
    }

    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_len = pcm_le.len() as u32;
    let riff_len = 36 + data_len;

    let mut wav = Vec::with_capacity(44 + pcm_le.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm_le);
    wav
}
