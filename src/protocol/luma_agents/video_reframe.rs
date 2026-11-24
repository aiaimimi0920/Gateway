//! Ray 3.2 outpainting uses one video source, not image anchors or video-edit controls.
use super::{invalid_request, media_ref, request::choice};
use crate::error::GatewayError;
use serde_json::{Map, Value};

pub(super) fn pack(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
) -> Result<(), GatewayError> {
    if [
        "style",
        "output_format",
        "web_search",
        "image_ref",
        "images",
    ]
    .iter()
    .any(|key| raw.get(*key).is_some_and(|v| !v.is_null()))
    {
        return Err(invalid_request());
    }
    let source = media_ref::video(raw.get("source").ok_or_else(invalid_request)?)?;
    body.insert("source".into(), source);
    choice(
        raw,
        body,
        "aspect_ratio",
        &["9:16", "3:4", "1:1", "4:3", "16:9", "21:9"],
    )?;
    if !body.contains_key("aspect_ratio") {
        return Err(invalid_request());
    }
    let empty = Map::new();
    let controls = match raw.get("video").filter(|v| !v.is_null()) {
        None => &empty,
        Some(value) => value.as_object().ok_or_else(invalid_request)?,
    };
    if controls
        .keys()
        .any(|key| !["duration", "resolution", "source_position"].contains(&key.as_str()))
    {
        return Err(invalid_request());
    }
    let mut video = Map::new();
    choice(controls, &mut video, "duration", &["5s", "10s"])?;
    choice(
        controls,
        &mut video,
        "resolution",
        &["360p", "540p", "720p", "1080p"],
    )?;
    if video.get("resolution").and_then(Value::as_str) == Some("1080p")
        && matches!(body["aspect_ratio"].as_str(), Some("9:16" | "3:4"))
    {
        return Err(invalid_request());
    }
    if let Some(position) = controls.get("source_position").filter(|v| !v.is_null()) {
        let rect = position
            .as_object()
            .filter(|v| v.len() == 4)
            .ok_or_else(invalid_request)?;
        for (key, positive) in [
            ("x_norm", false),
            ("y_norm", false),
            ("w_norm", true),
            ("h_norm", true),
        ] {
            let number = rect
                .get(key)
                .and_then(Value::as_f64)
                .ok_or_else(invalid_request)?;
            if !number.is_finite()
                || number > 2.0
                || if positive {
                    number <= 0.0
                } else {
                    number < -2.0
                }
            {
                return Err(invalid_request());
            }
        }
        video.insert("source_position".into(), position.clone());
    }
    if !video.is_empty() {
        body.insert("video".into(), Value::Object(video));
    }
    // Source ownership, actual source duration and remote bytes are checked by Luma;
    // this mapper never fetches input URLs or pretends to have decoded the source video.
    Ok(())
}
