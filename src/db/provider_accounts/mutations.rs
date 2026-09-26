use super::input::{
    execution_mode_string, normalize_endpoint_execution_modes, normalize_gateway_execution_mode,
    normalize_optional_text, normalize_required_text, normalize_service_provider_identity,
    payload_base_url, resolve_protocol_family, resolve_protocol_profile,
    validate_provider_account_input,
};
use super::payload::{persist_payload_for_account, provider_account_view_from_row};
use super::{GatewayProviderAccountRow, GatewayProviderAccountView, UpsertProviderAccountInput};
use crate::db::map_db_error;
use crate::error::GatewayError;
use crate::object_storage::{choose_provider_payload_storage_mode, gateway_object_storage};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::PgPool;
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

pub async fn create_provider_account(
    pool: &PgPool,
    input: UpsertProviderAccountInput,
) -> Result<GatewayProviderAccountView, GatewayError> {
    validate_provider_account_input(&input)?;
    let account_id = Uuid::new_v4().to_string();
    let timestamp = OffsetDateTime::now_utc();
    let execution_mode =
        normalize_gateway_execution_mode(&input.adapter, input.execution_mode.as_deref())?;
    let endpoint_execution_modes =
        normalize_endpoint_execution_modes(&input.adapter, input.endpoint_execution_modes.clone())?;
    let (service_provider_key, service_provider_label) = normalize_service_provider_identity(
        &input.label,
        &input.service_provider_key,
        &input.service_provider_label,
    )?;
    let storage_mode = choose_provider_payload_storage_mode(&input.payload);
    let (payload_inline, payload_object_key) =
        persist_payload_for_account(&account_id, &input.payload, storage_mode, None).await?;

    let row = sqlx::query_as::<_, GatewayProviderAccountRow>(
        r#"
        insert into gateway_provider_accounts (
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, payload_content_type, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at, archived_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8,
          $9, $10, $11, $12,
          $13, $14,
          $15, $16, 'application/json', $17,
          null, null, 0, null, $18, $18, null
        )
        returning
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at
        "#,
    )
    .bind(&account_id)
    .bind(normalize_required_text(&input.label, "Provider account 标题", 120)?)
    .bind(&service_provider_key)
    .bind(&service_provider_label)
    .bind(&input.adapter)
    .bind(normalize_required_text(
        resolve_protocol_family(
            input.protocol_family.as_str(),
            input.protocol_profile.as_deref(),
            &input.adapter,
            input
                .service_provider_key
                .as_deref()
                .or(input.service_provider_label.as_deref())
                .or(Some(input.label.as_str())),
            payload_base_url(&input.payload),
        )
        .as_str(),
        "protocolFamily",
        120,
    )?)
    .bind(resolve_protocol_profile(
        input.protocol_profile.as_deref(),
        &input.adapter,
        input
            .service_provider_key
            .as_deref()
            .or(input.service_provider_label.as_deref())
            .or(Some(input.label.as_str())),
        payload_base_url(&input.payload),
    ))
    .bind(input.status.as_deref().unwrap_or("active"))
    .bind(input.source_kind.as_deref())
    .bind(input.aggregator_api_mode.as_deref())
    .bind(input.web_reverse_access_mode.as_deref())
    .bind(normalize_optional_text(input.source_notes.as_deref(), 500))
    .bind(execution_mode_string(execution_mode))
    .bind(endpoint_execution_modes.as_ref().map(|value| Json(serde_json::to_value(value).unwrap_or(Value::Null))))
    .bind(payload_inline.map(Json))
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(timestamp)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    provider_account_view_from_row(row).await
}

pub async fn update_provider_account(
    pool: &PgPool,
    provider_account_id: &str,
    input: UpsertProviderAccountInput,
) -> Result<GatewayProviderAccountView, GatewayError> {
    validate_provider_account_input(&input)?;
    let existing = sqlx::query_as::<_, GatewayProviderAccountRow>(
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
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let execution_mode = normalize_gateway_execution_mode(
        &input.adapter,
        input
            .execution_mode
            .as_deref()
            .or(Some(existing.execution_mode.as_str())),
    )?;
    let endpoint_execution_modes = normalize_endpoint_execution_modes(
        &input.adapter,
        input.endpoint_execution_modes.clone().or_else(|| {
            existing
                .endpoint_execution_modes
                .as_ref()
                .and_then(|value| {
                    serde_json::from_value::<HashMap<String, String>>(value.0.clone()).ok()
                })
        }),
    )?;
    let effective_service_provider_key = input
        .service_provider_key
        .clone()
        .or_else(|| Some(existing.service_provider_key.clone()));
    let effective_service_provider_label = input
        .service_provider_label
        .clone()
        .or_else(|| Some(existing.service_provider_label.clone()));
    let (service_provider_key, service_provider_label) = normalize_service_provider_identity(
        &input.label,
        &effective_service_provider_key,
        &effective_service_provider_label,
    )?;
    let storage_mode = choose_provider_payload_storage_mode(&input.payload);
    let (payload_inline, payload_object_key) = persist_payload_for_account(
        provider_account_id,
        &input.payload,
        storage_mode,
        existing.payload_object_key.as_deref(),
    )
    .await?;

    let row = sqlx::query_as::<_, GatewayProviderAccountRow>(
        r#"
        update gateway_provider_accounts
        set
          label = $2,
          service_provider_key = $3,
          service_provider_label = $4,
          adapter = $5,
          protocol_family = $6,
          protocol_profile = $7,
          status = $8,
          source_kind = $9,
          aggregator_api_mode = $10,
          web_reverse_access_mode = $11,
          source_notes = $12,
          execution_mode = $13,
          endpoint_execution_modes = $14,
          payload_inline = $15,
          payload_object_key = $16,
          payload_content_type = 'application/json',
          storage_mode = $17,
          updated_at = $18
        where id = $1
        returning
          id, label, service_provider_key, service_provider_label, adapter, protocol_family, protocol_profile, status,
          source_kind, aggregator_api_mode, web_reverse_access_mode, source_notes,
          execution_mode, endpoint_execution_modes,
          payload_inline, payload_object_key, storage_mode,
          cooldown_until, last_error, failure_count, last_health_check_at, created_at, updated_at
        "#,
    )
    .bind(provider_account_id)
    .bind(normalize_required_text(
        &input.label,
        "Provider account 标题",
        120,
    )?)
    .bind(&service_provider_key)
    .bind(&service_provider_label)
    .bind(&input.adapter)
    .bind(normalize_required_text(
        resolve_protocol_family(
            input.protocol_family.as_str(),
            input.protocol_profile.as_deref(),
            &input.adapter,
            effective_service_provider_key
                .as_deref()
                .or(effective_service_provider_label.as_deref())
                .or(Some(input.label.as_str())),
            payload_base_url(&input.payload),
        )
        .as_str(),
        "protocolFamily",
        120,
    )?)
    .bind(resolve_protocol_profile(
        input.protocol_profile.as_deref(),
        &input.adapter,
        effective_service_provider_key
            .as_deref()
            .or(effective_service_provider_label.as_deref())
            .or(Some(input.label.as_str())),
        payload_base_url(&input.payload),
    ))
    .bind(input.status.as_deref().unwrap_or(existing.status.as_str()))
    .bind(input.source_kind.as_deref())
    .bind(input.aggregator_api_mode.as_deref())
    .bind(input.web_reverse_access_mode.as_deref())
    .bind(normalize_optional_text(input.source_notes.as_deref(), 500))
    .bind(execution_mode_string(execution_mode))
    .bind(
        endpoint_execution_modes
            .as_ref()
            .map(|value| Json(serde_json::to_value(value).unwrap_or(Value::Null))),
    )
    .bind(payload_inline.map(Json))
    .bind(payload_object_key.as_deref())
    .bind(storage_mode)
    .bind(OffsetDateTime::now_utc())
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    provider_account_view_from_row(row).await
}

pub async fn delete_provider_account(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    let existing = sqlx::query_as::<_, GatewayProviderAccountRow>(
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
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    if let Some(existing_key) = existing.payload_object_key.as_deref() {
        gateway_object_storage()?
            .delete_object(existing_key)
            .await?;
    }

    let result = sqlx::query(
        r#"
        delete from gateway_provider_accounts
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    Ok(())
}
