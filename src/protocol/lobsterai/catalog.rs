//! Dynamic catalog facts only: no invented tool/reasoning support, prices or token limits.
use super::LoginError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// Saved declarations are not successful inference receipts. Validate again at config admission.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalog {
    models: Vec<DeclaredModel>,
}
impl ModelCatalog {
    pub fn new(models: Vec<DeclaredModel>) -> Result<Self, LoginError> {
        let catalog = Self { models };
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn models(&self) -> &[DeclaredModel] {
        &self.models
    }
    pub fn validate(&self) -> Result<(), LoginError> {
        let parsed = parse(serde_json::json!({"code":0,"data":self.models}))?;
        if parsed != self.models {
            return Err(LoginError::InvalidResponse);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredModel {
    pub model_id: String,
    pub model_name: Option<String>,
    pub api_format: Option<String>,
    pub runtime_profile: Option<String>,
    pub supports_tool_calling: Option<bool>,
    pub supports_thinking: Option<bool>,
    pub supports_image: Option<bool>,
    pub supports_video: Option<bool>,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    pub request_capabilities: Option<Vec<String>>,
    pub accessible: Option<bool>,
}
pub(super) fn parse(value: Value) -> Result<Vec<DeclaredModel>, LoginError> {
    if value["code"].as_i64() != Some(0) {
        return Err(LoginError::InvalidResponse);
    }
    let rows = value["data"]
        .as_array()
        .ok_or(LoginError::InvalidResponse)?;
    if rows.len() > 2048 {
        return Err(LoginError::ResponseTooLarge);
    }
    let mut ids = HashSet::new();
    let mut models = Vec::new();
    for row in rows {
        let model: DeclaredModel =
            serde_json::from_value(row.clone()).map_err(|_| LoginError::InvalidResponse)?;
        if !text(&model.model_id, 512)
            || !ids.insert(model.model_id.clone())
            || [&model.model_name, &model.api_format, &model.runtime_profile]
                .into_iter()
                .flatten()
                .any(|s| !text(s, 512))
            || [model.context_window, model.max_tokens]
                .into_iter()
                .flatten()
                .any(|n| n == 0)
            || model
                .request_capabilities
                .as_ref()
                .is_some_and(|c| c.len() > 64 || c.iter().any(|s| !text(s, 128)))
        {
            return Err(LoginError::InvalidResponse);
        }
        if model.accessible != Some(false) {
            models.push(model);
        }
    }
    Ok(models)
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max
        && !value.chars().any(char::is_control)
}
