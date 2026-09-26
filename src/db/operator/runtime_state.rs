//! Provider breaker keys and ordered Redis/database state updates.

use super::*;
use crate::redis::keys;
use redis::AsyncCommands;

pub async fn clear_provider_runtime_keys(
    redis_pool: &Pool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: usize = conn
        .del(&[
            keys::provider_failure_count_key(provider_account_id),
            keys::provider_breaker_open_key(provider_account_id),
            keys::provider_quota_snapshot_key(provider_account_id),
            keys::provider_quota_lock_key(provider_account_id),
        ])
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("clear provider runtime keys: {error}"))
        })?;
    Ok(())
}

pub async fn note_provider_runtime_failure(
    redis_pool: &Pool,
    pool: &PgPool,
    provider_account_id: &str,
    route_policy: &GatewayRoutePolicyConfig,
    message: &str,
) -> Result<(), GatewayError> {
    let ttl_seconds = route_policy.circuit_breaker_cooldown_seconds.max(30) as u64;
    let threshold = route_policy.circuit_breaker_threshold.max(1) as u64;
    let failure_count_key = keys::provider_failure_count_key(provider_account_id);
    let breaker_open_key = keys::provider_breaker_open_key(provider_account_id);
    let failure_count = {
        let mut conn = redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
        let failure_count: u64 = conn.incr(&failure_count_key, 1).await.map_err(|error| {
            GatewayError::server_error(format!("increment provider failures: {error}"))
        })?;
        let _: bool = conn
            .expire(&failure_count_key, ttl_seconds as i64)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("expire provider failures: {error}"))
            })?;
        if failure_count >= threshold {
            let _: () = redis::cmd("SET")
                .arg(&breaker_open_key)
                .arg("1")
                .arg("EX")
                .arg(ttl_seconds)
                .query_async(&mut conn)
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("open provider breaker: {error}"))
                })?;
        }
        failure_count
    };

    let now = OffsetDateTime::now_utc();
    let truncated = truncate_error_summary(message, 1000);
    if failure_count >= threshold {
        sqlx::query(
            r#"
            update gateway_provider_accounts
            set
              status = 'cooling',
              cooldown_until = $2,
              last_error = $3,
              failure_count = $4,
              updated_at = $5
            where id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(now + time::Duration::seconds(ttl_seconds as i64))
        .bind(truncated)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    } else {
        sqlx::query(
            r#"
            update gateway_provider_accounts
            set
              last_error = $2,
              failure_count = $3,
              updated_at = $4
            where id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(truncated)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    }

    Ok(())
}

pub async fn note_provider_runtime_success(
    redis_pool: &Pool,
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    clear_provider_runtime_keys(redis_pool, provider_account_id).await?;
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'active',
          cooldown_until = null,
          last_error = null,
          failure_count = 0,
          last_health_check_at = $2,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn read_breaker_open_map(
    redis_pool: &Pool,
    provider_ids: &[String],
) -> HashMap<String, bool> {
    let mut result = HashMap::new();
    let Ok(mut conn) = redis_pool.get().await else {
        return result;
    };

    for provider_id in provider_ids {
        let breaker_open = conn
            .get::<_, Option<String>>(keys::provider_breaker_open_key(provider_id))
            .await
            .ok()
            .flatten()
            .is_some();
        result.insert(provider_id.clone(), breaker_open);
    }

    result
}
