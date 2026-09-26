use super::*;
use crate::redis::keys;
use redis::AsyncCommands;

const ACCESS_AFFINITY_TTL_SECS: u64 = 3600;

fn sticky_affinity_scope(explicit_session_key: Option<&str>) -> String {
    explicit_session_key
        .map(|value| format!("session:{value}"))
        .unwrap_or_else(|| "key".to_string())
}

fn sticky_affinity_key(
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> String {
    keys::access_affinity_key(
        &sticky_affinity_scope(explicit_session_key),
        requesting_access_key_id,
        model,
    )
}

pub async fn inspect_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<Option<GatewayAccessStickyAffinityView>, GatewayError> {
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(&key)
        .await
        .map_err(|error| GatewayError::server_error(format!("read sticky affinity: {error}")))?;
    raw.map(|value| {
        serde_json::from_str::<GatewayAccessStickyAffinityView>(&value).map_err(|error| {
            GatewayError::server_error(format!("deserialize sticky affinity: {error}"))
        })
    })
    .transpose()
}

pub async fn reset_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<(), GatewayError> {
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .del(key)
        .await
        .map_err(|error| GatewayError::server_error(format!("delete sticky affinity: {error}")))?;
    Ok(())
}

pub async fn record_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    source_access_key_id: &str,
    platform_access_id: &str,
    provider_account_id: &str,
    real_credential_ref: Option<&str>,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<(), GatewayError> {
    let view = GatewayAccessStickyAffinityView {
        scope: sticky_affinity_scope(explicit_session_key),
        model: model.to_string(),
        requesting_access_key_id: requesting_access_key_id.to_string(),
        source_access_key_id: source_access_key_id.to_string(),
        platform_access_id: platform_access_id.to_string(),
        provider_account_id: provider_account_id.to_string(),
        real_credential_ref: real_credential_ref.map(str::to_string),
        expires_at: Some(format_timestamp(now_utc() + time::Duration::hours(1))),
    };
    let payload = serde_json::to_string(&view).map_err(|error| {
        GatewayError::server_error(format!("serialize sticky affinity: {error}"))
    })?;
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg(payload)
        .arg("EX")
        .arg(ACCESS_AFFINITY_TTL_SECS)
        .query_async(&mut conn)
        .await
        .map_err(|error| GatewayError::server_error(format!("write sticky affinity: {error}")))?;
    Ok(())
}
