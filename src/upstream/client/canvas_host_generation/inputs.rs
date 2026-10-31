//! Canvas owns inline multimodal input conversion; shared API/Web encoders stay unchanged.
use super::*;
use crate::protocol::gemini::document_input;

fn input_error() -> GatewayError {
    invalid(
        "Canvas inputs require supported nonempty inline base64 media (16 parts, 16 MiB encoded total); remote references are not fetched.",
        "invalid_gemini_canvas_host_media_input",
    )
}

fn required_string(value: Option<&Value>) -> Result<&str, GatewayError> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(input_error)
}

fn data_uri(uri: &str) -> Result<(String, String), GatewayError> {
    let (header, data) = uri.split_once(',').ok_or_else(input_error)?;
    let mime = header
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .ok_or_else(input_error)?;
    Ok((mime.to_string(), data.to_string()))
}

fn media_fields(part: &Value) -> Result<Option<(String, String, bool)>, GatewayError> {
    if let Some((mime, data)) = document_input::openai_file(part).map_err(|_| input_error())? {
        return Ok(Some((mime.to_string(), data.to_string(), true)));
    }
    let aliases = ["inlineData", "inline_data", "fileData", "file_data"];
    if aliases
        .iter()
        .filter(|name| part.get(**name).is_some())
        .count()
        > 1
    {
        return Err(input_error());
    }
    match part.get("type").and_then(Value::as_str) {
        Some("input_audio") => {
            let audio = part.get("input_audio").ok_or_else(input_error)?;
            let mime = match required_string(audio.get("format"))? {
                "wav" => "audio/wav",
                "mp3" => "audio/mpeg",
                _ => return Err(input_error()),
            };
            return Ok(Some((
                mime.to_string(),
                required_string(audio.get("data"))?.to_string(),
                true,
            )));
        }
        Some("image_url") => {
            let image = part.get("image_url").ok_or_else(input_error)?;
            let (mime, data) = data_uri(required_string(image.get("url").or(Some(image)))?)?;
            if !mime.starts_with("image/") {
                return Err(input_error());
            }
            return Ok(Some((mime, data, true)));
        }
        _ => {}
    }
    if let Some(inline) = part.get("inlineData").or_else(|| part.get("inline_data")) {
        if inline.get("mimeType").is_some() && inline.get("mime_type").is_some() {
            return Err(input_error());
        }
        return Ok(Some((
            required_string(inline.get("mimeType").or_else(|| inline.get("mime_type")))?
                .to_string(),
            required_string(inline.get("data"))?.to_string(),
            false,
        )));
    }
    if let Some(file) = part.get("fileData").or_else(|| part.get("file_data")) {
        let (mime, data) = data_uri(required_string(
            file.get("fileUri").or_else(|| file.get("file_uri")),
        )?)?;
        // The data URI is authoritative: the shared image encoder guesses PNG for data URLs.
        return Ok(Some((mime, data, false)));
    }
    Ok(None)
}

fn normalize_part(
    part: &mut Value,
    count: &mut usize,
    total: &mut usize,
) -> Result<(), GatewayError> {
    let Some((mime, data, replace)) = media_fields(part)? else {
        return Ok(());
    };
    let mime = mime.to_ascii_lowercase();
    let document = document_input::mime_and_extension(&mime).is_some();
    *count += 1;
    *total = total.checked_add(data.len()).ok_or_else(input_error)?;
    if *count > 16
        || *total > MAX_INLINE_BYTES
        || data.is_empty()
        || !(document
            || matches!(
                mime.as_str(),
                "image/png"
                    | "image/jpeg"
                    | "image/webp"
                    | "image/gif"
                    | "audio/wav"
                    | "audio/x-wav"
                    | "audio/mpeg"
                    | "audio/mp3"
                    | "audio/ogg"
                    | "audio/flac"
                    | "audio/webm"
                    | "audio/mp4"
                    | "audio/aiff"
                    | "video/mp4"
                    | "video/webm"
                    | "video/quicktime"
                    | "video/mpeg"
            ))
    {
        return Err(input_error());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|_| input_error())?;
    if bytes.is_empty() || (document && !document_input::valid_bytes(&mime, &bytes)) {
        return Err(input_error());
    }
    if replace {
        *part = json!({});
    }
    let object = part.as_object_mut().ok_or_else(input_error)?;
    for name in ["inlineData", "inline_data", "fileData", "file_data"] {
        object.remove(name);
    }
    object.insert("inlineData".into(), json!({"mimeType":mime,"data":data}));
    Ok(())
}

pub(in crate::upstream::client) fn text_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let mut body = gemini_canvas::build_text_request_body(req, model);
    let (mut count, mut total) = (0, 0);
    let contents = body
        .get_mut("contents")
        .and_then(Value::as_array_mut)
        .ok_or_else(input_error)?;
    for content in contents {
        for part in content
            .get_mut("parts")
            .and_then(Value::as_array_mut)
            .ok_or_else(input_error)?
        {
            normalize_part(part, &mut count, &mut total)?;
        }
    }
    Ok(body)
}

#[cfg(test)]
#[path = "inputs/tests.rs"]
mod tests;
