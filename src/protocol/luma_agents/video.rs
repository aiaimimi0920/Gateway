//! Ray 3.2 create controls and documented mutually exclusive combinations.
use super::{
    invalid_request, media_ref,
    request::{boolean, choice},
};
use crate::error::GatewayError;
use serde_json::{json, Map, Value};

pub(super) fn pack(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
) -> Result<(), GatewayError> {
    if [
        "style",
        "output_format",
        "web_search",
        "image_ref",
        "source",
        "images",
    ]
    .iter()
    .any(|k| raw.get(*k).is_some_and(|v| !v.is_null()))
    {
        return Err(invalid_request());
    }
    choice(
        raw,
        body,
        "aspect_ratio",
        &["9:16", "3:4", "1:1", "4:3", "16:9", "21:9"],
    )?;
    let empty = Map::new();
    let video = match raw.get("video").filter(|v| !v.is_null()) {
        None => &empty,
        Some(v) => v.as_object().ok_or_else(invalid_request)?,
    };
    if video.keys().any(|k| {
        ![
            "resolution",
            "duration",
            "loop",
            "hdr",
            "start_frame",
            "end_frame",
            "keyframes",
            "keyframe_indexes",
        ]
        .contains(&k.as_str())
    }) {
        // EXR/edit/reframe need separate output/operation owners, not silent dropping.
        return Err(invalid_request());
    }
    let mut result = Map::new();
    choice(
        video,
        &mut result,
        "resolution",
        &["360p", "540p", "720p", "1080p"],
    )?;
    choice(video, &mut result, "duration", &["5s", "10s"])?;
    boolean(video, &mut result, "loop")?;
    boolean(video, &mut result, "hdr")?;
    let long = result.get("duration") == Some(&json!("10s"));
    let hdr = result.get("hdr") == Some(&json!(true));
    let looping = result.get("loop") == Some(&json!(true));
    let mut anchors = Vec::new();
    for key in ["start_frame", "end_frame"] {
        if let Some(value) = video.get(key).filter(|v| !v.is_null()) {
            let value = media_ref::checked(value, true)?;
            anchors.push(value.clone());
            result.insert(key.into(), value);
        }
    }
    if (hdr
        && (long
            || looping
            || matches!(
                result.get("resolution").and_then(Value::as_str),
                Some("360p" | "540p")
            )))
        || (long && (!anchors.is_empty() || looping))
        || (looping && result.contains_key("end_frame"))
        || (anchors.iter().any(|v| v.get("generation_id").is_some()) && (anchors.len() != 1 || hdr))
    {
        return Err(invalid_request());
    }
    match (
        video.get("keyframes").filter(|v| !v.is_null()),
        video.get("keyframe_indexes").filter(|v| !v.is_null()),
    ) {
        (None, None) => (),
        (Some(frames), Some(indexes)) => {
            let frames = frames
                .as_array()
                .filter(|v| !v.is_empty() && v.len() <= 64)
                .ok_or_else(invalid_request)?;
            let indexes = indexes
                .as_array()
                .filter(|v| v.len() == frames.len())
                .ok_or_else(invalid_request)?;
            if !anchors.is_empty()
                || looping
                || indexes.iter().any(|v| {
                    !v.as_u64()
                        .is_some_and(|n| n <= if long { 240 } else { 120 })
                })
            {
                return Err(invalid_request());
            }
            let mut seen = std::collections::HashSet::new();
            if indexes.iter().any(|v| !seen.insert(v.as_u64().unwrap())) {
                return Err(invalid_request());
            }
            let frames = frames
                .iter()
                .map(|r| media_ref::checked(r, true))
                .collect::<Result<Vec<_>, _>>()?;
            result.insert("keyframes".into(), json!(frames));
            result.insert("keyframe_indexes".into(), json!(indexes));
        }
        _ => return Err(invalid_request()),
    }
    if !result.is_empty() {
        body.insert("video".into(), Value::Object(result));
    }
    Ok(())
}
