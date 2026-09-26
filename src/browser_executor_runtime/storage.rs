use super::{BrowserCapabilityLeaseView, BrowserCapabilitySlotView};
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
pub(super) async fn get_browser_capability_slot(
    state: &AppState,
    slot_id: &str,
) -> Result<Option<BrowserCapabilitySlotView>, GatewayError> {
    read_view(state, &keys::browser_executor_slot_key(slot_id)).await
}

pub(super) async fn get_browser_capability_lease(
    state: &AppState,
    lease_id: &str,
) -> Result<Option<BrowserCapabilityLeaseView>, GatewayError> {
    read_view(state, &keys::browser_executor_lease_key(lease_id)).await
}

async fn read_view<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    key: &str,
) -> Result<Option<T>, GatewayError> {
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let raw: Option<String> = conn
        .get(key)
        .await
        .map_err(|error| GatewayError::server_error(format!("read redis key {key}: {error}")))?;
    raw.map(|value| deserialize_json(&value, key)).transpose()
}

pub(super) async fn list_members(
    state: &AppState,
    set_key: &str,
) -> Result<Vec<String>, GatewayError> {
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let members: Vec<String> = conn.smembers(set_key).await.map_err(|error| {
        GatewayError::server_error(format!("read redis set {set_key}: {error}"))
    })?;
    Ok(members
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect())
}

pub(super) async fn read_views_by_ids<T: for<'de> Deserialize<'de>>(
    state: &AppState,
    set_key: &str,
    ids: &[String],
    key_builder: impl Fn(&str) -> String,
) -> Result<Vec<T>, GatewayError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut conn = state
        .redis_pool
        .get()
        .await
        .map_err(redis_connection_error)?;
    let mut stale_ids = Vec::new();
    let mut views = Vec::new();
    for id in ids {
        let key = key_builder(id);
        let raw: Option<String> = conn.get(&key).await.map_err(|error| {
            GatewayError::server_error(format!("read redis key {key}: {error}"))
        })?;
        match raw {
            Some(value) => match deserialize_json(&value, &key) {
                Ok(view) => views.push(view),
                Err(_) => stale_ids.push(id.clone()),
            },
            None => stale_ids.push(id.clone()),
        }
    }
    if !stale_ids.is_empty() {
        let _: usize = conn.srem(set_key, stale_ids).await.map_err(|error| {
            GatewayError::server_error(format!(
                "cleanup stale redis set members {set_key}: {error}"
            ))
        })?;
    }
    Ok(views)
}

pub(super) fn serialize_json<T: Serialize>(value: &T, label: &str) -> Result<String, GatewayError> {
    serde_json::to_string(value)
        .map_err(|error| GatewayError::server_error(format!("serialize {label}: {error}")))
}

pub(super) fn deserialize_json<T: for<'de> Deserialize<'de>>(
    value: &str,
    label: &str,
) -> Result<T, GatewayError> {
    serde_json::from_str(value)
        .map_err(|error| GatewayError::server_error(format!("deserialize {label}: {error}")))
}

pub(super) fn redis_connection_error(error: deadpool_redis::PoolError) -> GatewayError {
    GatewayError::server_error(format!("get redis connection: {error}"))
}
