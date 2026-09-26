use super::input::{
    archived_at_for_status, normalize_optional_text, normalize_provider_credential_status,
    normalize_required_text, validate_provider_credential_input,
};
use super::payload::{persist_payload_for_credential, provider_credential_view_from_row};
use super::{
    GatewayProviderCredentialRow, GatewayProviderCredentialView, UpsertProviderCredentialInput,
};
use crate::db::map_db_error;
use crate::error::GatewayError;
use crate::object_storage::{choose_provider_payload_storage_mode, gateway_object_storage};
use sqlx::types::Json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn create_provider_credential(
    pool: &PgPool,
    input: UpsertProviderCredentialInput,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    validate_provider_credential_input(&input)?;
    let credential_id = Uuid::new_v4().to_string();
    let now = OffsetDateTime::now_utc();
    let effective_status = normalize_provider_credential_status(input.status.as_deref(), "active");
    let archived_at = archived_at_for_status(effective_status.as_str(), None, now);
    let storage_mode = choose_provider_payload_storage_mode(&input.payload);
    let (payload_inline, payload_object_key) =
        persist_payload_for_credential(&credential_id, &input.payload, storage_mode, None).await?;

    let row = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        insert into gateway_provider_credentials (
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, payload_content_type, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        ) values (
          $1, $2, $3, $4,
          $5, $6, 'application/json', $7,
          $8, $9, $10,
          $11, $12, $13,
          null, null, 0, null,
          $14, $14, $15
        )
        returning
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        "#,
    )
    .bind(&credential_id)
    .bind(&input.provider_account_id)
    .bind(normalize_required_text(
        &input.label,
        "Provider credential 标题",
        160,
    )?)
    .bind(&effective_status)
    .bind(payload_inline.map(Json))
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(
        input
            .source_kind
            .as_deref()
            .unwrap_or("manual")
            .trim()
            .to_string(),
    )
    .bind(normalize_optional_text(input.source_path.as_deref(), 512))
    .bind(normalize_optional_text(input.source_hash.as_deref(), 160))
    .bind(
        input
            .sync_mode
            .as_deref()
            .unwrap_or("manual")
            .trim()
            .to_string(),
    )
    .bind(
        input
            .sync_state
            .as_deref()
            .unwrap_or("idle")
            .trim()
            .to_string(),
    )
    .bind(normalize_optional_text(input.sync_error.as_deref(), 1000))
    .bind(now)
    .bind(archived_at)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    provider_credential_view_from_row(row).await
}

pub async fn update_provider_credential(
    pool: &PgPool,
    provider_credential_id: &str,
    input: UpsertProviderCredentialInput,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    validate_provider_credential_input(&input)?;
    let existing = sqlx::query_as::<_, GatewayProviderCredentialRow>(
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
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;

    let storage_mode = choose_provider_payload_storage_mode(&input.payload);
    let now = OffsetDateTime::now_utc();
    let effective_status =
        normalize_provider_credential_status(input.status.as_deref(), existing.status.as_str());
    let archived_at = archived_at_for_status(effective_status.as_str(), existing.archived_at, now);
    let (payload_inline, payload_object_key) = persist_payload_for_credential(
        provider_credential_id,
        &input.payload,
        storage_mode,
        existing.payload_object_key.as_deref(),
    )
    .await?;

    let row = sqlx::query_as::<_, GatewayProviderCredentialRow>(
        r#"
        update gateway_provider_credentials
        set
          provider_account_id = $2,
          label = $3,
          status = $4,
          payload_inline = $5,
          payload_object_key = $6,
          payload_content_type = 'application/json',
          storage_mode = $7,
          source_kind = $8,
          source_path = $9,
          source_hash = $10,
          sync_mode = $11,
          sync_state = $12,
          sync_error = $13,
          updated_at = $14,
          archived_at = $15
        where id = $1
        returning
          id, provider_account_id, label, status,
          payload_inline, payload_object_key, storage_mode,
          source_kind, source_path, source_hash,
          sync_mode, sync_state, sync_error,
          cooldown_until, last_error, failure_count, last_health_check_at,
          created_at, updated_at, archived_at
        "#,
    )
    .bind(provider_credential_id)
    .bind(&input.provider_account_id)
    .bind(normalize_required_text(
        &input.label,
        "Provider credential 标题",
        160,
    )?)
    .bind(&effective_status)
    .bind(payload_inline.map(Json))
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(
        input
            .source_kind
            .as_deref()
            .unwrap_or(existing.source_kind.as_str())
            .trim()
            .to_string(),
    )
    .bind(normalize_optional_text(
        input
            .source_path
            .as_deref()
            .or(existing.source_path.as_deref()),
        512,
    ))
    .bind(normalize_optional_text(
        input
            .source_hash
            .as_deref()
            .or(existing.source_hash.as_deref()),
        160,
    ))
    .bind(
        input
            .sync_mode
            .as_deref()
            .unwrap_or(existing.sync_mode.as_str())
            .trim()
            .to_string(),
    )
    .bind(
        input
            .sync_state
            .as_deref()
            .unwrap_or(existing.sync_state.as_str())
            .trim()
            .to_string(),
    )
    .bind(normalize_optional_text(input.sync_error.as_deref(), 1000))
    .bind(now)
    .bind(archived_at)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    provider_credential_view_from_row(row).await
}

pub async fn delete_provider_credential(
    pool: &PgPool,
    provider_credential_id: &str,
) -> Result<(), GatewayError> {
    let existing = sqlx::query_as::<_, GatewayProviderCredentialRow>(
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
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;

    if let Some(existing_key) = existing.payload_object_key.as_deref() {
        gateway_object_storage()?
            .delete_object(existing_key)
            .await?;
    }

    let result = sqlx::query(
        r#"
        delete from gateway_provider_credentials
        where id = $1
        "#,
    )
    .bind(provider_credential_id)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider credential 不存在"));
    }

    Ok(())
}
