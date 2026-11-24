//! Validate a complete front-to-back layer manifest before downloading any of its assets.
use super::{assets, layering_text::Text};
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::failure,
    upstream::public_assets,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Layer {
    index: usize,
    label: String,
    description: String,
    alpha_hint: Alpha,
    bounds: Option<Bounds>,
    contains_text: Option<bool>,
    text: Option<Text>,
}
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Alpha {
    Soft,
    Hard,
    None,
}
#[derive(Clone, Deserialize, Serialize)]
struct Bounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    canvas_width: u32,
    canvas_height: u32,
}
impl Bounds {
    fn valid(&self) -> bool {
        (1..=8192).contains(&self.canvas_width)
            && (1..=8192).contains(&self.canvas_height)
            && self.width > 0
            && self.height > 0
            && self
                .x
                .checked_add(self.width)
                .is_some_and(|n| n <= self.canvas_width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|n| n <= self.canvas_height)
    }
}

pub(super) fn stack(outputs: &[Value]) -> Result<Vec<Layer>, GatewayError> {
    if outputs.is_empty() || outputs.len() > 10 {
        return Err(invalid());
    }
    let mut canvas = None;
    outputs
        .iter()
        .enumerate()
        .map(|(index, output)| {
            if output.get("type").and_then(Value::as_str) != Some("image") {
                return Err(invalid());
            }
            let layer: Layer =
                serde_json::from_value(output.get("layer").ok_or_else(invalid)?.clone())
                    .map_err(|_| invalid())?;
            if layer.index != index
                || (layer.alpha_hint == Alpha::None) != (index + 1 == outputs.len())
                || layer
                    .text
                    .as_ref()
                    .is_some_and(|text| layer.contains_text != Some(true) || !text.valid())
            {
                return Err(invalid());
            }
            if let Some(bounds) = &layer.bounds {
                let size = (bounds.canvas_width, bounds.canvas_height);
                let partial_background = layer.alpha_hint == Alpha::None
                    && (bounds.x != 0
                        || bounds.y != 0
                        || bounds.width != bounds.canvas_width
                        || bounds.height != bounds.canvas_height);
                if !bounds.valid()
                    || canvas.is_some_and(|prior| prior != size)
                    || partial_background
                {
                    return Err(invalid());
                }
                canvas = Some(size);
            }
            Ok(layer)
        })
        .collect()
}

pub(super) async fn download(
    url: &str,
    layer: &Layer,
    deadline: tokio::time::Instant,
    limit: usize,
) -> Result<(Vec<u8>, &'static str), GatewayError> {
    let (bytes, mime) = public_assets::download(url, deadline, limit)
        .await
        .map_err(assets::error)?;
    let layer = layer.clone();
    let (bytes, mime, ()) =
        public_assets::image::validate_with(bytes, &mime, move |image| {
            if layer.bounds.as_ref().is_some_and(|bounds| {
                bounds.width != image.width() || bounds.height != image.height()
            }) {
                return Err(public_assets::Failure::Type);
            }
            let opaque = layer.alpha_hint == Alpha::None;
            let valid = match image {
                image::DynamicImage::ImageRgba8(rgba) => {
                    !opaque || rgba.pixels().all(|p| p.0[3] == u8::MAX)
                }
                image::DynamicImage::ImageRgba16(rgba) => {
                    !opaque || rgba.pixels().all(|p| p.0[3] == u16::MAX)
                }
                _ => false,
            };
            if valid {
                Ok(())
            } else {
                Err(public_assets::Failure::Type)
            }
        })
        .await
        .map_err(|_| invalid())?;
    if mime != "image/png" {
        return Err(invalid());
    }
    Ok((bytes, mime))
}

pub(super) fn invalid() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_agents_layers",
        "Luma Agents 图层顺序、元数据或 PNG 资产不完整；未返回部分成功或重提生成。",
    )
}
