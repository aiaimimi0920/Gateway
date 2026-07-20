use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

const RAW_REQUEST_BODY_BASE64_KEY: &str = "__gateway_raw_body_base64";
const RAW_REQUEST_BODY_CONTENT_TYPE_KEY: &str = "__gateway_raw_body_content_type";

pub fn build_audio_transcription_request_body(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let map = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Audio transcription request body must be a JSON object")
            .with_code("invalid_audio_transcription_body")
    })?;
    let file = map
        .get("file")
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            GatewayError::bad_request("Audio transcription requests require a `file` upload.")
                .with_code("missing_audio_file")
        })?;
    let file_name = file
        .get("file_name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("audio.bin");
    let mime_type = file
        .get("mime_type")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("application/octet-stream");
    let encoded = file
        .get("base64")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            GatewayError::bad_request("Audio transcription uploads require a base64 payload.")
                .with_code("missing_audio_file_payload")
        })?;
    let file_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| {
            GatewayError::bad_request("Audio transcription upload is not valid base64.")
                .with_code("invalid_audio_file_payload")
        })?;

    let boundary = format!("gw-audio-{}", uuid::Uuid::new_v4());
    let mut body = Vec::<u8>::new();
    append_multipart_text_field(&mut body, &boundary, "model", model);
    for (key, value) in map {
        if key == "file" || key == "model" {
            continue;
        }
        append_multipart_value(&mut body, &boundary, key, value);
    }
    append_multipart_file_field(
        &mut body,
        &boundary,
        "file",
        file_name,
        mime_type,
        &file_bytes,
    );
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    Ok(encode_raw_request_body(
        &body,
        &format!("multipart/form-data; boundary={boundary}"),
    ))
}

pub fn decode_raw_request_body(value: &Value) -> Option<(String, bytes::Bytes)> {
    let map = value.as_object()?;
    let content_type = map
        .get(RAW_REQUEST_BODY_CONTENT_TYPE_KEY)?
        .as_str()?
        .to_string();
    let encoded = map.get(RAW_REQUEST_BODY_BASE64_KEY)?.as_str()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    Some((content_type, bytes::Bytes::from(bytes)))
}

fn append_multipart_value(body: &mut Vec<u8>, boundary: &str, key: &str, value: &Value) {
    match value {
        Value::Null => {}
        Value::Array(values) => {
            let field_name = if key.ends_with("[]") {
                key.to_string()
            } else {
                format!("{key}[]")
            };
            for entry in values {
                append_multipart_value(body, boundary, &field_name, entry);
            }
        }
        Value::Bool(boolean) => {
            append_multipart_text_field(
                body,
                boundary,
                key,
                if *boolean { "true" } else { "false" },
            );
        }
        Value::Number(number) => {
            append_multipart_text_field(body, boundary, key, &number.to_string());
        }
        Value::String(text) => append_multipart_text_field(body, boundary, key, text),
        other => append_multipart_text_field(body, boundary, key, &other.to_string()),
    }
}

fn append_multipart_text_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn append_multipart_file_field(
    body: &mut Vec<u8>,
    boundary: &str,
    name: &str,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {mime_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n");
}

fn encode_raw_request_body(bytes: &[u8], content_type: &str) -> Value {
    json!({
        RAW_REQUEST_BODY_BASE64_KEY: base64::engine::general_purpose::STANDARD.encode(bytes),
        RAW_REQUEST_BODY_CONTENT_TYPE_KEY: content_type,
    })
}
