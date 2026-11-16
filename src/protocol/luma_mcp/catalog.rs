//! Tool descriptions and schemas are data, not execution approval or verified business capability.
use super::McpError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
}

#[derive(Default)]
pub(super) struct Catalog {
    pub tools: Vec<ToolDefinition>,
    names: HashSet<String>,
    cursors: HashSet<String>,
}
impl Catalog {
    pub(super) fn page(&mut self, mut value: Value) -> Result<Option<String>, McpError> {
        let values = value
            .get_mut("tools")
            .and_then(Value::as_array_mut)
            .ok_or(McpError::InvalidResponse)?;
        if self.tools.len().saturating_add(values.len()) > 256 {
            return Err(McpError::Limit);
        }
        for value in values.drain(..) {
            let tool: ToolDefinition =
                serde_json::from_value(value).map_err(|_| McpError::InvalidResponse)?;
            if tool.name.is_empty()
                || tool.name.len() > 128
                || !tool
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                || !self.names.insert(tool.name.clone())
                || tool.title.as_ref().is_some_and(|v| v.len() > 1024)
                || tool.description.as_ref().is_some_and(|v| v.len() > 16_384)
                || !tool.input_schema.is_object()
                || tool.input_schema["type"] != "object"
                || tool.output_schema.as_ref().is_some_and(|v| !v.is_object())
            {
                return Err(McpError::InvalidResponse);
            }
            self.tools.push(tool);
        }
        match value.get("nextCursor") {
            None => Ok(None),
            Some(Value::String(cursor)) if !cursor.is_empty() && cursor.len() <= 4096 => {
                if !self.cursors.insert(cursor.clone()) {
                    return Err(McpError::InvalidResponse);
                }
                Ok(Some(cursor.clone()))
            }
            _ => Err(McpError::InvalidResponse),
        }
    }
}
