use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

use super::{chataibot_error, requested_image_count};

pub fn extract_image_urls(body: &Value) -> Result<Vec<String>, GatewayError> {
    if let Some(array) = body.as_array() {
        let urls = array
            .iter()
            .filter_map(|entry| {
                entry
                    .get("imageUrl")
                    .or_else(|| entry.get("image_url"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|url| !url.is_empty())
                    .map(ToString::to_string)
            })
            .collect::<Vec<_>>();
        if urls.is_empty() {
            return Err(chataibot_error(
                "Chataibot generation response did not include any imageUrl values.",
                "chataibot_missing_image_url",
            ));
        }
        return Ok(urls);
    }

    if let Some(url) = body
        .get("imageUrl")
        .or_else(|| body.get("image_url"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        return Ok(vec![url.to_string()]);
    }

    if let Some(message) = body
        .get("message")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("error").and_then(|v| v.as_str()))
    {
        return Err(chataibot_error(message, "chataibot_upstream_error"));
    }

    Err(chataibot_error(
        "Chataibot response did not include any image URLs.",
        "chataibot_missing_image_url",
    ))
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    urls: &[String],
) -> Result<Value, GatewayError> {
    let max_images = requested_image_count(req);
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(json!({
        "created": created,
        "data": urls
            .iter()
            .take(max_images)
            .map(|url| json!({
                "url": url,
                "revised_prompt": prompt,
            }))
            .collect::<Vec<_>>(),
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Value {
    let max_images = requested_image_count(req);
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "created": created,
        "data": images
            .iter()
            .take(max_images)
            .map(|(mime_type, bytes)| {
                json!({
                    "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
                    "revised_prompt": prompt,
                    "mime_type": mime_type,
                })
            })
            .collect::<Vec<_>>(),
    })
}
