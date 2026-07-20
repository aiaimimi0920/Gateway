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
            "Gemini Canvas TTS response did not contain candidates[0].content.parts.",
        )
        .with_code("gemini_canvas_missing_audio_parts"));
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
                        "Gemini Canvas TTS response inlineData was missing base64 audio bytes.",
                    )
                    .with_code("gemini_canvas_missing_audio_data")
                })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(raw)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Failed to decode Gemini Canvas TTS audio payload: {error}"
                    ))
                    .with_code("gemini_canvas_invalid_audio_base64")
                })?;
            return Ok(legacy::GeminiCanvasAudio { mime_type, bytes });
        }
    }

    Err(GatewayError::server_error(
        "Gemini Canvas TTS response did not include an inlineData audio part.",
    )
    .with_code("gemini_canvas_audio_part_not_found"))
}

pub fn extract_inline_image_from_generate_content_response(
    body: &Value,
) -> Result<legacy::GeminiCanvasImage, GatewayError> {
    let Some(candidates) = body.get("candidates").and_then(Value::as_array) else {
        return Err(GatewayError::server_error(
            "Gemini Canvas image response did not contain candidates.",
        )
        .with_code("gemini_canvas_missing_image_candidates"));
    };

    for candidate in candidates {
        if let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
        {
            for part in parts {
                if let Some(inline_data) =
                    part.get("inlineData").or_else(|| part.get("inline_data"))
                {
                    let mime_type = inline_data
                        .get("mimeType")
                        .or_else(|| inline_data.get("mime_type"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("image/png")
                        .to_string();
                    if !mime_type.starts_with("image/") {
                        continue;
                    }
                    let raw = inline_data
                        .get("data")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            GatewayError::server_error(
                                "Gemini Canvas image response inlineData was missing base64 image bytes.",
                            )
                            .with_code("gemini_canvas_missing_image_data")
                        })?;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(raw)
                        .map_err(|error| {
                            GatewayError::server_error(format!(
                                "Failed to decode Gemini Canvas image payload: {error}"
                            ))
                            .with_code("gemini_canvas_invalid_image_base64")
                        })?;
                    return Ok(legacy::GeminiCanvasImage { mime_type, bytes });
                }
            }
        }
    }

    Err(GatewayError::server_error(
        "Gemini Canvas image response did not include an inlineData image part.",
    )
    .with_code("gemini_canvas_image_part_not_found"))
}

pub fn requested_tts_response_format(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
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
            "Gemini Canvas TTS currently supports response_format=wav, pcm, or opus.",
        )
        .with_code("unsupported_gemini_canvas_tts_response_format")),
    }
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
                    "Gemini Canvas returned `{}` audio which cannot satisfy response_format=wav.",
                    audio.mime_type
                ))
                .with_code("gemini_canvas_tts_wav_unavailable"))
            }
        }
        "opus" => {
            if is_ogg_opus_mime_type(&audio.mime_type) {
                Ok((audio.bytes.clone(), "audio/ogg".to_string()))
            } else {
                Err(GatewayError::server_error(format!(
                    "Gemini Canvas returned `{}` audio which cannot satisfy response_format=opus.",
                    audio.mime_type
                ))
                .with_code("gemini_canvas_tts_opus_unavailable"))
            }
        }
        _ => unreachable!("validated TTS response format"),
    }
}

pub fn extract_images_from_imagen_predict_response(
    body: &Value,
) -> Result<Vec<legacy::GeminiCanvasImage>, GatewayError> {
    let mut images = Vec::new();
    let mut seen_payloads = std::collections::HashSet::new();

    for collection in [
        body.get("generatedImages").and_then(Value::as_array),
        body.get("predictions").and_then(Value::as_array),
    ]
    .into_iter()
    .flatten()
    {
        for candidate in collection {
            let Some(image) = extract_imagen_predict_image(candidate) else {
                continue;
            };
            let dedupe_key = base64::engine::general_purpose::STANDARD.encode(&image.bytes);
            if seen_payloads.insert(dedupe_key) {
                images.push(image);
            }
        }
    }

    if images.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas Imagen predict response did not include any decodable images.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_imagen_predict_missing_image"));
    }

    Ok(images)
}

fn extract_imagen_predict_image(value: &Value) -> Option<legacy::GeminiCanvasImage> {
    let record = value.as_object()?;
    if let Some(image) = decode_imagen_image_object(record) {
        return Some(image);
    }
    record
        .get("image")
        .and_then(Value::as_object)
        .and_then(decode_imagen_image_object)
}

fn decode_imagen_image_object(
    record: &serde_json::Map<String, Value>,
) -> Option<legacy::GeminiCanvasImage> {
    let encoded = record
        .get("imageBytes")
        .or_else(|| record.get("bytesBase64Encoded"))
        .or_else(|| record.get("b64_json"))
        .or_else(|| record.get("b64Json"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()?;
    let mime_type = record
        .get("mimeType")
        .or_else(|| record.get("mime_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("image/png")
        .to_string();

    Some(legacy::GeminiCanvasImage { mime_type, bytes })
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
