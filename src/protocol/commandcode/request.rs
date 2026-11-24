//! Only native Chat may extend the shared lossless text/function boundary with images.
use super::{invalid, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole};
use crate::protocol::openai::text_tools_request;
use serde_json::Value;

pub(super) fn validate(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    if req.endpoint_kind != EndpointKind::ChatCompletions {
        return text_tools_request::validate(req, invalid);
    }
    text_tools_request::validate_with_content(req, invalid, |role, part| {
        role == MessageRole::User && matches!(part, ContentPart::ImageUrl { .. })
    })?;
    // Chat normalization discards unknown image properties and defaults malformed URLs.
    // Reject those shapes instead of shipping a different image than the caller requested.
    let parts = req.raw_body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|message| message["content"].as_array().into_iter().flatten());
    if parts
        .filter(|part| part["type"] == "image_url")
        .any(|part| !valid_image(part))
    {
        return Err(invalid());
    }
    Ok(())
}

fn valid_image(part: &Value) -> bool {
    let Some(image) = part["image_url"].as_object() else {
        return false;
    };
    part.as_object().is_some_and(|obj| obj.len() == 2)
        && image
            .keys()
            .all(|key| matches!(key.as_str(), "url" | "detail"))
        && image
            .get("detail")
            .is_none_or(|detail| matches!(detail.as_str(), Some("auto" | "low" | "high")))
        && image
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(valid_image_url)
}

fn valid_image_url(value: &str) -> bool {
    // Leave format and model compatibility to Command Code; Gateway never fetches the image.
    if let Some(data) = value.strip_prefix("data:image/") {
        return data
            .split_once(";base64,")
            .is_some_and(|(format, encoded)| !format.is_empty() && !encoded.is_empty());
    }
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
    })
}
