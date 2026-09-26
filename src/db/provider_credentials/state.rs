use super::input::normalize_optional_text;
use super::lookup::get_provider_credential;
use super::GatewayProviderCredentialView;
use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::PgPool;
use time::OffsetDateTime;

pub async fn mark_provider_credential_probe_success(
    pool: &PgPool,
    provider_credential_id: &str,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_credentials
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
    .bind(provider_credential_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    get_provider_credential(pool, provider_credential_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))
}

pub async fn mark_provider_credential_probe_failure(
    pool: &PgPool,
    provider_credential_id: &str,
    message: &str,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let cooldown_until = now + time::Duration::seconds(30);
    sqlx::query(
        r#"
        update gateway_provider_credentials
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
    .bind(provider_credential_id)
    .bind(cooldown_until)
    .bind(message)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    get_provider_credential(pool, provider_credential_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))
}

pub async fn note_provider_credential_runtime_failure(
    pool: &PgPool,
    provider_credential_id: &str,
    failure_count: u64,
    breaker_open: bool,
    ttl_seconds: u64,
    message: &str,
) -> Result<(), GatewayError> {
    let now = OffsetDateTime::now_utc();
    if breaker_open {
        sqlx::query(
            r#"
            update gateway_provider_credentials
            set
              status = 'cooling',
              cooldown_until = $2,
              last_error = $3,
              failure_count = $4,
              updated_at = $5
            where id = $1
            "#,
        )
        .bind(provider_credential_id)
        .bind(now + time::Duration::seconds(ttl_seconds as i64))
        .bind(message)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    } else {
        sqlx::query(
            r#"
            update gateway_provider_credentials
            set
              last_error = $2,
              failure_count = $3,
              updated_at = $4
            where id = $1
            "#,
        )
        .bind(provider_credential_id)
        .bind(message)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    }
    Ok(())
}

pub async fn note_provider_credential_runtime_success(
    pool: &PgPool,
    provider_credential_id: &str,
) -> Result<(), GatewayError> {
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_credentials
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
    .bind(provider_credential_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub async fn update_provider_credential_sync_state(
    pool: &PgPool,
    provider_credential_id: &str,
    sync_state: &str,
    sync_error: Option<&str>,
    source_hash: Option<&str>,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_provider_credentials
        set
          sync_state = $2,
          sync_error = $3,
          source_hash = coalesce($4, source_hash),
          updated_at = $5
        where id = $1
        "#,
    )
    .bind(provider_credential_id)
    .bind(sync_state)
    .bind(sync_error)
    .bind(source_hash)
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub async fn update_provider_credential_sync_metadata(
    pool: &PgPool,
    provider_credential_id: &str,
    source_path: Option<&str>,
    source_hash: Option<&str>,
    sync_mode: Option<&str>,
    sync_state: &str,
    sync_error: Option<&str>,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_provider_credentials
        set
          source_path = $2,
          source_hash = $3,
          sync_mode = coalesce($4, sync_mode),
          sync_state = $5,
          sync_error = $6,
          updated_at = $7
        where id = $1
        "#,
    )
    .bind(provider_credential_id)
    .bind(normalize_optional_text(source_path, 512))
    .bind(normalize_optional_text(source_hash, 160))
    .bind(normalize_optional_text(sync_mode, 80))
    .bind(sync_state)
    .bind(normalize_optional_text(sync_error, 1000))
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}
