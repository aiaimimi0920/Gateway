//! Exactly initialize, initialized and paginated tools/list; server descriptions cannot add calls.
use super::{catalog::Catalog, transport::Connection, McpError, ToolDefinition, VERSION};
use crate::protocol::luma_oauth::{TokenMaterial, RESOURCE};
use serde_json::json;
use std::time::Duration;
use time::OffsetDateTime;

/// Explicit read-only inspection. Caller supplies a dedicated no-cookie, no-custom-retry OAuth client.
/// This does not open a Luma workspace, call a tool, or prove model availability/credits.
pub async fn inspect_tools(
    http: &rquest::Client,
    material: &TokenMaterial,
) -> Result<Vec<ToolDefinition>, McpError> {
    let endpoint = RESOURCE;
    #[cfg(test)]
    let endpoint = super::test_support::endpoint(endpoint);
    inspect_at(
        http,
        &endpoint,
        material.access_token(),
        material.expires_at(),
        Duration::from_secs(30),
    )
    .await
}

pub(super) async fn inspect_at(
    http: &rquest::Client,
    endpoint: &str,
    token: &str,
    expires_at: Option<OffsetDateTime>,
    budget: Duration,
) -> Result<Vec<ToolDefinition>, McpError> {
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
        let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":VERSION,"capabilities":{},
            "clientInfo":{"name":"neuro-gateway","version":"0.1.0"}}});
        let result = connection
            .post(initialize, Some(1), false)
            .await?
            .ok_or(McpError::InvalidResponse)?;
        if result["protocolVersion"] != VERSION || !result["capabilities"]["tools"].is_object() {
            return Err(McpError::Unsupported);
        }
        connection
            .post(
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                None,
                true,
            )
            .await?;
        let mut catalog = Catalog::default();
        let mut cursor = None;
        for id in 2..=9 {
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |cursor| json!({"cursor":cursor}));
            let result = connection
                .post(
                    json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}),
                    Some(id),
                    true,
                )
                .await?
                .ok_or(McpError::InvalidResponse)?;
            cursor = catalog.page(result)?;
            if cursor.is_none() {
                return Ok(catalog.tools);
            }
        }
        Err(McpError::Limit)
    })
    .await
    .unwrap_or(Err(McpError::Timeout));
    // Drop the catalog future/body before cleanup. Its outcome remains authoritative.
    // Cancellation drops both futures locally; never spawn an unowned token-bearing task.
    let cleanup = connection.close().await;
    tracing::debug!(session_cleanup = ?cleanup, "Luma MCP inspection finished");
    result
}
