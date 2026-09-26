//! User-credential cache payloads, expiration and deletion.

use crate::db;
use crate::error::GatewayError;
use crate::redis::keys;
use crate::state::AppState;
use redis::AsyncCommands;
use serde_json::Value;

pub(super) async fn set_cached_user_credential(
    state: &AppState,
    issued: &db::IssuedUserCredential,
) -> Result<(), GatewayError> {
    let payload = serde_json::json!({
        "id": issued.id,
        "userId": issued.user_id,
        "projectId": issued.project_id,
        "scope": issued.scope,
        "expiresAt": issued.expires_at,
        "status": "active",
        "tenantId": issued.tenant_id,
    });
    write_user_credential_cache(
        &state.redis_pool,
        &issued.credential_key,
        &payload,
        &issued.expires_at,
    )
    .await
}

pub(super) async fn set_cached_user_credential_from_cache_entry(
    state: &AppState,
    credential_key: &str,
    credential: &db::GatewayUserCredentialCacheEntry,
) -> Result<(), GatewayError> {
    let payload = serde_json::json!({
        "id": credential.id,
        "userId": credential.user_id,
        "projectId": credential.project_id,
        "scope": credential.scope,
        "expiresAt": credential.expires_at,
        "status": credential.status,
        "tenantId": credential.tenant_id,
    });
    write_user_credential_cache(
        &state.redis_pool,
        credential_key,
        &payload,
        &credential.expires_at,
    )
    .await
}

async fn write_user_credential_cache(
    redis_pool: &deadpool_redis::Pool,
    credential_key: &str,
    payload: &Value,
    expires_at: &str,
) -> Result<(), GatewayError> {
    let ttl = compute_cache_ttl_secs(expires_at)
        .unwrap_or(300)
        .clamp(60, 300);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let serialized = serde_json::to_string(payload).map_err(|error| {
        GatewayError::server_error(format!("serialize user credential cache: {error}"))
    })?;
    conn.set_ex::<_, _, ()>(keys::user_credential_key(credential_key), serialized, ttl)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write user credential cache: {error}"))
        })?;
    Ok(())
}

pub(super) async fn delete_cached_user_credential(
    state: &AppState,
    credential_key: &str,
) -> Result<(), GatewayError> {
    let mut conn =
        state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
    conn.del::<_, ()>(keys::user_credential_key(credential_key))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("delete user credential cache: {error}"))
        })?;
    Ok(())
}

fn compute_cache_ttl_secs(expires_at: &str) -> Option<u64> {
    let parsed =
        time::OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            .ok()?;
    if parsed <= time::OffsetDateTime::now_utc() {
        return None;
    }
    Some(
        (parsed - time::OffsetDateTime::now_utc())
            .whole_seconds()
            .max(1) as u64,
    )
}
