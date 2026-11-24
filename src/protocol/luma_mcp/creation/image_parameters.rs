//! Model-specific image options and combinations that the upstream would otherwise ignore.
use super::{profiles, McpError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageParameters {
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub output_format: Option<String>,
    pub background: Option<String>,
    // Omit new empty fields to retain hashes of previously prepared requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_search: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magic_prompt: Option<String>,
}
impl ImageParameters {
    pub(super) fn pack(&self, model: &str, references: usize) -> Result<Value, McpError> {
        let mut values = Map::new();
        for (key, value) in [
            ("resolution", &self.resolution),
            ("quality", &self.quality),
            ("output_format", &self.output_format),
            ("background", &self.background),
            ("size", &self.size),
            ("thinking_level", &self.thinking_level),
            ("magic_prompt", &self.magic_prompt),
        ] {
            if let Some(value) = value {
                values.insert(key.into(), json!(value));
            }
        }
        if let Some(value) = self.enable_search {
            values.insert("enable_search".into(), json!(value));
        }
        if let Some(value) = self.temperature {
            if !value.is_finite() {
                return Err(McpError::InvalidResponse);
            }
            values.insert("temperature".into(), json!(value));
        }
        profiles::get(model)?.parameters(&values)?;
        if self.background.as_deref() == Some("transparent")
            && self.output_format.as_deref() == Some("jpeg")
            || references > 0 && self.magic_prompt.is_some()
            || model == "ideogram_4_6"
                && self.quality.as_deref() == Some("very_low")
                && self
                    .size
                    .as_deref()
                    .is_some_and(|s| s.starts_with("4096") || s.ends_with("4096"))
            || model == "bytedance_seedream_5_flash"
                && self.size.as_deref().is_some_and(|s| !valid_size(s))
        {
            return Err(McpError::InvalidResponse);
        }
        Ok(Value::Object(values))
    }
}
fn valid_size(size: &str) -> bool {
    if ["1K", "1.5K", "2K"].contains(&size) {
        return true;
    }
    let Some((width, height)) = size.split_once('x') else {
        return false;
    };
    if ![width, height]
        .iter()
        .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(width), Ok(height)) = (width.parse::<u64>(), height.parse::<u64>()) else {
        return false;
    };
    width
        .checked_mul(height)
        .is_some_and(|pixels| (921_600..=4_624_220).contains(&pixels))
        && width <= height.saturating_mul(16)
        && height <= width.saturating_mul(16)
}
