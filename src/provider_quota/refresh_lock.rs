use crate::error::GatewayError;
use crate::redis::keys;
use deadpool_redis::Pool;
use redis::AsyncCommands;
#[derive(Debug, Clone)]
pub(super) struct ProviderQuotaLock {
    key: String,
    token: String,
}

pub(super) async fn acquire_provider_quota_lock(
    redis_pool: &Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<ProviderQuotaLock, GatewayError> {
    let key = quota_lock_key(provider_account_id, provider_credential_id);
    let token = uuid::Uuid::new_v4().to_string();
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("PX")
        .arg(15_000)
        .arg("NX")
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("acquire provider quota lock: {error}"))
        })?;

    if acquired.as_deref() != Some("OK") {
        return Err(GatewayError::conflict(
            "当前 provider account 正在刷新额度快照，请稍后再试。",
        ));
    }

    Ok(ProviderQuotaLock { key, token })
}

fn quota_lock_key(provider_account_id: &str, provider_credential_id: Option<&str>) -> String {
    provider_credential_id
        .map(keys::provider_credential_quota_lock_key)
        .unwrap_or_else(|| keys::provider_quota_lock_key(provider_account_id))
}

pub(super) async fn release_provider_quota_lock(
    redis_pool: &Pool,
    lock: &ProviderQuotaLock,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let current: Option<String> = conn.get(&lock.key).await.map_err(|error| {
        GatewayError::server_error(format!("read provider quota lock: {error}"))
    })?;
    if current.as_deref() == Some(lock.token.as_str()) {
        let _: usize = conn.del(&lock.key).await.map_err(|error| {
            GatewayError::server_error(format!("release provider quota lock: {error}"))
        })?;
    }
    Ok(())
}
