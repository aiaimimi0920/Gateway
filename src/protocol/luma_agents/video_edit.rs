//! Ray 3.2 video conditioning; input references remain upstream-owned, never fetched locally.
use super::{
    invalid_request, media_ref,
    request::{boolean, choice},
};
use crate::error::GatewayError;
use serde_json::{Map, Value};

pub(super) fn pack(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
) -> Result<(), GatewayError> {
    // Edits derive aspect ratio from the source; reject controls we cannot honor.
    if [
        "aspect_ratio",
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
    body.insert(
        "source".into(),
        media_ref::video(raw.get("source").ok_or_else(invalid_request)?)?,
    );
    let empty = Map::new();
    let controls = match raw.get("video").filter(|v| !v.is_null()) {
        None => &empty,
        Some(value) => value.as_object().ok_or_else(invalid_request)?,
    };
    // EXR has a different asset contract. Duration is not part of this edit profile.
    if controls
        .keys()
        .any(|key| !["resolution", "hdr", "start_frame", "edit"].contains(&key.as_str()))
    {
        return Err(invalid_request());
    }
    let mut video = Map::new();
    choice(
        controls,
        &mut video,
        "resolution",
        &["360p", "540p", "720p", "1080p"],
    )?;
    boolean(controls, &mut video, "hdr")?;
    if video.get("hdr") == Some(&Value::Bool(true))
        && matches!(
            video.get("resolution").and_then(Value::as_str),
            Some("360p" | "540p")
        )
    {
        return Err(invalid_request());
    }
    let anchor = controls.get("start_frame").filter(|v| !v.is_null());
    if let Some(value) = anchor {
        video.insert("start_frame".into(), media_ref::checked(value, true)?);
    }
    if let Some(edit) = controls.get("edit").filter(|v| !v.is_null()) {
        let edit = pack_edit(edit, anchor.is_some())?;
        video.insert("edit".into(), Value::Object(edit));
    }
    if !video.is_empty() {
        body.insert("video".into(), Value::Object(video));
    }
    Ok(())
}

fn pack_edit(value: &Value, anchor: bool) -> Result<Map<String, Value>, GatewayError> {
    let edit = value.as_object().ok_or_else(invalid_request)?;
    if edit.keys().any(|key| {
        ![
            "auto_controls",
            "strength",
            "controls",
            "keyframes",
            "keyframe_indexes",
        ]
        .contains(&key.as_str())
    }) {
        return Err(invalid_request());
    }
    let mut result = Map::new();
    boolean(edit, &mut result, "auto_controls")?;
    choice(
        edit,
        &mut result,
        "strength",
        &[
            "adhere_1",
            "adhere_2",
            "adhere_3",
            "flex_1",
            "flex_2",
            "flex_3",
            "reimagine_1",
            "reimagine_2",
            "reimagine_3",
        ],
    )?;
    if let Some(value) = edit.get("controls").filter(|v| !v.is_null()) {
        if result.get("auto_controls") == Some(&Value::Bool(true)) {
            return Err(invalid_request());
        }
        result.insert("controls".into(), Value::Object(pack_signals(value)?));
    }
    match (
        edit.get("keyframes").filter(|v| !v.is_null()),
        edit.get("keyframe_indexes").filter(|v| !v.is_null()),
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
            let mut seen = std::collections::HashSet::new();
            if anchor
                || indexes
                    .iter()
                    .any(|v| v.as_u64().is_none_or(|index| !seen.insert(index)))
            {
                return Err(invalid_request());
            }
            let frames = frames
                .iter()
                .map(|value| media_ref::checked(value, true))
                .collect::<Result<Vec<_>, _>>()?;
            result.insert("keyframes".into(), Value::Array(frames));
            // Actual frame count/positions are checked by Luma against its source video.
            result.insert("keyframe_indexes".into(), Value::Array(indexes.clone()));
        }
        _ => return Err(invalid_request()),
    }
    Ok(result)
}

fn pack_signals(value: &Value) -> Result<Map<String, Value>, GatewayError> {
    let signals = value.as_object().ok_or_else(invalid_request)?;
    let mut result = Map::new();
    for (name, value) in signals {
        let tuning = match name.as_str() {
            "pose" => Some("strength"),
            "depth" => Some("blur"),
            "normals" => Some("augmentation"),
            "trajectory" => Some("sparsity"),
            "face" => None,
            _ => return Err(invalid_request()),
        };
        let signal = value.as_object().ok_or_else(invalid_request)?;
        if signal
            .keys()
            .any(|key| key != "enabled" && tuning != Some(key.as_str()))
        {
            return Err(invalid_request());
        }
        let mut item = Map::new();
        boolean(signal, &mut item, "enabled")?;
        if name == "pose" {
            choice(signal, &mut item, "strength", &["precise", "coarse"])?;
        } else if let Some(key) = tuning {
            if let Some(value) = signal.get(key).filter(|v| !v.is_null()) {
                if !value
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
                {
                    return Err(invalid_request());
                }
                item.insert(key.into(), value.clone());
            }
        }
        result.insert(name.clone(), Value::Object(item));
    }
    Ok(result)
}
