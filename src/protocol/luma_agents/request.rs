//! Map the resolved model and explicit media operation; never forward arbitrary raw options.
use super::{invalid_request, layering, media_ref, video, video_edit, video_reframe};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind},
};
use serde_json::{json, Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Media {
    Image,
    Video,
}
pub(crate) struct GenerationRequest {
    pub body: Value,
    pub media: Media,
    pub model: String,
    pub kind: &'static str,
    pub base64: bool,
}
impl GenerationRequest {
    pub fn pack(request: &CanonicalRelayRequest, model: &str) -> Result<Self, GatewayError> {
        let raw = request.raw_body.as_object().ok_or_else(invalid_request)?;
        let (media, kind) = match request.endpoint_kind {
            EndpointKind::ImagesGenerations
                if raw.get("type").and_then(Value::as_str) == Some("layering") =>
            {
                (Media::Image, "layering")
            }
            EndpointKind::ImagesGenerations => (Media::Image, "image"),
            EndpointKind::ImagesEdits => (Media::Image, "image_edit"),
            EndpointKind::VideosGenerations => (
                Media::Video,
                match raw.get("type").and_then(Value::as_str) {
                    Some("video_reframe") => "video_reframe",
                    Some("video_edit") => "video_edit",
                    _ => "video",
                },
            ),
            _ => return Err(invalid_request()),
        };
        if request.stream
            || !request.tools.is_empty()
            || request.tool_choice.is_some()
            || request.reasoning.is_some()
            || !request.extra.is_empty()
            || !matches!(
                (media, model),
                (Media::Image, "uni-1" | "uni-1-max") | (Media::Video, "ray-3.2")
            )
        {
            return Err(invalid_request());
        }
        if kind == "layering" && model != "uni-1" {
            return Err(invalid_request());
        }
        const ALLOWED: &[&str] = &[
            "model",
            "prompt",
            "type",
            "n",
            "stream",
            "response_format",
            "aspect_ratio",
            "style",
            "output_format",
            "web_search",
            "image_ref",
            "source",
            "images",
            "video",
            "layering",
            "user_id",
        ];
        if raw
            .iter()
            .any(|(k, v)| !v.is_null() && !ALLOWED.contains(&k.as_str()))
            || raw
                .get("stream")
                .is_some_and(|v| !v.is_null() && v != &json!(false))
            || raw.get("type").is_some_and(|v| !v.is_null() && v != kind)
            || raw
                .get("n")
                .is_some_and(|v| !v.is_null() && v.as_u64() != Some(1))
            || kind != "layering" && raw.get("layering").is_some_and(|v| !v.is_null())
        {
            return Err(invalid_request());
        }
        serde_json::to_writer(RequestBytes(0), raw).map_err(|_| invalid_request())?;
        let prompt = raw
            .get("prompt")
            .and_then(Value::as_str)
            .filter(|s| {
                if kind == "layering" {
                    s.chars().count() <= 500
                } else {
                    !s.trim().is_empty() && s.chars().count() <= 6000
                }
            })
            .ok_or_else(invalid_request)?;
        let base64 = match raw.get("response_format").filter(|v| !v.is_null()) {
            None => false,
            Some(Value::String(s)) if s == "url" => false,
            Some(Value::String(s)) if s == "b64_json" => true,
            _ => return Err(invalid_request()),
        };
        let mut body = json!({"model":model,"type":kind,"prompt":prompt})
            .as_object()
            .unwrap()
            .clone();
        if let Some(user) = raw.get("user_id").filter(|v| !v.is_null()) {
            if !user
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.chars().count() <= 256)
            {
                return Err(invalid_request());
            }
            body.insert("user_id".into(), user.clone());
        }
        if kind == "layering" {
            layering::pack(raw, &mut body)?;
        } else if media == Media::Image {
            pack_image(raw, &mut body, kind == "image_edit")?;
        } else if kind == "video_reframe" {
            video_reframe::pack(raw, &mut body)?;
        } else if kind == "video_edit" {
            video_edit::pack(raw, &mut body)?;
        } else {
            video::pack(raw, &mut body)?;
        }
        Ok(Self {
            body: Value::Object(body),
            media,
            model: model.into(),
            kind,
            base64,
        })
    }
}
fn pack_image(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
    edit: bool,
) -> Result<(), GatewayError> {
    if raw.get("video").is_some_and(|v| !v.is_null()) {
        return Err(invalid_request());
    }
    choice(raw, body, "style", &["auto", "manga"])?;
    choice(raw, body, "output_format", &["png", "jpeg"])?;
    if edit && raw.get("aspect_ratio").is_some_and(|v| !v.is_null()) {
        // The API ignores this field on edits; reject an unfulfillable explicit control.
        return Err(invalid_request());
    }
    choice(
        raw,
        body,
        "aspect_ratio",
        &[
            "3:1", "2:1", "16:9", "3:2", "1:1", "2:3", "9:16", "1:2", "1:3",
        ],
    )?;
    if !edit
        && body.get("style") == Some(&json!("manga"))
        && body
            .get("aspect_ratio")
            .is_some_and(|v| !["2:3", "9:16", "1:2", "1:3"].contains(&v.as_str().unwrap()))
    {
        return Err(invalid_request());
    }
    boolean(raw, body, "web_search")?;
    if let Some(refs) = raw.get("image_ref").filter(|v| !v.is_null()) {
        let refs = refs
            .as_array()
            .filter(|v| v.len() <= if edit { 8 } else { 9 })
            .ok_or_else(invalid_request)?;
        let refs = refs
            .iter()
            .map(|r| media_ref::checked(r, false))
            .collect::<Result<Vec<_>, _>>()?;
        body.insert("image_ref".into(), json!(refs));
    }
    let source = raw.get("source").filter(|v| !v.is_null());
    let images = raw.get("images").filter(|v| !v.is_null());
    if !edit {
        if source.is_some() || images.is_some() {
            return Err(invalid_request());
        }
    } else {
        let source = match (source, images) {
            (Some(source), None) => media_ref::checked(source, true)?,
            (None, Some(images)) => {
                let images = images
                    .as_array()
                    .filter(|v| v.len() == 1)
                    .ok_or_else(invalid_request)?;
                media_ref::upload(&images[0])?
            }
            _ => return Err(invalid_request()),
        };
        body.insert("source".into(), source);
    }
    Ok(())
}
pub(super) fn choice(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
    key: &str,
    values: &[&str],
) -> Result<(), GatewayError> {
    if let Some(v) = raw.get(key).filter(|v| !v.is_null()) {
        if !v.as_str().is_some_and(|s| values.contains(&s)) {
            return Err(invalid_request());
        }
        body.insert(key.into(), v.clone());
    }
    Ok(())
}
pub(super) fn boolean(
    raw: &Map<String, Value>,
    body: &mut Map<String, Value>,
    key: &str,
) -> Result<(), GatewayError> {
    if let Some(v) = raw.get(key).filter(|v| !v.is_null()) {
        if !v.is_boolean() {
            return Err(invalid_request());
        }
        body.insert(key.into(), v.clone());
    }
    Ok(())
}

// Bound the aggregate serialized input before cloning references; no retained second JSON buffer.
struct RequestBytes(usize);
impl std::io::Write for RequestBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= 16 * 1024 * 1024)
            .ok_or_else(|| std::io::Error::other("Luma Agents local request limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
