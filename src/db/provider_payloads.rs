//! Provider payload persistence and object-storage lifecycle.

use super::*;

pub async fn get_provider_payload(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<Option<GatewayProviderPayloadView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
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
        None => Ok(None),
        Some(row) => {
            let payload = match row.payload_inline {
                Some(payload) => payload.0,
                None if row.payload_object_key.is_some() => {
                    gateway_object_storage()?
                        .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                        .await?
                }
                None => return Err(GatewayError::conflict("Provider account payload 缺失")),
            };
            Ok(Some(GatewayProviderPayloadView {
                provider_account_id: row.id,
                payload,
                storage_mode: row.storage_mode,
                status: row.status,
                updated_at: format_timestamp(row.updated_at),
            }))
        }
    }
}

pub async fn save_provider_payload_inline(
    pool: &PgPool,
    provider_account_id: &str,
    payload: Value,
) -> Result<GatewayProviderPayloadView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let existing = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let storage_mode = choose_provider_payload_storage_mode(&payload);
    let object_storage = gateway_object_storage()?;
    let payload_object_key = if storage_mode == "inline" {
        if let Some(existing_key) = existing.payload_object_key.as_deref() {
            object_storage.delete_object(existing_key).await?;
        }
        None
    } else {
        let object_key = existing
            .payload_object_key
            .clone()
            .unwrap_or_else(|| build_gateway_provider_account_object_key(provider_account_id));
        object_storage.put_json(&object_key, &payload).await?;
        Some(object_key)
    };
    let updated = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        update gateway_provider_accounts
        set
          payload_inline = $2,
          payload_object_key = $3,
          payload_content_type = 'application/json',
          storage_mode = $4,
          updated_at = $5
        where id = $1
        returning id, payload_inline, payload_object_key, storage_mode, status, updated_at
        "#,
    )
    .bind(provider_account_id)
    .bind(
        (storage_mode == "inline")
            .then_some(Json(payload.clone()))
            .map(|value| value),
    )
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let row = updated.ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    Ok(GatewayProviderPayloadView {
        provider_account_id: row.id,
        payload,
        storage_mode: row.storage_mode,
        status: row.status,
        updated_at: format_timestamp(row.updated_at),
    })
}

pub async fn delete_provider_payload(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    let existing = sqlx::query_as::<_, GatewayProviderPayloadRow>(
        r#"
        select id, payload_inline, payload_object_key, storage_mode, status, updated_at
        from gateway_provider_accounts
        where id = $1
        limit 1
        "#,
    )
    .bind(provider_account_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    if let Some(existing_key) = existing.payload_object_key.as_deref() {
        gateway_object_storage()?
            .delete_object(existing_key)
            .await?;
    }
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          payload_inline = null,
          payload_object_key = null,
          storage_mode = 'inline',
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
    Ok(())
}

pub async fn list_active_provider_account_ids(pool: &PgPool) -> Result<Vec<String>, GatewayError> {
    let rows = sqlx::query(
        r#"
        select id
        from gateway_provider_accounts
        where status = 'active'
        order by updated_at desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    rows.into_iter()
        .map(|row| row.try_get::<String, _>("id").map_err(map_db_decode_error))
        .collect()
}
