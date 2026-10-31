//! Inline Web inputs are bounded data, never filenames or URLs to read locally.
use base64::Engine;
use serde_json::Value;

use crate::error::{FallbackHint, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole};
use crate::protocol::gemini::document_input;

pub(crate) const MAX_WEB_INLINE_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct WebUpload {
    pub file_name: String,
    pub mime_type: &'static str,
    pub bytes: Vec<u8>,
}

pub(crate) fn input_error() -> GatewayError {
    GatewayError::bad_request("Gemini Web requires supported inline user media, at most 16 files and 16 MiB base64 in total; remote references and media metadata are unsupported.")
        .with_provider("gemini_web_compatible")
        .with_code("invalid_gemini_web_media_input")
}

pub(crate) fn without_media_replay(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Web media upload or generation must not be replayed automatically.".into(),
    };
    error
}

pub(crate) fn has_media(req: &CanonicalRelayRequest) -> bool {
    req.messages
        .iter()
        .flat_map(|m| &m.content)
        .any(|part| !matches!(part, ContentPart::Text { .. }))
}

pub(crate) fn mime_and_extension(mime: &str) -> Result<(&'static str, &'static str), GatewayError> {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => Ok(("image/png", "png")),
        "image/jpeg" => Ok(("image/jpeg", "jpg")),
        "image/webp" => Ok(("image/webp", "webp")),
        "image/gif" => Ok(("image/gif", "gif")),
        "audio/wav" | "audio/x-wav" => Ok(("audio/wav", "wav")),
        "audio/mpeg" | "audio/mp3" => Ok(("audio/mpeg", "mp3")),
        "audio/flac" | "audio/x-flac" => Ok(("audio/flac", "flac")),
        "audio/ogg" => Ok(("audio/ogg", "ogg")),
        "audio/webm" => Ok(("audio/webm", "webm")),
        "audio/mp4" | "audio/x-m4a" => Ok(("audio/mp4", "m4a")),
        "video/mp4" => Ok(("video/mp4", "mp4")),
        "video/webm" => Ok(("video/webm", "webm")),
        "video/quicktime" => Ok(("video/quicktime", "mov")),
        _ => document_input::mime_and_extension(&mime.to_ascii_lowercase()).ok_or_else(input_error),
    }
}

fn data_url(url: &str) -> Result<(&str, &str), GatewayError> {
    let (header, data) = url
        .strip_prefix("data:")
        .and_then(|s| s.split_once(','))
        .ok_or_else(input_error)?;
    let mime = header
        .strip_suffix(";base64")
        .filter(|s| !s.contains(';'))
        .ok_or_else(input_error)?;
    Ok((mime, data))
}

fn raw_media(value: &Value) -> Result<(&str, &str), GatewayError> {
    if let Some(file) = document_input::openai_file(value).map_err(|_| input_error())? {
        return Ok(file);
    }
    let object = value.as_object().ok_or_else(input_error)?;
    if value.get("type").and_then(Value::as_str) == Some("input_audio") {
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "type" | "input_audio"))
        {
            return Err(input_error());
        }
        let audio = value
            .get("input_audio")
            .and_then(Value::as_object)
            .ok_or_else(input_error)?;
        if audio
            .keys()
            .any(|k| !matches!(k.as_str(), "format" | "data"))
        {
            return Err(input_error());
        }
        let mime = match audio.get("format").and_then(Value::as_str) {
            Some("wav") => "audio/wav",
            Some("mp3") => "audio/mpeg",
            _ => return Err(input_error()),
        };
        return Ok((
            mime,
            audio
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(input_error)?,
        ));
    }
    // Accept one native Gemini inline alias, without silently dropping video metadata.
    if object.len() != 1 {
        return Err(input_error());
    }
    let inline = value
        .get("inlineData")
        .or_else(|| value.get("inline_data"))
        .and_then(Value::as_object)
        .ok_or_else(input_error)?;
    if inline.len() != 2 || inline.contains_key("mimeType") == inline.contains_key("mime_type") {
        return Err(input_error());
    }
    let mime = inline
        .get("mimeType")
        .or_else(|| inline.get("mime_type"))
        .and_then(Value::as_str)
        .ok_or_else(input_error)?;
    Ok((
        mime,
        inline
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(input_error)?,
    ))
}

pub(crate) fn prepare_media(
    req: &CanonicalRelayRequest,
) -> Result<(CanonicalRelayRequest, Vec<WebUpload>), GatewayError> {
    let mut sanitized = req.clone();
    let mut uploads = Vec::new();
    let mut total = 0usize;
    for message in &mut sanitized.messages {
        let mut text = Vec::new();
        for part in std::mem::take(&mut message.content) {
            if matches!(part, ContentPart::Text { .. }) {
                text.push(part);
                continue;
            }
            if !matches!(message.role, MessageRole::User) {
                return Err(input_error());
            }
            let (mime, data) = match &part {
                ContentPart::ImageUrl { image_url, detail } => {
                    if detail.as_deref().is_some_and(|d| d != "auto") {
                        return Err(input_error());
                    }
                    let media = data_url(image_url)?;
                    if !media.0.starts_with("image/") {
                        return Err(input_error());
                    }
                    media
                }
                ContentPart::Raw { value } | ContentPart::Json { value } => raw_media(value)?,
                _ => unreachable!(),
            };
            total = total.saturating_add(data.len());
            if uploads.len() >= 16 || total > MAX_WEB_INLINE_BYTES || data.is_empty() {
                return Err(input_error());
            }
            let (mime_type, extension) = mime_and_extension(mime)?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| input_error())?;
            if bytes.is_empty()
                || (document_input::mime_and_extension(mime_type).is_some()
                    && !document_input::valid_bytes(mime_type, &bytes))
            {
                return Err(input_error());
            }
            uploads.push(WebUpload {
                file_name: format!("input-{}.{}", uploads.len() + 1, extension),
                mime_type,
                bytes,
            });
        }
        message.content = text;
    }
    Ok((sanitized, uploads))
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
