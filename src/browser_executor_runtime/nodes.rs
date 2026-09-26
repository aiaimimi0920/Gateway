use super::normalization::{normalize_string, normalize_string_array, required_trimmed};
use super::storage::{list_members, read_views_by_ids, redis_connection_error, serialize_json};
use super::timestamps::now_rfc3339;
use super::{
    BrowserExecutorNodeHeartbeatInput, BrowserExecutorNodeStatus, BrowserExecutorNodeView,
    DEFAULT_NODE_TTL_SECONDS,
};
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
pub async fn heartbeat_browser_executor_node(
    state: &AppState,
    input: BrowserExecutorNodeHeartbeatInput,
) -> Result<BrowserExecutorNodeView, GatewayError> {
    let node_id = required_trimmed(&input.node_id, "nodeId")?;
    let view = BrowserExecutorNodeView {
        node_id: node_id.clone(),
        status: input.status.unwrap_or(BrowserExecutorNodeStatus::Active),
        capabilities: normalize_string_array(input.capabilities),
        last_heartbeat_at: now_rfc3339(),
        version: normalize_string(input.version),
        host: normalize_string(input.host),
    };
    let ttl_seconds = input
        .ttl_seconds
        .unwrap_or(DEFAULT_NODE_TTL_SECONDS)
        .max(30);
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let payload = serialize_json(&view, "browser executor node view")?;
    redis::pipe()
        .cmd("SADD")
        .arg(keys::browser_executor_nodes_key())
        .arg(&node_id)
        .ignore()
        .cmd("SET")
        .arg(keys::browser_executor_node_key(&node_id))
        .arg(payload)
        .arg("EX")
        .arg(ttl_seconds)
        .ignore()
        .query_async::<()>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write browser executor node: {error}"))
        })?;
    Ok(view)
}

pub async fn list_browser_executor_nodes(
    state: &AppState,
) -> Result<Vec<BrowserExecutorNodeView>, GatewayError> {
    let ids = list_members(state, keys::browser_executor_nodes_key()).await?;
    let mut views = read_views_by_ids::<BrowserExecutorNodeView>(
        state,
        keys::browser_executor_nodes_key(),
        &ids,
        keys::browser_executor_node_key,
    )
    .await?;
    views.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    Ok(views)
}
