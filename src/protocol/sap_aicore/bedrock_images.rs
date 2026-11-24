//! SAP Bedrock projects admitted inline images into native byte blocks, never fetchable URLs.
use super::{
    image_input::{split_inline_url, InlineImages},
    invalid, GatewayError,
};
use crate::protocol::canonical::ContentPart;
use serde_json::{json, Value};

pub(super) fn project_user_content(
    raw: &[Value],
    canonical: &[ContentPart],
    budget: &mut InlineImages,
) -> Result<Vec<ContentPart>, GatewayError> {
    if raw.len() != canonical.len() {
        return Err(invalid());
    }
    let mut has_image = false;
    let parts = raw
        .iter()
        .zip(canonical)
        .map(|(raw, part)| {
            // Compare the wire-relevant source fields to the canonical projection. Reject fields
            // that would otherwise be silently dropped by normalization or by the native mapping.
            let object = raw
                .as_object()
                .filter(|value| value.len() == 2)
                .ok_or_else(invalid)?;
            match part {
                ContentPart::Text { text }
                    if raw["type"] == "text"
                        && !text.is_empty()
                        && raw["text"].as_str() == Some(text) =>
                {
                    Ok(part.clone())
                }
                ContentPart::ImageUrl { image_url, detail } if raw["type"] == "image_url" => {
                    let image = object
                        .get("image_url")
                        .and_then(Value::as_object)
                        .ok_or_else(invalid)?;
                    let raw_detail = image
                        .get("detail")
                        .map(|v| v.as_str().ok_or_else(invalid))
                        .transpose()?;
                    if image.len() != 1 + usize::from(raw_detail.is_some())
                        || image.get("url").and_then(Value::as_str) != Some(image_url)
                        || raw_detail != detail.as_deref()
                        || !matches!(raw_detail, None | Some("auto"))
                    {
                        return Err(invalid());
                    }
                    budget.admit(image_url, raw_detail).ok_or_else(invalid)?;
                    let (format, bytes) = split_inline_url(image_url).ok_or_else(invalid)?;
                    has_image = true;
                    Ok(ContentPart::Raw {
                        value: json!({"image":{"format":format,"source":{"bytes":bytes}}}),
                    })
                }
                _ => Err(invalid()),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    has_image.then_some(parts).ok_or_else(invalid)
}
