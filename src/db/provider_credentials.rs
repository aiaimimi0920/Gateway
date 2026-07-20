use serde::{Deserialize, Serialize};
use serde_json::Map;
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::object_storage::{
    build_gateway_provider_credential_object_key, choose_provider_payload_storage_mode,
    gateway_object_storage,
};

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderCredentialView {
    pub id: String,
    pub provider_account_id: String,
    pub label: String,
    pub status: String,
    pub payload: Value,
    pub storage_mode: String,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub source_hash: Option<String>,
    pub sync_mode: String,
    pub sync_state: String,
    pub sync_error: Option<String>,
    pub cooldown_until: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: i32,
    pub last_health_check_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GatewayProviderCredentialRef {
    pub id: String,
    pub provider_account_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderCredentialInput {
    pub provider_account_id: String,
    pub label: String,
    #[serde(default)]
    pub status: Option<String>,
    pub payload: Value,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub sync_mode: Option<String>,
    #[serde(default)]
    pub sync_state: Option<String>,
    #[serde(default)]
    pub sync_error: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderCredentialRow {
    id: String,
    provider_account_id: String,
    label: String,
    status: String,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
    storage_mode: String,
    source_kind: String,
    source_path: Option<String>,
    source_hash: Option<String>,
    sync_mode: String,
    sync_state: String,
    sync_error: Option<String>,
    cooldown_until: Option<OffsetDateTime>,
    last_error: Option<String>,
    failure_count: i32,
    last_health_check_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    archived_at: Option<OffsetDateTime>,
}

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

pub fn merge_provider_account_and_credential_payloads(
    account_payload: &Value,
    credential_payload: &Value,
) -> Value {
    match (account_payload, credential_payload) {
        (Value::Object(account), Value::Object(credential)) => {
            let mut merged = account.clone();
            deep_merge_object(&mut merged, credential);
            Value::Object(merged)
        }
        (_, value) => value.clone(),
    }
}

async fn provider_credential_view_from_row(
    row: GatewayProviderCredentialRow,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    let payload = read_payload_from_row(&row).await?;
    Ok(GatewayProviderCredentialView {
        id: row.id,
        provider_account_id: row.provider_account_id,
        label: row.label,
        status: row.status,
        payload,
        storage_mode: row.storage_mode,
        source_kind: row.source_kind,
        source_path: row.source_path,
        source_hash: row.source_hash,
        sync_mode: row.sync_mode,
        sync_state: row.sync_state,
        sync_error: row.sync_error,
        cooldown_until: row.cooldown_until.map(format_timestamp),
        last_error: row.last_error,
        failure_count: row.failure_count,
        last_health_check_at: row.last_health_check_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        archived_at: row.archived_at.map(format_timestamp),
    })
}

async fn read_payload_from_row(row: &GatewayProviderCredentialRow) -> Result<Value, GatewayError> {
    match &row.payload_inline {
        Some(payload) => Ok(payload.0.clone()),
        None if row.payload_object_key.is_some() => {
            gateway_object_storage()?
                .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                .await
        }
        None => Err(GatewayError::conflict("Provider credential payload 缺失")),
    }
}

async fn persist_payload_for_credential(
    provider_credential_id: &str,
    payload: &Value,
    storage_mode: &str,
    existing_object_key: Option<&str>,
) -> Result<(Option<Value>, Option<String>), GatewayError> {
    if storage_mode == "inline" {
        if let Some(existing_object_key) = existing_object_key {
            gateway_object_storage()?
                .delete_object(existing_object_key)
                .await?;
        }
        return Ok((Some(payload.clone()), None));
    }

    let object_key = existing_object_key
        .map(str::to_string)
        .unwrap_or_else(|| build_gateway_provider_credential_object_key(provider_credential_id));
    gateway_object_storage()?
        .put_json(&object_key, payload)
        .await?;
    Ok((None, Some(object_key)))
}

fn validate_provider_credential_input(
    input: &UpsertProviderCredentialInput,
) -> Result<(), GatewayError> {
    normalize_required_text(&input.provider_account_id, "providerAccountId", 120)?;
    normalize_required_text(&input.label, "Provider credential 标题", 160)?;
    if !input.payload.is_object() {
        return Err(GatewayError::bad_request(
            "provider credential payload 必须是 JSON object",
        ));
    }
    Ok(())
}

fn deep_merge_object(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        if should_preserve_existing_scalar_for_empty_overlay(target.get(key), key, value) {
            continue;
        }
        match (target.get_mut(key), value) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => {
                deep_merge_object(existing, incoming);
            }
            (Some(existing), Value::String(incoming))
                if incoming.trim().is_empty() && !matches!(existing, Value::Null) =>
            {
                // Credential payload overlays are allowed to omit account-owned
                // defaults via blank strings. Keep the non-empty account-side
                // value instead of degrading runtime routing into relative URLs.
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if normalized.chars().count() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    let normalized = value?.trim();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(max_len).collect())
}

fn normalize_provider_credential_status(status: Option<&str>, fallback: &str) -> String {
    status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn archived_at_for_status(
    status: &str,
    existing_archived_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    if status.eq_ignore_ascii_case("archived") {
        return existing_archived_at.or(Some(now));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        archived_at_for_status, merge_provider_account_and_credential_payloads,
        normalize_provider_credential_status,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    #[test]
    fn active_status_clears_archived_at() {
        let now = OffsetDateTime::now_utc();
        let archived = Some(now - time::Duration::hours(1));
        assert_eq!(archived_at_for_status("active", archived, now), None);
        assert_eq!(archived_at_for_status("cooling", archived, now), None);
        assert_eq!(archived_at_for_status("disabled", archived, now), None);
    }

    #[test]
    fn archived_status_preserves_or_sets_timestamp() {
        let now = OffsetDateTime::now_utc();
        let existing = Some(now - time::Duration::hours(2));
        assert_eq!(archived_at_for_status("archived", existing, now), existing);
        assert_eq!(archived_at_for_status("archived", None, now), Some(now));
    }

    #[test]
    fn normalize_provider_credential_status_uses_trimmed_value_or_fallback() {
        assert_eq!(
            normalize_provider_credential_status(Some(" active "), "archived"),
            "active"
        );
        assert_eq!(
            normalize_provider_credential_status(Some("   "), "cooling"),
            "cooling"
        );
        assert_eq!(
            normalize_provider_credential_status(None, "active"),
            "active"
        );
    }

    #[test]
    fn merge_provider_payload_preserves_non_empty_account_values_when_overlay_is_blank() {
        let account = json!({
            "baseUrl": "https://chataibot.pro",
            "apiKey": "",
            "headers": {
                "Origin": "https://chataibot.pro"
            },
            "sessionAuth": {
                "transport": "cookie",
                "primaryCookieName": "token"
            }
        });
        let credential = json!({
            "baseUrl": "",
            "apiKey": "jwt-token",
            "headers": {
                "Cookie": "token=jwt-token"
            }
        });

        let merged = merge_provider_account_and_credential_payloads(&account, &credential);

        assert_eq!(merged["baseUrl"], "https://chataibot.pro");
        assert_eq!(merged["apiKey"], "jwt-token");
        assert_eq!(merged["headers"]["Origin"], "https://chataibot.pro");
        assert_eq!(merged["headers"]["Cookie"], "token=jwt-token");
        assert_eq!(merged["sessionAuth"]["transport"], "cookie");
    }

    #[test]
    fn merge_provider_payload_preserves_account_base_url_when_credential_has_empty_serde_defaults()
    {
        let merged = merge_provider_account_and_credential_payloads(
            &json!({
                "adapter": "qwen_web_compatible",
                "baseUrl": "https://chat.qwen.ai",
                "apiKey": "",
                "headers": {
                    "Origin": "https://chat.qwen.ai"
                }
            }),
            &json!({
                "apiKey": "session-token",
                "baseUrl": "",
                "headers": {
                    "Cookie": "token=abc"
                }
            }),
        );

        assert_eq!(
            merged.get("baseUrl").and_then(|value| value.as_str()),
            Some("https://chat.qwen.ai")
        );
        assert_eq!(
            merged.get("apiKey").and_then(|value| value.as_str()),
            Some("session-token")
        );
        assert_eq!(
            merged
                .get("headers")
                .and_then(|value| value.get("Origin"))
                .and_then(|value| value.as_str()),
            Some("https://chat.qwen.ai")
        );
        assert_eq!(
            merged
                .get("headers")
                .and_then(|value| value.get("Cookie"))
                .and_then(|value| value.as_str()),
            Some("token=abc")
        );
    }
}

fn should_preserve_existing_scalar_for_empty_overlay(
    existing: Option<&Value>,
    key: &str,
    incoming: &Value,
) -> bool {
    if !matches!(
        key,
        "adapter" | "baseUrl" | "base_url" | "apiKey" | "api_key"
    ) {
        return false;
    }

    let Some(existing_text) = existing.and_then(Value::as_str).map(str::trim) else {
        return false;
    };
    let Some(incoming_text) = incoming.as_str().map(str::trim) else {
        return false;
    };

    !existing_text.is_empty() && incoming_text.is_empty()
}
