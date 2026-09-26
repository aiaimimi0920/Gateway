use super::payload::provider_credential_view_from_row;
use super::{
    GatewayProviderCredentialRef, GatewayProviderCredentialRow, GatewayProviderCredentialView,
};
use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::PgPool;
use time::OffsetDateTime;

pub async fn list_provider_credentials(
    pool: &PgPool,
    provider_account_id: Option<&str>,
) -> Result<Vec<GatewayProviderCredentialView>, GatewayError> {
    let rows = if let Some(provider_account_id) = provider_account_id {
        sqlx::query_as::<_, GatewayProviderCredentialRow>(
            r#"
            select
              id, provider_account_id, label, status,
              payload_inline, payload_object_key, storage_mode,
              source_kind, source_path, source_hash,
              sync_mode, sync_state, sync_error,
              cooldown_until, last_error, failure_count, last_health_check_at,
              created_at, updated_at, archived_at
            from gateway_provider_credentials
            where provider_account_id = $1
            order by created_at asc
            "#,
        )
        .bind(provider_account_id)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as::<_, GatewayProviderCredentialRow>(
            r#"
            select
              id, provider_account_id, label, status,
              payload_inline, payload_object_key, storage_mode,
              source_kind, source_path, source_hash,
              sync_mode, sync_state, sync_error,
              cooldown_until, last_error, failure_count, last_health_check_at,
              created_at, updated_at, archived_at
            from gateway_provider_credentials
            order by created_at asc
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    };

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        views.push(provider_credential_view_from_row(row).await?);
    }
    Ok(views)
}

pub async fn list_active_provider_credentials_for_accounts(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<Vec<GatewayProviderCredentialView>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(Vec::new());
    }
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_credentials
        set
          status = 'active',
          cooldown_until = null,
          last_error = null,
          failure_count = 0,
          updated_at = $2
        where provider_account_id = any($1)
          and archived_at is null
          and status = 'cooling'
          and cooldown_until is not null
          and cooldown_until <= $2
              and (
                last_error is null
                or (
                  lower(last_error) not like '%deactivated_workspace%'
                  and lower(last_error) not like '%token_invalidated%'
                  and lower(last_error) not like '%token_revoked%'
                  and lower(last_error) not like '%token has been invalidated%'
                  and lower(last_error) not like '%invalid api key%'
                  and lower(last_error) not like '%invalid_api_key%'
                  and lower(last_error) not like '%appidnoautherror%'
                )
              )
        "#,
    )
    .bind(provider_account_ids)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let rows = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        select
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        from gateway_provider_credentials
        where provider_account_id = any($1)
          and archived_at is null
          and status = 'active'
        order by created_at asc
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        views.push(provider_credential_view_from_row(row).await?);
    }
    Ok(views)
}

pub async fn list_active_provider_credential_refs_for_accounts(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<Vec<GatewayProviderCredentialRef>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(Vec::new());
    }
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_credentials
        set
          status = 'active',
          cooldown_until = null,
          last_error = null,
          failure_count = 0,
          updated_at = $2
        where provider_account_id = any($1)
          and archived_at is null
          and status = 'cooling'
          and cooldown_until is not null
          and cooldown_until <= $2
              and (
                last_error is null
                or (
                  lower(last_error) not like '%deactivated_workspace%'
                  and lower(last_error) not like '%token_invalidated%'
                  and lower(last_error) not like '%token_revoked%'
                  and lower(last_error) not like '%token has been invalidated%'
                  and lower(last_error) not like '%invalid api key%'
                  and lower(last_error) not like '%invalid_api_key%'
                  and lower(last_error) not like '%appidnoautherror%'
                )
              )
        "#,
    )
    .bind(provider_account_ids)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    sqlx::query_as::<_, GatewayProviderCredentialRef>(
        r#"
        select id, provider_account_id
        from gateway_provider_credentials
        where provider_account_id = any($1)
          and archived_at is null
          and status = 'active'
        order by created_at asc
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

pub async fn list_active_provider_credentials_for_adapter(
    pool: &PgPool,
    adapter: &str,
    limit: i64,
) -> Result<Vec<GatewayProviderCredentialView>, GatewayError> {
    let rows = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        select
          c.id, c.provider_account_id, c.label, c.status,
          c.payload_inline, c.payload_object_key, c.storage_mode,
          c.source_kind, c.source_path, c.source_hash,
          c.sync_mode, c.sync_state, c.sync_error,
          c.cooldown_until, c.last_error, c.failure_count, c.last_health_check_at,
          c.created_at, c.updated_at, c.archived_at
        from gateway_provider_credentials c
        join gateway_provider_accounts a on a.id = c.provider_account_id
        where c.archived_at is null
          and c.status = 'active'
          and a.status = 'active'
          and a.adapter = $1
        order by c.updated_at asc, c.created_at asc
        limit $2
        "#,
    )
    .bind(adapter.trim())
    .bind(limit.max(1))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        views.push(provider_credential_view_from_row(row).await?);
    }
    Ok(views)
}

pub async fn get_provider_credential(
    pool: &PgPool,
    provider_credential_id: &str,
) -> Result<Option<GatewayProviderCredentialView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        select
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        from gateway_provider_credentials
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_credential_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    match row {
        Some(row) => Ok(Some(provider_credential_view_from_row(row).await?)),
        None => Ok(None),
    }
}

pub async fn get_provider_credential_by_source_path(
    pool: &PgPool,
    source_path: &str,
) -> Result<Option<GatewayProviderCredentialView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        select
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        from gateway_provider_credentials
        where source_path = $1
        limit 1
        "#,
    )
    .bind(source_path)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    match row {
        Some(row) => Ok(Some(provider_credential_view_from_row(row).await?)),
        None => Ok(None),
    }
}

pub async fn list_expired_cooling_provider_credential_ids(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<String>, GatewayError> {
    sqlx::query_scalar::<_, String>(
        r#"
        select id
        from gateway_provider_credentials
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
