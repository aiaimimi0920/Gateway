use deadpool_redis::Pool;

use crate::error::GatewayError;
use crate::provider_quota::GatewayProviderQuotaView;
use crate::redis::keys;

/// Read route candidates' cached quota snapshots with one Redis command.
/// Invalid entries are isolated because routing treats decode failures as
/// cache misses and then follows the existing guarded refresh path.
pub(super) async fn read_cached_quota_snapshots(
    redis_pool: &Pool,
    runtime_ids: &[(&str, Option<&str>)],
) -> Result<Vec<Option<GatewayProviderQuotaView>>, GatewayError> {
    if runtime_ids.is_empty() {
        return Ok(Vec::new());
    }

    let cache_keys = runtime_ids
        .iter()
        .map(|(account_id, credential_id)| quota_key(account_id, *credential_id))
        .collect::<Vec<_>>();
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let values = redis::cmd("MGET")
        .arg(&cache_keys)
        .query_async::<Vec<Option<String>>>(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read provider quota snapshots: {error}"))
        })?;
    Ok(values
        .into_iter()
        .map(|value| value.and_then(|raw| serde_json::from_str(&raw).ok()))
        .collect())
}

/// Read route candidates' breaker markers with one Redis command. Dependency
/// failures remain fail-open, matching the existing single-candidate contract.
pub(super) async fn read_breakers_open(
    redis_pool: &Pool,
    runtime_ids: &[(&str, Option<&str>)],
) -> Vec<bool> {
    if runtime_ids.is_empty() {
        return Vec::new();
    }

    let cache_keys = runtime_ids
        .iter()
        .map(|(account_id, credential_id)| breaker_key(account_id, *credential_id))
        .collect::<Vec<_>>();
    let Ok(mut conn) = redis_pool.get().await else {
        return vec![false; runtime_ids.len()];
    };
    match redis::cmd("MGET")
        .arg(&cache_keys)
        .query_async::<Vec<Option<String>>>(&mut conn)
        .await
    {
        Ok(values) if values.len() == runtime_ids.len() => {
            values.into_iter().map(|value| value.is_some()).collect()
        }
        _ => vec![false; runtime_ids.len()],
    }
}

fn quota_key(account_id: &str, credential_id: Option<&str>) -> String {
    credential_id
        .map(keys::provider_credential_quota_snapshot_key)
        .unwrap_or_else(|| keys::provider_quota_snapshot_key(account_id))
}

fn breaker_key(account_id: &str, credential_id: Option<&str>) -> String {
    credential_id
        .map(keys::provider_credential_breaker_open_key)
        .unwrap_or_else(|| keys::provider_breaker_open_key(account_id))
}

#[cfg(test)]
#[path = "stage_route_health_redis_tests.rs"]
mod redis_tests;
