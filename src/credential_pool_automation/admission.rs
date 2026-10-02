//! Cross-process provider admission supplements route CAS and process-local ordering.
use crate::{error::GatewayError, redis::pool::RedisPool, state::AppState};
use sha2::{Digest, Sha256};
use std::{fs::File, time::Duration};

pub(crate) enum ProviderAdmissionGuard {
    Local {
        _file: File,
    },
    Redis {
        pool: RedisPool,
        key: String,
        token: String,
    },
}

pub(crate) async fn acquire(
    state: &AppState,
    provider_id: &str,
) -> Result<ProviderAdmissionGuard, GatewayError> {
    // Only configured providers can allocate a durable per-provider lock file.
    if !state
        .route_config
        .snapshot()
        .document()
        .providers
        .iter()
        .any(|p| p.id == provider_id)
    {
        return Err(GatewayError::not_found(
            "Credential pool provider no longer exists",
        ));
    }
    if let Some(local) = &state.local_runtime {
        return local
            .try_pool_capacity_lock(provider_id)
            .await?
            .map(|file| ProviderAdmissionGuard::Local { _file: file })
            .ok_or_else(busy);
    }
    let key = format!(
        "gw:credential-pool:admission:{:x}",
        Sha256::digest(provider_id.as_bytes())
    );
    let token = uuid::Uuid::new_v4().to_string();
    let mut connection = state.redis_pool.get().await.map_err(|_| unavailable())?;
    // The driver has an enforced 900s maximum; this bounded lease covers its I/O
    // and commit. Redis expiry also recovers abandoned process/cancellation locks.
    let result: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("NX")
        .arg("EX")
        .arg(1_800)
        .query_async(&mut connection)
        .await
        .map_err(|_| unavailable())?;
    if result.is_none() {
        return Err(busy());
    }
    Ok(ProviderAdmissionGuard::Redis {
        pool: state.redis_pool.clone(),
        key,
        token,
    })
}

impl Drop for ProviderAdmissionGuard {
    fn drop(&mut self) {
        let Self::Redis { pool, key, token } = self else {
            return;
        };
        let (pool, key, token) = (pool.clone(), key.clone(), token.clone());
        // Cancellation must release only our ownership; a successor's token is safe.
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(2), async move {
                    let Ok(mut connection) = pool.get().await else { return; };
                    let _: redis::RedisResult<i64> = redis::Script::new(
                        "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) end return 0"
                    ).key(key).arg(token).invoke_async(&mut connection).await;
                }).await;
            });
        }
    }
}
fn busy() -> GatewayError {
    GatewayError::conflict("该渠道已有凭证池操作进行中，请稍后重试")
        .with_code("credential_pool_admission_busy")
}
fn unavailable() -> GatewayError {
    GatewayError::service_unavailable("Credential pool admission storage is unavailable")
        .with_code("credential_pool_admission_unavailable")
}
