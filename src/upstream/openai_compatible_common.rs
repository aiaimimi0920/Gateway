use base64::Engine;
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

const RAW_REQUEST_BODY_BASE64_KEY: &str = "__gateway_raw_body_base64";
const RAW_REQUEST_BODY_CONTENT_TYPE_KEY: &str = "__gateway_raw_body_content_type";

fn session_header(id: &str) -> Option<HeaderValue> {
    if id.len() == 36 && uuid::Uuid::parse_str(id).is_ok() {
        HeaderValue::from_str(id).ok()
    } else {
        None
    }
}

fn metadata_affinity<'a>(get: impl Fn(&str) -> Option<&'a Value>) -> Option<HeaderValue> {
    ["root_session_id", "session_id"]
        .iter()
        .find_map(|key| get(key)?.as_str().and_then(session_header))
}

pub(super) fn apply_vibe_inference_headers(
    headers: &mut HeaderMap,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
) {
    if !crate::preset::is_mistral_vibe_wire(payload) {
        return;
    }
    let affinity = extra_headers
        .and_then(|extra| extra.get(crate::http::request_headers::VIBE_AFFINITY_CONTEXT))
        .map(String::as_str)
        .and_then(session_header)
        .or_else(|| metadata_affinity(|key| req.metadata.as_ref()?.get(key)));
    if let Some(value) = affinity {
        headers.insert("x-affinity", value);
    }
    headers.insert(
        rquest::header::ACCEPT,
        HeaderValue::from_static(if req.stream {
            "text/event-stream"
        } else {
            "application/json"
        }),
    );
}

pub(super) fn finalize_vibe_affinity(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
    headers: &mut HeaderMap,
) {
    if !crate::preset::is_mistral_vibe_wire(payload) {
        return;
    }
    let explicit = headers.remove("x-affinity");
    if plan.method != rquest::Method::POST
        || plan.url != "https://api.mistral.ai/v1/chat/completions"
    {
        return;
    }
    // A real CLI header takes precedence because subagents may use the root
    // session for affinity. For a body-only request, reuse the supplied session.
    let value = explicit
        .as_ref()
        .and_then(|header| header.to_str().ok())
        .and_then(session_header)
        .or_else(|| metadata_affinity(|key| plan.body.as_ref()?.get("metadata")?.get(key)));
    if let Some(value) = value {
        headers.insert("x-affinity", value);
    }
}

#[cfg(feature = "family-openai-compatible-official-api")]
pub(super) fn preserve_vibe_chat_wire_fields(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    use_responses_bridge: bool,
    body: &mut Value,
) {
    if use_responses_bridge {
        return;
    }
    let Some(fields) = body.as_object_mut() else {
        return;
    };
    let chat_surface = matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Messages
            | EndpointKind::Completions
    );
    if (crate::protocol::kilo_free::owns_payload(payload)
        || crate::protocol::vercel_free::owns_payload(payload))
        && req.endpoint_kind != EndpointKind::ChatCompletions
    {
        for key in ["store", "include", "text", "verbosity", "top_k"] {
            fields.remove(key);
        }
    }
    if !crate::preset::is_mistral_official_payload(payload) || !chat_surface {
        return;
    }
    if !req.extra.contains_key("stream_options") {
        fields.remove("stream_options");
    }
    if req.endpoint_kind != EndpointKind::ChatCompletions {
        // Codex/Responses and Messages set non-Chat fields; Mistral Chat rejects unknown keys.
        for key in ["store", "include", "text", "verbosity"] {
            fields.remove(key);
        }
        return;
    }
    if !crate::preset::is_mistral_vibe_wire(payload) {
        return;
    }
    // Preserve native key order only when the canonical packer made no
    // semantic changes. Routing, sanitization and operator overrides still win.
    for key in ["messages", "tools"] {
        if let Some(raw) = req.raw_body.get(key) {
            if let Some(packed) = fields.get_mut(key) {
                if *packed == *raw {
                    *packed = raw.clone();
                }
            }
        }
    }
}

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
