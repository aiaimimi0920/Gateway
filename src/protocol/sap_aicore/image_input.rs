//! Shared SAP inline image bounds; Chat and native Responses keep their own wire shape.
use super::invalid;
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole},
};
use base64::{engine::general_purpose::STANDARD, read::DecoderReader};
use serde_json::Value;

const MAX_IMAGES: usize = 10;
// Limit aggregate encoded data, not decoded pixels. The upstream validates the image itself.
const MAX_BASE64_BYTES: usize = 4 * 1024 * 1024;

pub(super) struct InlineImages {
    used: usize,
    remaining: usize,
}

impl InlineImages {
    pub(super) fn new() -> Self {
        Self {
            used: 0,
            remaining: MAX_BASE64_BYTES,
        }
    }

    pub(super) fn admit(&mut self, url: &str, detail: Option<&str>) -> Option<()> {
        if self.used >= MAX_IMAGES || !matches!(detail, None | Some("auto" | "low" | "high")) {
            return None;
        }
        self.remaining = admit_inline_url(url, self.remaining)?;
        self.used += 1;
        Some(())
    }

    pub(super) fn admit_responses_part(&mut self, part: &Value) -> bool {
        only_keys(part, &["type", "image_url", "detail"])
            && part.get("detail").is_none_or(Value::is_string)
            && part["image_url"]
                .as_str()
                .is_some_and(|url| self.admit(url, part["detail"].as_str()).is_some())
    }
}

pub(super) fn validate(req: &CanonicalRelayRequest, enabled: bool) -> Result<(), GatewayError> {
    let mut images = InlineImages::new();
    for message in &req.messages {
        for part in &message.content {
            match part {
                part if part.as_text().is_some() => (),
                ContentPart::ImageUrl { image_url, detail }
                    if enabled && message.role == MessageRole::User =>
                {
                    images
                        .admit(image_url, detail.as_deref())
                        .ok_or_else(invalid)?;
                }
                _ => return Err(invalid()),
            }
        }
    }

    // The canonical translators intentionally discard some protocol-specific fields. Check only
    // original image blocks here; build the upstream body from the canonical messages as usual.
    if !source_images_preserved(req, images.used)
        || req.raw_body.get("audio").is_some_and(|v| !v.is_null())
        || req.raw_body.get("modalities").is_some_and(|v| {
            !v.is_null() && !v.as_array().is_some_and(|m| m.len() == 1 && m[0] == "text")
        })
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn split_inline_url(url: &str) -> Option<(&'static str, &str)> {
    // Never resolve remote URLs or forward them for upstream dereferencing.
    url.strip_prefix("data:image/png;base64,")
        .map(|bytes| ("png", bytes))
        .or_else(|| {
            url.strip_prefix("data:image/jpeg;base64,")
                .map(|bytes| ("jpeg", bytes))
        })
}

fn admit_inline_url(url: &str, remaining: usize) -> Option<usize> {
    let (_, encoded) = split_inline_url(url)?;
    let remaining = remaining.checked_sub(encoded.len())?;
    // Validate syntax with constant extra space, without loading pixels or storing decoded data.
    let mut reader = DecoderReader::new(encoded.as_bytes(), &STANDARD);
    (std::io::copy(&mut reader, &mut std::io::sink()).ok()? > 0).then_some(remaining)
}

fn source_images_preserved(req: &CanonicalRelayRequest, expected: usize) -> bool {
    let (history, kind) = match req.endpoint_kind {
        EndpointKind::ChatCompletions => ("messages", "image_url"),
        EndpointKind::Messages => ("messages", "image"),
        EndpointKind::Responses => ("input", "input_image"),
        _ => return expected == 0,
    };
    if req.endpoint_kind == EndpointKind::Messages
        && array(req.raw_body.get("system"))
            .iter()
            .any(|p| p["type"] == kind)
    {
        return false;
    }
    let mut count = 0;
    for message in array(req.raw_body.get(history)) {
        for block in array(message.get("content"))
            .iter()
            .filter(|block| block["type"] == kind)
        {
            if message["role"] != "user"
                || !match req.endpoint_kind {
                    EndpointKind::ChatCompletions => {
                        only_keys(block, &["type", "image_url"])
                            && only_keys(&block["image_url"], &["url", "detail"])
                            && block["image_url"]
                                .get("detail")
                                .is_none_or(Value::is_string)
                    }
                    EndpointKind::Messages => {
                        only_keys(block, &["type", "source"])
                            && only_keys(&block["source"], &["type", "media_type", "data"])
                            && block["source"]["type"] == "base64"
                    }
                    // Responses normalization already preserves unsupported image shapes as Raw.
                    _ => true,
                }
            {
                return false;
            }
            count += 1;
        }
    }
    count == expected
}

fn array(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn only_keys(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|map| map.keys().all(|k| keys.contains(&k.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sap_aicore_native_responses_inline_images_share_the_request_budget() {
        let mut budget = InlineImages {
            used: 0,
            remaining: 8,
        };
        let part =
            serde_json::json!({"type":"input_image","image_url":"data:image/png;base64,YQ=="});
        assert!(budget.admit_responses_part(&part));
        assert!(budget.admit_responses_part(&part));
        assert!(!budget.admit_responses_part(&part));
        assert_eq!((budget.used, budget.remaining), (2, 0));
    }

    #[test]
    fn sap_aicore_inline_budget_counts_encoded_bytes_and_validates_standard_base64() {
        assert_eq!(admit_inline_url("data:image/jpeg;base64,YQ==", 4), Some(0));
        assert_eq!(admit_inline_url("data:image/png;base64,YQ==", 3), None);
        for invalid in ["", "YQ", "YQ==junk", "YR==", "YQ==\n", "YWJ_"] {
            assert_eq!(
                admit_inline_url(&format!("data:image/png;base64,{invalid}"), 100),
                None
            );
        }
    }
}
