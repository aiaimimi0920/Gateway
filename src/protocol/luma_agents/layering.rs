//! Image decomposition has its own short, optionally empty guidance and a single source.
use super::{invalid_request, media_ref, request::choice};
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use serde_json::{Map, Value};

pub(crate) fn normalize(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let prompt = body
        .get("prompt")
        .and_then(Value::as_str)
        .filter(|s| s.chars().count() <= 500)
        .ok_or_else(invalid_request)?
        .to_owned();
    if body.get("type").and_then(Value::as_str) != Some("layering") {
        return Err(invalid_request());
    }
    Ok(crate::protocol::gemini_business::image_generation_with_prompt(body, prompt))
}

pub(super) fn pack(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
) -> Result<(), GatewayError> {
    if [
        "aspect_ratio",
        "style",
        "output_format",
        "web_search",
        "image_ref",
        "images",
        "video",
    ]
    .iter()
    .any(|key| raw.get(*key).is_some_and(|v| !v.is_null()))
    {
        return Err(invalid_request());
    }
    body.insert(
        "source".into(),
        media_ref::checked(raw.get("source").ok_or_else(invalid_request)?, true)?,
    );
    if let Some(value) = raw.get("layering").filter(|v| !v.is_null()) {
        let options = value.as_object().ok_or_else(invalid_request)?;
        if options.keys().any(|key| key != "resolution") {
            return Err(invalid_request());
        }
        let mut result = Map::new();
        choice(options, &mut result, "resolution", &["1k", "2k"])?;
        body.insert("layering".into(), Value::Object(result));
    }
    Ok(())
}
