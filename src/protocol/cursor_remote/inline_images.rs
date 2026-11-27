//! Inline-only media admission. Native image bytes stay separate from the transcript.
//! The live owner retains a typed digest in its signature, never base64 history.
mod decode;
#[cfg(test)]
pub(super) mod tests;
use super::{request, tool_schema::invalid};
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(super) const MAX_REQUEST: usize = 4 * 1024 * 1024;
const MAX_IMAGE: usize = 1024 * 1024;
const MAX_TOTAL: usize = 2 * MAX_IMAGE;
const MAX_IMAGES: usize = 4;

#[derive(Clone, Default)]
pub(crate) struct Images(Vec<Image>);
#[derive(Clone)]
struct Image {
    message: usize,
    part: usize,
    mime: &'static str,
    digest: String,
    text_prefix: String,
    bytes: Arc<[u8]>,
}
impl std::fmt::Debug for Images {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Images")
            .field("count", &self.0.len())
            .finish()
    }
}
impl Images {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn binding(&self) -> Value {
        json!(self
            .0
            .iter()
            .map(|image| json!({
                "message":image.message, "part":image.part,
                "mime":image.mime, "sha256":image.digest, "textPrefix":image.text_prefix,
            }))
            .collect::<Vec<_>>())
    }
    /// Only a new Run is decoded. Continuations compare bounded byte digests and
    /// their exact positions, then send solely the tool result to the live worker.
    pub(crate) async fn worker_input(&self) -> Result<Value, GatewayError> {
        decode::validate(self.clone()).await?;
        Ok(json!(self
            .0
            .iter()
            .map(|image| json!({
                "mime":image.mime, "data":STANDARD.encode(&image.bytes),
            }))
            .collect::<Vec<_>>()))
    }
}

pub(super) fn extract(
    req: &CanonicalRelayRequest,
    chat: bool,
) -> Result<(CanonicalRelayRequest, Images), GatewayError> {
    if req.raw_body.to_string().len() > MAX_REQUEST {
        return Err(invalid());
    }
    let mut projected = req.clone();
    let mut images = Images::default();
    let mut total = 0;
    if let Some(items) = projected.raw_body["input"].as_array_mut() {
        let last_user = items.iter().rposition(|item| item["role"] == "user");
        for (index, item) in items.iter_mut().enumerate() {
            let Some(parts) = item["content"].as_array() else {
                continue;
            };
            if !parts.iter().any(|part| part["type"] == "input_image") {
                continue;
            }
            if Some(index) != last_user
                || item["role"] != "user"
                || item.get("type").is_some_and(|v| v != "message")
                || parts.len() > 128
            {
                return Err(invalid());
            }
            let mut text = Vec::new();
            for (part_index, part) in parts.iter().enumerate() {
                if part["type"] != "input_image" {
                    text.push(part.clone());
                    continue;
                }
                let object = part.as_object().ok_or_else(invalid)?;
                if object
                    .keys()
                    .any(|key| !["type", "image_url", "detail"].contains(&key.as_str()))
                    || part.get("detail").is_some_and(|v| v != "auto")
                    || images.0.len() >= MAX_IMAGES
                {
                    return Err(invalid());
                }
                let (mime, bytes) = data_url(part["image_url"].as_str().ok_or_else(invalid)?)?;
                total += bytes.len();
                if total > MAX_TOTAL {
                    return Err(invalid());
                }
                images.0.push(Image {
                    message: index,
                    part: part_index,
                    mime,
                    text_prefix: format!(
                        "{:x}",
                        Sha256::digest(serde_json::to_vec(&text).map_err(|_| invalid())?,)
                    ),
                    digest: format!("{:x}", Sha256::digest(&bytes)),
                    bytes: bytes.into(),
                });
            }
            item["content"] = json!(request::content_with_separator(
                &Value::Array(text),
                false,
                if chat { "" } else { "\n" },
            )?);
        }
    }
    // Images do not relax the existing text, declarations or metadata budget.
    if projected.raw_body.to_string().len() > 512 * 1024 {
        return Err(invalid());
    }
    Ok((projected, images))
}

fn data_url(raw: &str) -> Result<(&'static str, Vec<u8>), GatewayError> {
    let (mime, encoded) = if let Some(data) = raw.strip_prefix("data:image/png;base64,") {
        ("image/png", data)
    } else if let Some(data) = raw.strip_prefix("data:image/jpeg;base64,") {
        ("image/jpeg", data)
    } else {
        return Err(invalid());
    };
    if encoded.is_empty() || encoded.len() > MAX_IMAGE.div_ceil(3) * 4 {
        return Err(invalid());
    }
    // STANDARD rejects whitespace, missing padding and noncanonical trailing bits.
    let bytes = STANDARD.decode(encoded).map_err(|_| invalid())?;
    if bytes.is_empty()
        || bytes.len() > MAX_IMAGE
        || !(match mime {
            "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            _ => bytes.starts_with(b"\xff\xd8\xff"),
        })
    {
        return Err(invalid());
    }
    Ok((mime, bytes))
}
