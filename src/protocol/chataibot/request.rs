use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

use super::DEFAULT_RATIO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChataibotUpload {
    pub mime_type: String,
    pub base64_data: String,
}

pub fn prompt_from_request(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    if let Some(prompt) = read_optional_string(&req.raw_body, &["prompt"]) {
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }

    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(
            GatewayError::bad_request("Chataibot image requests require a prompt.")
                .with_code("missing_prompt"),
        );
    }
    Ok(prompt)
}

pub fn requested_image_count(req: &CanonicalRelayRequest) -> usize {
    req.raw_body
        .get("n")
        .and_then(|v| v.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(1)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    let Some(value) = req.raw_body.get("response_format") else {
        return Ok(true);
    };
    let Some(format) = value.as_str() else {
        return Err(
            GatewayError::bad_request("response_format must be a string when provided.")
                .with_code("invalid_image_response_format"),
        );
    };
    match format {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "Chataibot image endpoints currently support response_format=url or b64_json.",
        )
        .with_code("unsupported_image_response_format")),
    }
}

pub fn aspect_ratio_from_request(req: &CanonicalRelayRequest) -> String {
    let Some(size) = read_optional_string(&req.raw_body, &["size"]) else {
        return DEFAULT_RATIO.to_string();
    };

    match size.as_str() {
        "1024x1024" | "1:1" => "1:1".to_string(),
        "1024x1792" | "9:16" => "9:16".to_string(),
        "1792x1024" | "16:9" => "16:9".to_string(),
        "auto" => DEFAULT_RATIO.to_string(),
        _ => DEFAULT_RATIO.to_string(),
    }
}

pub fn extract_uploads_from_request_body(
    body: &Value,
) -> Result<Vec<ChataibotUpload>, GatewayError> {
    let mut uploads = Vec::new();

    if let Some(image) = body.get("image") {
        uploads.push(parse_upload_value(image)?);
    }

    if let Some(images) = body.get("images").and_then(|v| v.as_array()) {
        for image in images {
            uploads.push(parse_upload_value(image)?);
        }
    }

    Ok(uploads)
}

fn parse_upload_value(value: &Value) -> Result<ChataibotUpload, GatewayError> {
    match value {
        Value::String(s) => parse_data_uri_upload(s),
        Value::Object(record) => {
            let mime_type = read_required_map_string(record, &["mimeType", "mime_type"])?;
            let data = read_required_map_string(record, &["base64", "data"])?;
            if data.starts_with("data:image/") {
                return parse_data_uri_upload(data);
            }
            Ok(ChataibotUpload {
                mime_type: mime_type.to_string(),
                base64_data: data.to_string(),
            })
        }
        _ => Err(GatewayError::bad_request(
            "Chataibot image uploads must be a data URL string or JSON upload object.",
        )
        .with_code("invalid_image_upload")),
    }
}

fn parse_data_uri_upload(value: &str) -> Result<ChataibotUpload, GatewayError> {
    let trimmed = value.trim();
    let Some(rest) = trimmed.strip_prefix("data:") else {
        return Err(GatewayError::bad_request(
            "Chataibot image uploads must use data:image/*;base64,... payloads.",
        )
        .with_code("invalid_image_upload"));
    };

    let Some((meta, base64_data)) = rest.split_once(',') else {
        return Err(GatewayError::bad_request(
            "Chataibot image uploads must use data:image/*;base64,... payloads.",
        )
        .with_code("invalid_image_upload"));
    };

    let Some(mime_type) = meta.strip_suffix(";base64") else {
        return Err(GatewayError::bad_request(
            "Chataibot image uploads must use base64-encoded data URLs.",
        )
        .with_code("invalid_image_upload"));
    };

    if !mime_type.starts_with("image/") || base64_data.trim().is_empty() {
        return Err(GatewayError::bad_request(
            "Chataibot image uploads must use image/* base64 data URLs.",
        )
        .with_code("invalid_image_upload"));
    }

    Ok(ChataibotUpload {
        mime_type: mime_type.to_string(),
        base64_data: base64_data.trim().to_string(),
    })
}

fn read_required_map_string<'a>(
    map: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Result<&'a str, GatewayError> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    Err(
        GatewayError::bad_request(format!("Missing required field `{}`.", keys[0]))
            .with_code("missing_required_field"),
    )
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
