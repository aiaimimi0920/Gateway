//! Typed options from the captured MCP contract; never forward arbitrary tool arguments.
use super::{image_parameters::ImageParameters, profiles};
use super::{reuse, Kind, McpError, Reuse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Options {
    pub model: Option<String>,
    pub operation: Option<String>,
    pub aspect_ratio: Option<String>,
    pub source_url: Option<String>,
    pub source_media_type: Option<String>,
    #[serde(default)]
    pub reference_urls: Vec<String>,
    pub style: Option<String>,
    pub output_format: Option<String>,
    pub parameters: Option<ImageParameters>,
    pub video: Option<super::video::Controls>,
}

impl Options {
    pub(crate) fn model(&self, kind: Kind) -> &str {
        self.model.as_deref().unwrap_or(kind.model())
    }
    pub(crate) fn operation(&self, kind: Kind) -> Result<&'static str, McpError> {
        match (kind, self.operation.as_deref().unwrap_or("generate")) {
            (Kind::Image, "generate") => Ok("image"),
            (Kind::Image, "edit") => Ok("image_edit"),
            (Kind::Video, "generate") => Ok("video"),
            (Kind::Video, "edit") => Ok("video_edit"),
            (Kind::Video, "reframe") => Ok("video_reframe"),
            _ => Err(McpError::InvalidResponse),
        }
    }
    pub(crate) fn pack(
        &self,
        kind: Kind,
        reuse: Option<&Reuse>,
    ) -> Result<Map<String, Value>, McpError> {
        let operation = self.operation(kind)?;
        let model = self.model(kind);
        let profile = profiles::get(model)?;
        if profile.kind != kind.name() {
            return Err(McpError::Unsupported);
        }
        profile.ratio(self.aspect_ratio.as_deref())?;
        let mut body = Map::new();
        body.insert("model".into(), json!(model));
        body.insert("type".into(), json!(operation));
        if kind == Kind::Image {
            self.image(&mut body, operation, model, reuse)?;
        } else {
            if self.style.is_some()
                || self.output_format.is_some()
                || self.parameters.is_some()
                || !self.reference_urls.is_empty()
                || reuse.is_some_and(|r| !r.reference_asset_ids.is_empty())
            {
                return Err(McpError::InvalidResponse);
            }
            if let Some(ratio) = &self.aspect_ratio {
                body.insert("aspect_ratio".into(), json!(ratio));
            }
            source(
                &mut body,
                &self.source_url,
                operation != "video",
                &self.source_media_type,
                reuse.and_then(|r| r.source_asset_id.as_deref()),
            )?;
            let video = self.video.clone().unwrap_or_default().pack(
                model,
                operation,
                self.aspect_ratio.as_deref(),
            )?;
            if !video.is_empty() {
                body.insert("video".into(), Value::Object(video));
            }
        }
        Ok(body)
    }
    fn image(
        &self,
        body: &mut Map<String, Value>,
        operation: &str,
        model: &str,
        reuse: Option<&Reuse>,
    ) -> Result<(), McpError> {
        if self.video.is_some() || self.source_media_type.is_some() {
            return Err(McpError::InvalidResponse);
        }
        let third_party = model != "luma_uni_1";
        let max_refs = profiles::get(model)?.max_references
            - usize::from(!third_party && operation == "image_edit");
        let mut references = reuse
            .map(Reuse::references)
            .transpose()?
            .unwrap_or_default();
        references.extend(
            self.reference_urls
                .iter()
                .map(|url| media(url))
                .collect::<Result<Vec<_>, _>>()?,
        );
        if references.len() > max_refs {
            return Err(McpError::Limit);
        }
        if let Some(ratio) = &self.aspect_ratio {
            body.insert("aspect_ratio".into(), json!(ratio));
        }
        if third_party {
            // Described third-party images accept reference-role medias, not native edit mode.
            if operation != "image"
                || self.source_url.is_some()
                || reuse.is_some_and(|r| r.source_asset_id.is_some())
                || self.style.is_some()
                || self.output_format.is_some()
            {
                return Err(McpError::InvalidResponse);
            }
            if let Some(params) = &self.parameters {
                body.insert("parameters".into(), params.pack(model, references.len())?);
            }
            let medias = references
                .into_iter()
                .map(|mut media| {
                    media["role"] = json!("reference");
                    media
                })
                .collect::<Vec<_>>();
            if !medias.is_empty() {
                body.insert("medias".into(), json!(medias));
            }
        } else {
            if self.parameters.is_some() || operation == "image_edit" && self.aspect_ratio.is_some()
            {
                return Err(McpError::InvalidResponse);
            }
            choice(body, "style", &self.style, &["auto", "manga"])?;
            choice(body, "output_format", &self.output_format, &["png", "jpeg"])?;
            source(
                body,
                &self.source_url,
                operation == "image_edit",
                &None,
                reuse.and_then(|r| r.source_asset_id.as_deref()),
            )?;
            if !references.is_empty() {
                body.insert("image_ref".into(), json!(references));
            }
        }
        Ok(())
    }
}
pub(super) fn choice(
    body: &mut Map<String, Value>,
    key: &str,
    value: &Option<String>,
    allowed: &[&str],
) -> Result<(), McpError> {
    if let Some(value) = value {
        if !allowed.contains(&value.as_str()) {
            return Err(McpError::InvalidResponse);
        }
        body.insert(key.into(), json!(value));
    }
    Ok(())
}
pub(super) fn media(url: &str) -> Result<Value, McpError> {
    if url.len() > 4096 {
        return Err(McpError::Limit);
    }
    let url = crate::http_client::public_asset_url::checked(url)
        .map_err(|_| McpError::InvalidResponse)?;
    // Input bytes are fetched by Luma. Gateway neither sends credentials nor downloads these URLs.
    Ok(json!({"url":url.as_str()}))
}
fn source(
    body: &mut Map<String, Value>,
    url: &Option<String>,
    required: bool,
    mime: &Option<String>,
    asset: Option<&str>,
) -> Result<(), McpError> {
    if url.is_some() && asset.is_some() || required != (url.is_some() || asset.is_some()) {
        return Err(McpError::InvalidResponse);
    }
    if let Some(id) = asset {
        if mime.is_some() {
            return Err(McpError::InvalidResponse);
        }
        body.insert("source".into(), reuse::media(id)?);
        return Ok(());
    }
    let video = body["type"] != "image_edit";
    if video
        && (mime.is_some() != required
            || mime
                .as_deref()
                .is_some_and(|v| !["video/mp4", "video/webm", "video/quicktime"].contains(&v)))
    {
        return Err(McpError::InvalidResponse);
    }
    if let Some(url) = url {
        let mut value = media(url)?;
        if let Some(mime) = mime {
            value["media_type"] = json!(mime);
        }
        body.insert("source".into(), value);
    }
    Ok(())
}
