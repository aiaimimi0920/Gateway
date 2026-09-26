//! Persistent provider probe and cooling-retry state.

use super::*;

pub async fn mark_provider_probe_success(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
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

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn mark_provider_probe_failure(
    pool: &PgPool,
    provider_account_id: &str,
    message: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let cooldown_until = now + time::Duration::seconds(30);
    let error_message = truncate_error_summary(message, 1000);
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = case when status = 'archived' then status else 'cooling' end,
          cooldown_until = case when status = 'archived' then cooldown_until else $2 end,
          last_error = $3,
          failure_count = failure_count + 1,
          last_health_check_at = $4,
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(cooldown_until)
    .bind(error_message)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn mark_provider_cooling_retry_failure(
    pool: &PgPool,
    provider_account_id: &str,
    message: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let cooldown_until = now + time::Duration::seconds(30);
    let error_message = truncate_error_summary(message, 1000);
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'cooling',
          cooldown_until = $2,
          last_error = $3,
          last_health_check_at = $4,
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(cooldown_until)
    .bind(error_message)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn list_expired_cooling_provider_account_ids(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<String>, GatewayError> {
    sqlx::query_scalar::<_, String>(
        r#"
        select id
        from gateway_provider_accounts
        where status = 'cooling'
          and cooldown_until is not null
          and cooldown_until <= $1
        order by cooldown_until asc
        limit $2
        "#,
    )
    .bind(OffsetDateTime::now_utc())
    .bind(limit.max(1).min(100))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}
