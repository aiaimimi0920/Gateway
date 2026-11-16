//! Two fixed read-only model tools. No session creation, generic calls or inferred entitlement.
use super::{transport::Connection, McpError, VERSION};
use crate::protocol::luma_oauth::{TokenMaterial, RESOURCE};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};
use time::OffsetDateTime;

pub enum ModelQuery {
    List,
    Describe { model: String, kind: String },
}
impl ModelQuery {
    pub fn validate(&self) -> Result<(), McpError> {
        if let Self::Describe { model, kind } = self {
            if model.is_empty()
                || model.chars().count() > 128
                || model.chars().any(char::is_control)
                || !matches!(kind.as_str(), "image" | "video")
            {
                return Err(McpError::InvalidResponse);
            }
        }
        Ok(())
    }
    fn params(&self) -> Value {
        match self {
            Self::List => json!({"name":"luma_model_list","arguments":{}}),
            Self::Describe { model, kind } => {
                json!({"name":"luma_model_describe","arguments":{"model":model,"type":kind}})
            }
        }
    }
}

pub async fn read_models(
    http: &rquest::Client,
    material: &TokenMaterial,
    query: &ModelQuery,
) -> Result<Value, McpError> {
    read_at(
        http,
        RESOURCE,
        material.access_token(),
        material.expires_at(),
        query,
        Duration::from_secs(30),
    )
    .await
}

pub(super) async fn read_at(
    http: &rquest::Client,
    endpoint: &str,
    token: &str,
    expires_at: Option<OffsetDateTime>,
    query: &ModelQuery,
    budget: Duration,
) -> Result<Value, McpError> {
    query.validate()?;
    let deadline = tokio::time::Instant::now() + budget.min(Duration::from_secs(30));
    let mut connection = Connection {
        http,
        endpoint,
        token,
        expires_at,
        deadline,
        session: None,
        total: 0,
    };
    let result = tokio::time::timeout_at(deadline, async {
        let init = connection.post(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":VERSION,"capabilities":{},"clientInfo":{"name":"neuro-gateway","version":"0.1.0"}}}), Some(1), false)
            .await?.ok_or(McpError::InvalidResponse)?;
        if init["protocolVersion"] != VERSION || !init["capabilities"]["tools"].is_object() {
            return Err(McpError::Unsupported);
        }
        connection.post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}), None, true).await?;
        let result = connection.post(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":query.params()}), Some(2), true)
            .await?.ok_or(McpError::InvalidResponse)?;
        let payload = tool_payload(result)?;
        validate_result(query, &payload)?;
        Ok(payload)
    }).await.unwrap_or(Err(McpError::Timeout));
    let cleanup = connection.close().await;
    tracing::debug!(session_cleanup = ?cleanup, "Luma model read finished");
    result
}

fn tool_payload(result: Value) -> Result<Value, McpError> {
    match result.get("isError") {
        Some(Value::Bool(true)) => return Err(McpError::Rejected),
        None | Some(Value::Bool(false)) => {}
        _ => return Err(McpError::InvalidResponse),
    }
    // Structured results are authoritative; do not execute prose, fetch resources or join text blocks.
    if let Some(value) = result.get("structuredContent") {
        return value
            .is_object()
            .then(|| value.clone())
            .ok_or(McpError::InvalidResponse);
    }
    let content = result["content"]
        .as_array()
        .ok_or(McpError::InvalidResponse)?;
    if content.len() != 1 || content[0]["type"] != "text" {
        return Err(McpError::InvalidResponse);
    }
    serde_json::from_str(
        content[0]["text"]
            .as_str()
            .ok_or(McpError::InvalidResponse)?,
    )
    .map_err(|_| McpError::InvalidResponse)
}

fn validate_result(query: &ModelQuery, value: &Value) -> Result<(), McpError> {
    let valid = match query {
        ModelQuery::List => {
            let rows = value["models"]
                .as_array()
                .ok_or(McpError::InvalidResponse)?;
            if rows.len() > 256 {
                return Err(McpError::Limit);
            }
            let mut ids = HashSet::new();
            value["source"].as_str() == Some("registry")
                && rows.iter().all(|row| {
                    let id = row["id"].as_str().unwrap_or("");
                    !id.is_empty()
                        && id.chars().count() <= 128
                        && !id.chars().any(char::is_control)
                        && ids.insert(id)
                        && matches!(
                            row["modality"].as_str(),
                            Some("image" | "video" | "audio" | "edit" | "utility")
                        )
                        && row["aliases"]
                            .as_array()
                            .is_some_and(|a| a.iter().all(Value::is_string))
                        && row["actions"].is_array()
                        && row["capabilities"].is_object()
                        && (row["pricing"].is_null() || row["pricing"].is_object())
                        && (row["strengths"].is_null() || row["strengths"].is_string())
                })
        }
        ModelQuery::Describe { model, kind } => {
            value["model"] == model.as_str()
                && value["type"] == kind.as_str()
                && value["parameters"].is_object()
                && value["media_roles"]
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string))
                && (value["guidance"].is_null() || value["guidance"].is_string())
        }
    };
    if valid {
        Ok(())
    } else {
        Err(McpError::InvalidResponse)
    }
}
