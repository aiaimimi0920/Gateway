//! Chat content admission uses the resolved model; it never fetches or decodes media.
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, MessageRole};
use serde_json::Value;

pub(super) fn validate_roles(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    // Generic normalization can turn malformed text into an empty Text. Validate
    // its original shape without restoring raw history over canonical policy edits.
    for message in req.raw_body["messages"].as_array().into_iter().flatten() {
        content_parts(&message["content"])?;
    }
    // The generic Chat packer can flatten system and tool content into text.
    for message in &req.messages {
        for part in &message.content {
            if part.as_text().is_some() {
                continue;
            }
            if !matches!(part, ContentPart::ImageUrl { .. }) {
                return Err(unsupported_content());
            }
            if message.role != MessageRole::User {
                return Err(invalid_image());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_wire(body: &Value) -> Result<(), GatewayError> {
    let model = body["model"].as_str().unwrap_or_default();
    let messages = body["messages"]
        .as_array()
        .ok_or_else(unsupported_content)?;
    for message in messages {
        if !message.is_object() {
            return Err(unsupported_content());
        }
        for part in content_parts(&message["content"])? {
            if part["type"] == "text" {
                continue;
            }
            if message["role"] != "user" {
                return Err(invalid_image());
            }
            let formats: &[&str] = match model {
                "sensenova-6.8-flash" | "sensenova-6.8-flash-lite" => {
                    &["jpg", "jpeg", "png", "webp"]
                }
                "deepseek-flash" => &["jpeg", "png", "gif", "webp"],
                "kimi-k3" => &["jpg", "jpeg", "png", "webp", "gif", "bmp", "heic", "heif"],
                _ => {
                    return Err(GatewayError::bad_request(
                        "This SenseNova Chat model is not documented to accept images.",
                    )
                    .with_code("sensenova_chat_image_model_unsupported"))
                }
            };
            let url = part["image_url"]["url"]
                .as_str()
                .ok_or_else(invalid_image)?;
            if !inline_image(url, formats) && !(model != "kimi-k3" && remote_image(url)) {
                return Err(invalid_image());
            }
        }
    }
    Ok(())
}

fn content_parts(content: &Value) -> Result<&[Value], GatewayError> {
    let parts = match content {
        Value::String(_) | Value::Null => return Ok(&[]),
        Value::Array(parts) => parts,
        _ => return Err(unsupported_content()),
    };
    for part in parts {
        match part["type"].as_str() {
            Some("text") if part["text"].is_string() => {}
            Some("image_url") => {}
            _ => return Err(unsupported_content()),
        }
    }
    Ok(parts)
}

fn inline_image(url: &str, formats: &[&str]) -> bool {
    url.strip_prefix("data:image/")
        .and_then(|s| s.split_once(";base64,"))
        .is_some_and(|(format, data)| formats.contains(&format) && !data.is_empty())
}

fn remote_image(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https")
            && parsed.has_host()
            && parsed.username().is_empty()
            && parsed.password().is_none()
    })
}

fn invalid_image() -> GatewayError {
    GatewayError::bad_request("Unsupported image source or content role for SenseNova Chat.")
        .with_code("sensenova_chat_image_unsupported")
}

fn unsupported_content() -> GatewayError {
    GatewayError::bad_request("Unsupported content block for SenseNova Chat.")
        .with_code("sensenova_chat_content_unsupported")
}

#[cfg(test)]
mod tests;
