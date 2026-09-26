//! Immutable Redis JSON storage with compare-and-set normalization.

use super::types::RouteConfigRedisStoreError;
use redis::AsyncCommands;

const NORMALIZE_IMMUTABLE_JSON_LUA: &str = r#"
local current = redis.call('GET', KEYS[1])
if current == ARGV[1] then
  redis.call('SET', KEYS[1], ARGV[2])
  return 1
end
if current == ARGV[2] then
  return 1
end
return 0
"#;

pub(super) async fn store_json_if_absent_or_identical(
    connection: &mut deadpool_redis::Connection,
    key: &str,
    value: &str,
) -> Result<(), RouteConfigRedisStoreError> {
    let existing: Option<String> = connection.get(key).await.map_err(|error| {
        RouteConfigRedisStoreError::unavailable(format!(
            "Gateway console Redis read failed for '{key}': {error}"
        ))
    })?;
    if let Some(existing) = existing {
        return accept_or_normalize_existing_json(connection, key, &existing, value).await;
    }
    let result: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .arg("NX")
        .query_async(connection)
        .await
        .map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis write failed for '{key}': {error}"
            ))
        })?;
    if result.as_deref() != Some("OK") {
        let existing: Option<String> = connection.get(key).await.map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis re-read failed for '{key}': {error}"
            ))
        })?;
        let Some(existing) = existing else {
            return Err(RouteConfigRedisStoreError::invalid_state(format!(
                "Gateway console Redis key '{key}' disappeared before the immutable payload could be stored",
            )));
        };
        return accept_or_normalize_existing_json(connection, key, &existing, value).await;
    }
    Ok(())
}

async fn accept_or_normalize_existing_json(
    connection: &mut deadpool_redis::Connection,
    key: &str,
    existing: &str,
    value: &str,
) -> Result<(), RouteConfigRedisStoreError> {
    if existing == value {
        return Ok(());
    }
    if !json_payloads_equivalent(existing, value) {
        return Err(RouteConfigRedisStoreError::invalid_state(format!(
            "Gateway console Redis key '{key}' already contains a different immutable value",
        )));
    }

    let normalized = redis::Script::new(NORMALIZE_IMMUTABLE_JSON_LUA)
        .key(key)
        .arg(existing)
        .arg(value)
        .invoke_async::<i64>(connection)
        .await
        .map_err(|error| {
            RouteConfigRedisStoreError::unavailable(format!(
                "Gateway console Redis immutable JSON normalization failed for '{key}': {error}"
            ))
        })?;
    if normalized == 1 {
        return Ok(());
    }

    let latest: Option<String> = connection.get(key).await.map_err(|error| {
        RouteConfigRedisStoreError::unavailable(format!(
            "Gateway console Redis re-read failed for '{key}': {error}"
        ))
    })?;
    if latest.as_deref() == Some(value) {
        return Ok(());
    }
    Err(RouteConfigRedisStoreError::invalid_state(format!(
        "Gateway console Redis key '{key}' changed while an equivalent immutable JSON payload was normalized",
    )))
}

pub(super) fn json_payloads_equivalent(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    match (
        serde_json::from_str::<serde_json::Value>(left),
        serde_json::from_str::<serde_json::Value>(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
