use redis::AsyncCommands;

use crate::error::GatewayError;

#[derive(Debug, Clone)]
pub(super) struct ProviderProbeLock {
    key: String,
    token: String,
}

pub(super) async fn acquire_provider_probe_lock(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
) -> Result<ProviderProbeLock, GatewayError> {
    let key = crate::redis::keys::provider_probe_lock_key(provider_account_id);
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
            GatewayError::server_error(format!("acquire provider probe lock: {error}"))
        })?;

    if acquired.as_deref() != Some("OK") {
        return Err(GatewayError::conflict(
            "当前 provider account 正在执行 probe，请稍后再试。",
        ));
    }

    Ok(ProviderProbeLock { key, token })
}

pub(super) async fn release_provider_probe_lock(
    redis_pool: &deadpool_redis::Pool,
    lock: &ProviderProbeLock,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let current: Option<String> = conn.get(&lock.key).await.map_err(|error| {
        GatewayError::server_error(format!("read provider probe lock: {error}"))
    })?;
    if current.as_deref() == Some(lock.token.as_str()) {
        let _: usize = conn.del(&lock.key).await.map_err(|error| {
            GatewayError::server_error(format!("release provider probe lock: {error}"))
        })?;
    }
    Ok(())
}
