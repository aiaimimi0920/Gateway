use super::accio::fetch_accio_quota_snapshot;
use super::cache::{read_cached_runtime_quota_snapshot, store_cached_provider_quota_snapshot};
use super::codex::fetch_codex_quota_snapshot;
use super::generic_balance::fetch_generic_balance_snapshot;
use super::refresh_clock::quota_refresh_due;
use super::refresh_lock::{acquire_provider_quota_lock, release_provider_quota_lock};
use super::{provider_supports_quota, GatewayProviderQuotaView};
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use deadpool_redis::Pool;
pub async fn refresh_provider_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    refresh_runtime_quota_snapshot(redis_pool, timeout_secs, provider_account_id, None, payload)
        .await
}

pub async fn refresh_runtime_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    if !provider_supports_quota(payload) {
        return Ok(None);
    }

    let lock = acquire_provider_quota_lock(redis_pool, provider_account_id, provider_credential_id)
        .await?;
    let result = refresh_provider_quota_snapshot_locked(
        redis_pool,
        timeout_secs,
        provider_account_id,
        provider_credential_id,
        payload,
    )
    .await;
    let _ = release_provider_quota_lock(redis_pool, &lock).await;
    result
}

pub async fn get_or_refresh_provider_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    payload: &ProviderAccountPayload,
) -> Option<GatewayProviderQuotaView> {
    get_or_refresh_runtime_quota_snapshot(
        redis_pool,
        timeout_secs,
        provider_account_id,
        None,
        payload,
    )
    .await
}

pub async fn get_or_refresh_runtime_quota_snapshot(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Option<GatewayProviderQuotaView> {
    if !provider_supports_quota(payload) {
        return None;
    }

    let cached =
        read_cached_runtime_quota_snapshot(redis_pool, provider_account_id, provider_credential_id)
            .await
            .ok()
            .flatten();

    get_or_refresh_runtime_quota_snapshot_with_cache(
        redis_pool,
        timeout_secs,
        provider_account_id,
        provider_credential_id,
        payload,
        cached,
    )
    .await
}

pub(crate) async fn get_or_refresh_runtime_quota_snapshot_with_cache(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
    cached: Option<GatewayProviderQuotaView>,
) -> Option<GatewayProviderQuotaView> {
    if !provider_supports_quota(payload) {
        return None;
    }

    if cached
        .as_ref()
        .is_some_and(|snapshot| !quota_refresh_due(snapshot))
    {
        return cached;
    }

    match acquire_provider_quota_lock(redis_pool, provider_account_id, provider_credential_id).await
    {
        Ok(lock) => {
            let refreshed = refresh_provider_quota_snapshot_locked(
                redis_pool,
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await;
            let _ = release_provider_quota_lock(redis_pool, &lock).await;
            refreshed.ok().flatten().or(cached)
        }
        Err(_) => cached,
    }
}

async fn refresh_provider_quota_snapshot_locked(
    redis_pool: &Pool,
    timeout_secs: u64,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    payload: &ProviderAccountPayload,
) -> Result<Option<GatewayProviderQuotaView>, GatewayError> {
    let snapshot =
        if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload) {
            fetch_codex_quota_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else if payload.canonical_adapter() == "accio_compatible" {
            fetch_accio_quota_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else if payload.balance_path.is_some() {
            fetch_generic_balance_snapshot(
                timeout_secs,
                provider_account_id,
                provider_credential_id,
                payload,
            )
            .await?
        } else {
            return Ok(None);
        };
    store_cached_provider_quota_snapshot(redis_pool, &snapshot).await?;
    Ok(Some(snapshot))
}
