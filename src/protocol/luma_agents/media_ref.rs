//! References are sent to Luma, never fetched locally or treated as executable paths.
use super::invalid_request;
use crate::error::GatewayError;
use base64::Engine;
use serde_json::{json, Value};

pub(super) fn checked(value: &Value, generation: bool) -> Result<Value, GatewayError> {
    checked_media(value, generation, false)
}

pub(super) fn video(value: &Value) -> Result<Value, GatewayError> {
    checked_media(value, true, true)
}

fn checked_media(value: &Value, generation: bool, video: bool) -> Result<Value, GatewayError> {
    let map = value.as_object().ok_or_else(invalid_request)?;
    if map
        .keys()
        .any(|k| !["url", "data", "media_type", "file_id", "generation_id"].contains(&k.as_str()))
        || ["url", "data", "file_id", "generation_id"]
            .iter()
            .filter(|k| map.get(**k).is_some_and(|v| !v.is_null()))
            .count()
            != 1
    {
        return Err(invalid_request());
    }
    if let Some(url) = map.get("url").filter(|v| !v.is_null()) {
        let url = url
            .as_str()
            .filter(|s| s.len() <= 8192)
            .ok_or_else(invalid_request)?;
        let parsed = url::Url::parse(url).map_err(|_| invalid_request())?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
            || parsed.port().is_some()
        {
            return Err(invalid_request());
        }
        if video {
            let mime = map
                .get("media_type")
                .and_then(Value::as_str)
                .filter(|s| video_mime(s))
                .ok_or_else(invalid_request)?;
            return Ok(json!({"url":url,"media_type":mime}));
        } else if map.get("media_type").is_some_and(|v| !v.is_null()) {
            return Err(invalid_request());
        }
        return Ok(json!({"url":url}));
    }
    for key in ["file_id", "generation_id"] {
        if let Some(id) = map.get(key).filter(|v| !v.is_null()) {
            if (key == "generation_id" && !generation)
                || map.get("media_type").is_some_and(|v| !v.is_null())
                || !id
                    .as_str()
                    .is_some_and(|s| s.len() == 36 && uuid::Uuid::parse_str(s).is_ok())
            {
                return Err(invalid_request());
            }
            return Ok(json!({key:id}));
        }
    }
    let data = map
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let mime = map
        .get("media_type")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    // A local 8 MiB inline cap bounds decode and JSON copies; the upstream's limit is larger.
    if !(if video {
        video_mime(mime)
    } else {
        ["image/png", "image/jpeg", "image/webp", "image/gif"].contains(&mime)
    }) || data.len() > 8 * 1024 * 1024
        || data.is_empty()
        || base64::engine::general_purpose::STANDARD
            .decode(data)
            .is_err()
    {
        return Err(invalid_request());
    }
    Ok(json!({"data":data,"media_type":mime}))
}

fn video_mime(mime: &str) -> bool {
    mime.len() <= 127
        && mime.strip_prefix("video/").is_some_and(|subtype| {
            !subtype.is_empty()
                && subtype
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        })
}

pub(super) fn upload(value: &Value) -> Result<Value, GatewayError> {
    let map = value.as_object().ok_or_else(invalid_request)?;
    if map.keys().any(|k| {
        !["data_url", "mime_type", "base64_data", "base64", "filename"].contains(&k.as_str())
    }) || map.contains_key("base64_data") && map.contains_key("base64")
    {
        return Err(invalid_request());
    }
    if let Some(url) = map.get("data_url").and_then(Value::as_str) {
        if map.get("base64_data").is_some() || map.get("base64").is_some() {
            return Err(invalid_request());
        }
        let (mime, data) = url
            .strip_prefix("data:")
            .and_then(|s| s.split_once(";base64,"))
            .ok_or_else(invalid_request)?;
        if map.get("mime_type").is_some_and(|v| v != mime) {
            return Err(invalid_request());
        }
        checked(&json!({"data":data,"media_type":mime}), true)
    } else {
        checked(
            // `base64` is the existing public multipart/JSON upload projection.
            &json!({"data":map.get("base64_data").or_else(||map.get("base64")),"media_type":map.get("mime_type")}),
            true,
        )
    }
}
