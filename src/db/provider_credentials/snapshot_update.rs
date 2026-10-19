//! Serialize credential update side effects and reject delayed import snapshots.
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
use crate::object_storage::choose_provider_payload_storage_mode;
use sqlx::types::Json;
use sqlx::PgPool;
use time::OffsetDateTime;

/// Commit a short-lived import snapshot only while its PostgreSQL row version matches.
/// The opaque token is run-local metadata, never a durable or ordered business revision.
pub async fn update_provider_credential_from_snapshot(
    pool: &PgPool,
    credential_id: &str,
    snapshot_version: &str,
    input: UpsertProviderCredentialInput,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    update(pool, credential_id, Some(snapshot_version), input).await
}

pub(super) async fn update(
    pool: &PgPool,
    provider_credential_id: &str,
    snapshot_version: Option<&str>,
    input: UpsertProviderCredentialInput,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    validate_provider_credential_input(&input)?;
    // The row lock covers validation and payload I/O, not just the final SQL write.
    let mut transaction = pool.begin().await.map_err(map_db_error)?;
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
        where id = $1 and ($2::text is null or xmin::text = $2)
        limit 1
        for update
        "#,
    )
    .bind(provider_credential_id)
    .bind(snapshot_version)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| match snapshot_version {
        Some(_) => GatewayError::conflict("Provider credential snapshot is no longer current")
            .with_code("provider_credential_snapshot_conflict"),
        None => GatewayError::not_found("Provider credential 不存在"),
    })?;

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
    .fetch_one(&mut *transaction)
    .await
    .map_err(map_db_error)?;

    transaction.commit().await.map_err(map_db_error)?;
    // Read the response after commit; object storage is not part of the PG transaction.
    provider_credential_view_from_row(row).await
}
