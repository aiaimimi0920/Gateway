//! Ray controls. Unsupported exports and model switches never become ordinary outputs.
use super::{
    options::{choice, media},
    McpError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Controls {
    pub resolution: Option<String>,
    pub duration: Option<String>,
    pub hdr: Option<bool>,
    pub start_frame_url: Option<String>,
    pub end_frame_url: Option<String>,
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
    pub edit: Option<Edit>,
    pub source_position: Option<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_video: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Keyframe {
    pub url: String,
    pub index: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Edit {
    pub strength: Option<String>,
    pub auto_controls: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Position {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Controls {
    pub(crate) fn pack(
        &self,
        model: &str,
        operation: &str,
        ratio: Option<&str>,
    ) -> Result<Map<String, Value>, McpError> {
        let mut body = Map::new();
        choice(
            &mut body,
            "resolution",
            &self.resolution,
            &["360p", "540p", "720p", "1080p"],
        )?;
        choice(&mut body, "duration", &self.duration, &["5s", "10s"])?;
        let anchors = self.start_frame_url.is_some()
            || self.end_frame_url.is_some()
            || !self.keyframes.is_empty();
        if model != "luma_ray3_2" && !self.keyframes.is_empty()
            || self.loop_video.is_some() && (model != "luma_ray3_14" || operation != "video")
            || model == "luma_ray3_14_hdr"
                && (operation != "video"
                    || self.hdr == Some(false)
                    || self.duration.as_deref() == Some("10s")
                    || matches!(self.resolution.as_deref(), Some("360p" | "540p")))
            || model == "luma_ray3_14"
                && operation == "video"
                && (self.hdr == Some(true)
                    || self.duration.as_deref() == Some("10s")
                        && (anchors || self.loop_video == Some(true)))
        {
            return Err(McpError::InvalidResponse);
        }
        if let Some(value) = self.loop_video {
            body.insert("loop".into(), json!(value));
        }
        if model == "luma_ray3_14_hdr" {
            body.insert("hdr".into(), json!(true));
        }
        if self.hdr == Some(true) && matches!(self.resolution.as_deref(), Some("360p" | "540p")) {
            return Err(McpError::InvalidResponse);
        }
        if let Some(hdr) = self.hdr {
            body.insert("hdr".into(), json!(hdr));
        }
        match operation {
            "video" => {
                if self.edit.is_some() || self.source_position.is_some() {
                    return Err(McpError::InvalidResponse);
                }
                if !self.keyframes.is_empty()
                    && (self.start_frame_url.is_some() || self.end_frame_url.is_some())
                {
                    return Err(McpError::InvalidResponse);
                }
                self.frames(&mut body)?;
            }
            "video_edit" => {
                if ratio.is_some()
                    || self.duration.is_some()
                    || self.source_position.is_some()
                    || self.end_frame_url.is_some()
                    || !self.keyframes.is_empty()
                {
                    return Err(McpError::InvalidResponse);
                }
                self.frames(&mut body)?;
                if let Some(edit) = &self.edit {
                    let mut values = Map::new();
                    choice(
                        &mut values,
                        "strength",
                        &edit.strength,
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
                    if let Some(auto) = edit.auto_controls {
                        values.insert("auto_controls".into(), json!(auto));
                    }
                    body.insert("edit".into(), Value::Object(values));
                }
            }
            "video_reframe" => {
                if ratio.is_none()
                    || anchors
                    || self.edit.is_some()
                    || self.hdr.is_some()
                    || self.resolution.as_deref() == Some("1080p")
                        && matches!(ratio, Some("9:16" | "3:4"))
                {
                    return Err(McpError::InvalidResponse);
                }
                if let Some(p) = &self.source_position {
                    if ![p.x, p.y, p.width, p.height].iter().all(|v| v.is_finite())
                        || !(-2.0..=2.0).contains(&p.x)
                        || !(-2.0..=2.0).contains(&p.y)
                        || !(0.0 < p.width && p.width <= 2.0 && 0.0 < p.height && p.height <= 2.0)
                    {
                        return Err(McpError::InvalidResponse);
                    }
                    body.insert(
                        "source_position".into(),
                        json!({"x_norm":p.x,"y_norm":p.y,"w_norm":p.width,"h_norm":p.height}),
                    );
                }
            }
            _ => return Err(McpError::InvalidResponse),
        }
        Ok(body)
    }
    fn frames(&self, body: &mut Map<String, Value>) -> Result<(), McpError> {
        for (key, url) in [
            ("start_frame", &self.start_frame_url),
            ("end_frame", &self.end_frame_url),
        ] {
            if let Some(url) = url {
                body.insert(key.into(), media(url)?);
            }
        }
        if self.keyframes.len() > 64 {
            return Err(McpError::Limit);
        }
        if !self.keyframes.is_empty() {
            let last = if self.duration.as_deref() == Some("10s") {
                240
            } else {
                120
            };
            let mut indexes = std::collections::HashSet::new();
            let mut frames = Vec::new();
            for frame in &self.keyframes {
                if frame.index > last || !indexes.insert(frame.index) {
                    return Err(McpError::InvalidResponse);
                }
                frames.push(media(&frame.url)?);
            }
            body.insert("keyframes".into(), json!(frames));
            body.insert(
                "keyframe_indexes".into(),
                json!(self.keyframes.iter().map(|f| f.index).collect::<Vec<_>>()),
            );
        }
        Ok(())
    }
}
