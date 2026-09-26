use super::payload::provider_account_view_from_row;
use super::{GatewayProviderAccountRow, GatewayProviderAccountView};
use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::{FromRow, PgPool};

#[derive(Debug, FromRow)]
struct ProviderAccountInlinePayloadSize {
    payload_inline_bytes: Option<i64>,
}

pub async fn list_provider_accounts(
    pool: &PgPool,
) -> Result<Vec<GatewayProviderAccountView>, GatewayError> {
    recover_expired_cooling_provider_accounts(pool).await?;
    let rows = sqlx::query_as::<_, GatewayProviderAccountRow>(
        r#"
        select
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at
        from gateway_provider_accounts
        order by created_at asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        views.push(provider_account_view_from_row(row).await?);
    }
    Ok(views)
}

pub async fn get_provider_account(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<Option<GatewayProviderAccountView>, GatewayError> {
    recover_expired_cooling_provider_accounts(pool).await?;
    let row = sqlx::query_as::<_, GatewayProviderAccountRow>(
        r#"
        select
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    match row {
        Some(row) => Ok(Some(provider_account_view_from_row(row).await?)),
        None => Ok(None),
    }
}

pub(crate) async fn get_provider_account_for_folder_sync(
    pool: &PgPool,
    provider_account_id: &str,
    max_inline_payload_bytes: usize,
) -> Result<Option<GatewayProviderAccountView>, GatewayError> {
    recover_expired_cooling_provider_accounts(pool).await?;
    let mut transaction = pool.begin().await.map_err(map_db_error)?;
    let payload_size = sqlx::query_as::<_, ProviderAccountInlinePayloadSize>(
        r#"
        select octet_length(payload_inline::text)::bigint as payload_inline_bytes
        from gateway_provider_accounts
        where id = $1
        limit 1
        for update
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(map_db_error)?;

    let Some(payload_size) = payload_size else {
        return Ok(None);
    };
    if payload_size.payload_inline_bytes.is_some_and(|bytes| {
        usize::try_from(bytes).map_or(true, |bytes| bytes > max_inline_payload_bytes)
    }) {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder account payload exceeds {max_inline_payload_bytes} byte limit"
        ))
        .with_code("provider_credential_folder_sync_account_payload_too_large"));
    }

    let row = sqlx::query_as::<_, GatewayProviderAccountRow>(
        r#"
        select
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(map_db_error)?;
    transaction.commit().await.map_err(map_db_error)?;

    match row {
        Some(row) => Ok(Some(provider_account_view_from_row(row).await?)),
        None => Ok(None),
    }
}

pub async fn recover_expired_cooling_provider_accounts(pool: &PgPool) -> Result<u64, GatewayError> {
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'active',
          cooldown_until = null,
          failure_count = 0,
          updated_at = now()
        where status = 'cooling'
          and cooldown_until is not null
          and cooldown_until <= now()
        "#,
    )
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(result.rows_affected())
}
