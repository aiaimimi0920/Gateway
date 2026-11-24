//! Each operation owns a bounded MCP connection; cleanup is not business cancellation.
use super::super::{models::tool_payload, transport::Connection, McpError, VERSION};
use crate::protocol::luma_oauth::{TokenMaterial, RESOURCE};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Copy)]
pub(crate) enum Tool {
    Open,
    Image,
    Video,
    Await,
    View,
    Usage,
    Cancel,
    Respond,
    SessionAwait,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Open => "luma_session_open",
            Self::Image => "luma_generate_image",
            Self::Video => "luma_generate_video",
            Self::Await => "luma_run_await",
            Self::View => "luma_workspace_view_asset",
            Self::Usage => "luma_usage",
            Self::Cancel => "luma_session_cancel",
            Self::Respond => "luma_run_respond",
            Self::SessionAwait => "luma_session_await",
        }
    }
}
pub(crate) fn endpoint() -> String {
    #[cfg(test)]
    {
        super::super::test_support::endpoint(RESOURCE)
    }
    #[cfg(not(test))]
    {
        RESOURCE.to_owned()
    }
}
pub(crate) async fn call(
    http: &rquest::Client,
    endpoint: &str,
    material: &TokenMaterial,
    tool: Tool,
    arguments: Value,
) -> Result<Value, McpError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(35);
    let mut connection = Connection {
        http,
        endpoint,
        token: material.access_token(),
        expires_at: material.expires_at(),
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
        let value = connection.post(json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":tool.name(),"arguments":arguments}}), Some(2), true)
            .await?.ok_or(McpError::InvalidResponse)?;
        tool_payload(value)
    }).await.unwrap_or(Err(McpError::Timeout));
    let cleanup = connection.close().await;
    tracing::debug!(session_cleanup = ?cleanup, "Luma creation operation transport closed");
    result
}
