use super::GatewayProviderQuotaView;
use crate::error::GatewayError;
use crate::redis::keys;
use deadpool_redis::Pool;
use redis::AsyncCommands;
const PROVIDER_QUOTA_CACHE_TTL_SECONDS: u64 = 86_400;

pub async fn read_cached_provider_quota_snapshot(
    redis_pool: &Pool,
    provider_account_id: &str,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    read_cached_runtime_quota_snapshot(redis_pool, provider_account_id, None).await
}

pub async fn read_cached_runtime_quota_snapshot(
    redis_pool: &Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(quota_snapshot_key(
            provider_account_id,
            provider_credential_id,
        ))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read provider quota snapshot: {error}"))
        })?;
    match raw {
        Some(raw) => serde_json::from_str::<GatewayProviderQuotaView>(&raw)
            .map(Some)
            .map_err(|error| {
                GatewayError::server_error(format!("decode provider quota snapshot: {error}"))
            }),
        None => Ok(None),
    }
}

pub(super) async fn store_cached_provider_quota_snapshot(
    redis_pool: &Pool,
    snapshot: &GatewayProviderQuotaView,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let value = serde_json::to_string(snapshot).map_err(|error| {
        GatewayError::server_error(format!("encode provider quota snapshot: {error}"))
    })?;
    let _: () = conn
        .set_ex(
            quota_snapshot_key(
                &snapshot.provider_account_id,
                snapshot.provider_credential_id.as_deref(),
            ),
            value,
            PROVIDER_QUOTA_CACHE_TTL_SECONDS,
        )
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("store provider quota snapshot: {error}"))
        })?;
    Ok(())
}

fn quota_snapshot_key(provider_account_id: &str, provider_credential_id: Option<&str>) -> String {
    provider_credential_id
        .map(keys::provider_credential_quota_snapshot_key)
        .unwrap_or_else(|| keys::provider_quota_snapshot_key(provider_account_id))
}
