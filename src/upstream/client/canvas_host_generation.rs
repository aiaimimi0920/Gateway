//! GenerateContent uses the selected account's Canvas HTTP host RPC, never a browser or Web composer.
use super::*;
use crate::protocol::gemini::api as surface;

const MAX_INLINE_BYTES: usize = 16 * 1024 * 1024;

#[path = "canvas_host_generation/inputs.rs"]
mod inputs;
pub(super) use inputs::text_request;
#[path = "canvas_host_generation/http.rs"]
mod http;
#[path = "canvas_host_generation/transcription.rs"]
mod transcription;

struct InlineMedia {
    mime_type: String,
    bytes: Vec<u8>,
}

fn invalid(message: &str, code: &str) -> GatewayError {
    GatewayError::bad_request(message)
        .with_provider("gemini_canvas_web_reverse_compatible")
        .with_code(code)
}

pub(super) fn media_contract_unavailable(operation: &str) -> GatewayError {
    without_host_replay(GatewayError::service_unavailable(format!(
        "Canvas {operation} has no verified permitted host transport; no Web composer fallback was used."
    ))
    .with_provider("gemini_canvas_web_reverse_compatible")
    .with_code("gemini_canvas_host_media_contract_unavailable"))
}

pub(super) fn without_host_replay(mut error: GatewayError) -> GatewayError {
    // Neither transport failure nor an unusable response proves non-execution.
    // Preserve the original diagnosis but prohibit same-account and sibling replay.
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Canvas host generation is not safe to replay automatically.".into(),
    };
    error
}

fn validate_model_path(model: &str) -> Result<(), GatewayError> {
    if model.len() > 256
        || !model
            .as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        || !model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(invalid(
            "Canvas model must be a single model identifier.",
            "invalid_gemini_canvas_host_model",
        ));
    }
    Ok(())
}

fn image_request(req: &CanonicalRelayRequest, model: &str) -> Result<Value, GatewayError> {
    gemini_canvas::resolve_direct_http_image_model(model)?;
    if req.raw_body.get("n").is_some_and(|n| n.as_u64() != Some(1)) {
        return Err(gemini_canvas::unsupported_modular_image_count_error(
            "gemini_canvas_web_reverse_compatible",
        ));
    }
    gemini_canvas::prefers_url_response(req)?;
    if req.raw_body.get("mask").is_some_and(|mask| !mask.is_null()) {
        return Err(invalid(
            "Canvas inline image editing does not support a mask.",
            "unsupported_gemini_canvas_host_mask",
        ));
    }
    let body = gemini_canvas::build_image_request_body(req, model);
    if req.endpoint_kind == EndpointKind::ImagesEdits {
        let parts = body
            .get("contents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|content| content.get("parts").and_then(Value::as_array))
            .flatten();
        let mut count = 0;
        let mut total = 0usize;
        for part in parts {
            if part.get("fileData").is_some() {
                return Err(invalid(
                    "Canvas edits require inline image data, not remote file references.",
                    "invalid_gemini_canvas_host_edit_input",
                ));
            }
            let Some(inline) = part.get("inlineData") else {
                continue;
            };
            let mime = inline.get("mimeType").and_then(Value::as_str).unwrap_or("");
            let data = inline.get("data").and_then(Value::as_str).unwrap_or("");
            total = total.saturating_add(data.len());
            count += 1;
            if count > 8
                || total > MAX_INLINE_BYTES
                || !mime.starts_with("image/")
                || data.is_empty()
                || base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .is_err()
            {
                return Err(invalid(
                    "Canvas edits require 1-8 nonempty base64 images within 16 MiB encoded total.",
                    "invalid_gemini_canvas_host_edit_input",
                ));
            }
        }
        if count == 0 {
            return Err(invalid(
                "Canvas editing requires an input image.",
                "missing_gemini_canvas_host_edit_input",
            ));
        }
    }
    Ok(body)
}

fn inline_media(body: &Value, prefix: &str) -> Result<InlineMedia, GatewayError> {
    for candidate in body
        .get("candidates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if candidate.get("finishReason").and_then(Value::as_str) != Some("STOP") {
            continue;
        }
        for part in candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(inline) = part.get("inlineData").or_else(|| part.get("inline_data")) else {
                continue;
            };
            let mime = inline
                .get("mimeType")
                .or_else(|| inline.get("mime_type"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if !mime.starts_with(prefix) || mime.len() > 128 {
                continue;
            }
            let data = inline.get("data").and_then(Value::as_str).unwrap_or("");
            if data.is_empty() || data.len() > MAX_INLINE_BYTES {
                break;
            }
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                if !bytes.is_empty() {
                    return Ok(InlineMedia {
                        mime_type: mime.to_string(),
                        bytes,
                    });
                }
            }
        }
    }
    Err(without_host_replay(
        GatewayError::server_error(
            "Canvas host returned no complete nonempty media of the requested type.",
        )
        .with_provider("gemini_canvas_web_reverse_compatible")
        .with_code("gemini_canvas_host_media_missing"),
    ))
}

fn tts_format(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    let format = match req.raw_body.get("response_format") {
        None => "wav".to_string(),
        Some(Value::String(value)) => value.trim().to_ascii_lowercase(),
        _ => {
            return Err(invalid(
                "Canvas speech response_format must be a string.",
                "invalid_gemini_canvas_host_audio_format",
            ))
        }
    };
    if !matches!(format.as_str(), "wav" | "pcm" | "opus" | "ogg") {
        return Err(invalid(
            "Canvas speech supports wav, pcm and opus/ogg responses.",
            "unsupported_gemini_canvas_host_audio_format",
        ));
    }
    Ok(format)
}

// Minimum container admission, not a replacement for a media decoder. All offsets stay bounded.
fn valid_wav(bytes: &[u8]) -> bool {
    if bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8 != bytes.len()
    {
        return false;
    }
    let (mut offset, mut format, mut data) = (12usize, false, false);
    while offset + 8 <= bytes.len() {
        let length = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let Some(end) = (offset + 8)
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
        else {
            return false;
        };
        match &bytes[offset..offset + 4] {
            b"fmt " => format = length >= 16,
            b"data" => data = length > 0,
            _ => {}
        }
        offset = end + length % 2;
    }
    offset == bytes.len() && format && data
}

fn valid_ogg_opus(bytes: &[u8]) -> bool {
    let (mut offset, mut first, mut ended) = (0usize, true, false);
    let mut serial = None;
    let mut sequence = 0u32;
    while offset + 27 <= bytes.len() && !ended {
        let header = &bytes[offset..offset + 27];
        if &header[..4] != b"OggS" || header[4] != 0 {
            return false;
        }
        let page_serial = u32::from_le_bytes(header[14..18].try_into().unwrap());
        let page_sequence = u32::from_le_bytes(header[18..22].try_into().unwrap());
        if serial.is_some_and(|value| value != page_serial) || page_sequence != sequence {
            return false;
        }
        serial = Some(page_serial);
        sequence = sequence.wrapping_add(1);
        let start = offset + 27;
        let count = usize::from(header[26]);
        let Some(lacing) = bytes.get(start..start + count) else {
            return false;
        };
        let length: usize = lacing.iter().map(|value| usize::from(*value)).sum();
        let Some(body) = bytes.get(start + count..start + count + length) else {
            return false;
        };
        if first && (header[5] & 2 == 0 || body.len() < 19 || !body.starts_with(b"OpusHead")) {
            return false;
        }
        first = false;
        ended = header[5] & 4 != 0;
        offset = start + count + length;
    }
    !first && ended && offset == bytes.len()
}

fn audio_response(
    req: &CanonicalRelayRequest,
    media: InlineMedia,
) -> Result<BinaryUpstreamResponse, GatewayError> {
    let mime = media.mime_type.trim().to_ascii_lowercase();
    let essence = mime.split(';').next().unwrap_or("").trim();
    let pcm = matches!(essence, "audio/l16" | "audio/pcm" | "audio/raw");
    if tts_format(req)? == "pcm" && !pcm {
        return Err(invalid(
            "Canvas did not return PCM audio; compressed bytes cannot be relabeled as PCM.",
            "gemini_canvas_host_audio_format_mismatch",
        ));
    }
    let (mut rate, mut channels, mut google_pcm) = (24_000u32, 1u32, false);
    for parameter in mime.split(';').skip(1).map(str::trim) {
        let Some((key, value)) = parameter.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key == "codec" && value == "pcm" {
            google_pcm = true;
        }
        let max = match key {
            "rate" => 192_000,
            "channels" => 8,
            _ => continue,
        };
        let parsed = value
            .parse::<u32>()
            .ok()
            .filter(|value| *value > 0 && *value <= max)
            .ok_or_else(|| {
                invalid(
                    "Canvas returned invalid PCM audio parameters.",
                    "gemini_canvas_host_invalid_audio_parameters",
                )
            })?;
        if key == "rate" {
            rate = parsed;
        } else {
            channels = parsed;
        }
    }
    let valid = if pcm {
        !media.bytes.is_empty() && media.bytes.len() % (2 * channels as usize) == 0
    } else {
        match essence {
            "audio/wav" | "audio/x-wav" => valid_wav(&media.bytes),
            "audio/ogg" | "audio/opus" => valid_ogg_opus(&media.bytes),
            _ => false,
        }
    };
    if !valid {
        return Err(invalid(
            "Canvas returned malformed or unsupported audio data.",
            "gemini_canvas_host_invalid_audio_data",
        ));
    }
    // Google Canvas's L16 + codec=pcm uses raw little-endian PCM, unlike generic RFC L16.
    // Normalize only this host contract; do not change the shared API/Web encoder.
    let mime_type = if pcm {
        let essence = if essence == "audio/l16" && google_pcm {
            "audio/pcm"
        } else {
            essence
        };
        format!("{essence};rate={rate};channels={channels}")
    } else {
        essence.to_string()
    };
    let audio = gemini_canvas::GeminiCanvasAudio {
        bytes: media.bytes,
        mime_type,
    };
    let (bytes, content_type) = surface::build_audio_binary_response(req, &audio)?;
    Ok(BinaryUpstreamResponse {
        body: bytes.into(),
        content_type: Some(content_type),
        extra_headers: vec![],
    })
}

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_host_generate(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        validate_model_path(model)?;
        self.execute_gemini_canvas_http_host_generate(payload, model, body, timeout)
            .await
            .map_err(without_host_replay)
    }

    pub(super) async fn execute_gemini_canvas_host_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Value, GatewayError> {
        let prompt = gemini_canvas::prompt_from_request(
            req,
            "Canvas image requests require a prompt.",
            "missing_gemini_canvas_host_image_prompt",
        )?;
        let request = image_request(req, model)?;
        let body = self
            .execute_gemini_canvas_host_generate(
                payload,
                model,
                &request,
                self.timeout.max(Duration::from_secs(240)),
            )
            .await?;
        let media = inline_media(&body, "image/")?;
        let image = gemini_canvas::GeminiCanvasImage {
            bytes: media.bytes,
            mime_type: media.mime_type,
        };
        gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &[image])
    }

    pub(super) async fn execute_gemini_canvas_host_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        gemini_canvas::prompt_for_text_request(
            req,
            "Canvas speech requires input text.",
            "missing_gemini_canvas_host_tts_prompt",
        )?;
        // Validate the response contract before sending a potentially billable generation.
        tts_format(req)?;
        let model = if model.is_empty() {
            gemini_canvas::GEMINI_CANVAS_DEFAULT_TTS_MODEL
        } else {
            gemini_canvas::resolve_text_model(model)?
        };
        let mut request = gemini_canvas::build_tts_request_body(req, model);
        // Google's normally generated Canvas TTS app includes model in both URL and body.
        request["model"] = json!(model);
        let body = self
            .execute_gemini_canvas_host_generate(
                payload,
                model,
                &request,
                self.timeout.max(Duration::from_secs(300)),
            )
            .await?;
        let media = inline_media(&body, "audio/")?;
        audio_response(req, media)
    }
}

#[cfg(test)]
#[path = "canvas_host_generation/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "canvas_host_generation/http_contract_tests.rs"]
mod http_contract_tests;
