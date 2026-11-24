//! Reviewed model snapshot shared with the form; live discovery never changes write admission.
use super::McpError;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Profile {
    pub kind: String,
    pub ratios: Vec<String>,
    pub max_references: usize,
    pub parameters: Map<String, Value>,
}
pub(super) fn get(model: &str) -> Result<&'static Profile, McpError> {
    static PROFILES: OnceLock<HashMap<String, Profile>> = OnceLock::new();
    PROFILES
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../../../../manifests/luma-mcp-creation-models.json"
            ))
            .expect("reviewed Luma model profiles")
        })
        .get(model)
        .ok_or(McpError::Unsupported)
}
impl Profile {
    pub(super) fn ratio(&self, value: Option<&str>) -> Result<(), McpError> {
        if value.is_some_and(|v| !self.ratios.iter().any(|ratio| ratio == v)) {
            return Err(McpError::InvalidResponse);
        }
        Ok(())
    }
    pub(super) fn parameters(&self, values: &Map<String, Value>) -> Result<(), McpError> {
        for (key, value) in values {
            let schema = self.parameters.get(key).ok_or(McpError::InvalidResponse)?;
            let valid = match schema["type"].as_str() {
                Some("string") => value.as_str().is_some_and(|s| s.len() <= 32),
                Some("boolean") => value.is_boolean(),
                Some("number") => value.as_f64().is_some_and(|n| {
                    n.is_finite()
                        && schema["minimum"].as_f64().is_none_or(|min| n >= min)
                        && schema["maximum"].as_f64().is_none_or(|max| n <= max)
                }),
                _ => false,
            };
            if !valid
                || schema["enum"]
                    .as_array()
                    .is_some_and(|choices| !choices.contains(value))
            {
                return Err(McpError::InvalidResponse);
            }
        }
        Ok(())
    }
}
