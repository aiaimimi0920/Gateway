//! Only assets proved by a saved task can be selected; the management journal binds the workspace.
use super::{reference, Kind, McpError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Reuse {
    pub task_id: String,
    pub source_asset_id: Option<String>,
    #[serde(default)]
    pub reference_asset_ids: Vec<String>,
}
impl Reuse {
    pub(crate) fn assets(&self, kind: Kind) -> Result<Vec<(String, Kind)>, McpError> {
        if !self.task_id.starts_with("luma-mcp-")
            || uuid::Uuid::parse_str(&self.task_id[9..]).is_err()
            || self.reference_asset_ids.len() > 16
            || self.source_asset_id.is_none() && self.reference_asset_ids.is_empty()
        {
            return Err(McpError::InvalidResponse);
        }
        let mut assets = Vec::new();
        if let Some(id) = &self.source_asset_id {
            assets.push((asset_id(id)?, kind));
        }
        for id in &self.reference_asset_ids {
            let id = asset_id(id)?;
            if assets.iter().any(|(other, _)| other == &id) {
                return Err(McpError::InvalidResponse);
            }
            assets.push((id, Kind::Image));
        }
        Ok(assets)
    }
    pub(super) fn references(&self) -> Result<Vec<Value>, McpError> {
        self.reference_asset_ids
            .iter()
            .map(|id| media(id))
            .collect()
    }
}
pub(super) fn media(id: &str) -> Result<Value, McpError> {
    Ok(json!({"asset_id":asset_id(id)?}))
}
fn asset_id(id: &str) -> Result<String, McpError> {
    if id.len() > 64 {
        return Err(McpError::InvalidResponse);
    }
    reference(&json!(id), "artifact")
}
