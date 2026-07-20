use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::object_storage::{
    build_gateway_provider_account_object_key, choose_provider_payload_storage_mode,
    gateway_object_storage,
};
use crate::protocol::registry::{
    canonicalize_protocol_family_key, infer_protocol_family, infer_protocol_profile,
};
use crate::routing::candidate::ProviderExecutionMode;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderAccountView {
    pub id: String,
    pub label: String,
    pub service_provider_key: String,
    pub service_provider_label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub protocol_profile: String,
    pub status: String,
    pub source_kind: Option<String>,
    pub aggregator_api_mode: Option<String>,
    pub web_reverse_access_mode: Option<String>,
    pub source_notes: Option<String>,
    pub execution_mode: ProviderExecutionMode,
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
    pub payload: Value,
    pub storage_mode: String,
    pub cooldown_until: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: i32,
    pub last_health_check_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderAccountInput {
    pub label: String,
    #[serde(default)]
    pub service_provider_key: Option<String>,
    #[serde(default)]
    pub service_provider_label: Option<String>,
    pub adapter: String,
    pub protocol_family: String,
    #[serde(default)]
    pub protocol_profile: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub aggregator_api_mode: Option<String>,
    #[serde(default)]
    pub web_reverse_access_mode: Option<String>,
    #[serde(default)]
    pub source_notes: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, String>>,
    pub payload: Value,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderAccountRow {
    id: String,
    label: String,
    service_provider_key: String,
    service_provider_label: String,
    adapter: String,
    protocol_family: String,
    protocol_profile: String,
    status: String,
    source_kind: Option<String>,
    aggregator_api_mode: Option<String>,
    web_reverse_access_mode: Option<String>,
    source_notes: Option<String>,
    execution_mode: String,
    endpoint_execution_modes: Option<Json<Value>>,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
    storage_mode: String,
    cooldown_until: Option<OffsetDateTime>,
    last_error: Option<String>,
    failure_count: i32,
    last_health_check_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
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

async fn provider_account_view_from_row(
    row: GatewayProviderAccountRow,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let payload = read_payload_from_row(&row).await?;
    Ok(GatewayProviderAccountView {
        id: row.id,
        label: row.label,
        service_provider_key: row.service_provider_key,
        service_provider_label: row.service_provider_label,
        adapter: row.adapter,
        protocol_family: row.protocol_family,
        protocol_profile: row.protocol_profile,
        status: row.status,
        source_kind: row.source_kind,
        aggregator_api_mode: row.aggregator_api_mode,
        web_reverse_access_mode: row.web_reverse_access_mode,
        source_notes: row.source_notes,
        execution_mode: parse_execution_mode(row.execution_mode.as_str())?,
        endpoint_execution_modes: row
            .endpoint_execution_modes
            .map(|value| serde_json::from_value(value.0).unwrap_or_default()),
        payload,
        storage_mode: row.storage_mode,
        cooldown_until: row.cooldown_until.map(format_timestamp),
        last_error: row.last_error,
        failure_count: row.failure_count,
        last_health_check_at: row.last_health_check_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

async fn read_payload_from_row(row: &GatewayProviderAccountRow) -> Result<Value, GatewayError> {
    match &row.payload_inline {
        Some(payload) => Ok(payload.0.clone()),
        None if row.payload_object_key.is_some() => {
            gateway_object_storage()?
                .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                .await
        }
        None => Err(GatewayError::conflict("Provider account payload 缺失")),
    }
}

async fn persist_payload_for_account(
    provider_account_id: &str,
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
        .unwrap_or_else(|| build_gateway_provider_account_object_key(provider_account_id));
    gateway_object_storage()?
        .put_json(&object_key, payload)
        .await?;
    Ok((None, Some(object_key)))
}

fn validate_provider_account_input(input: &UpsertProviderAccountInput) -> Result<(), GatewayError> {
    normalize_required_text(&input.label, "Provider account 标题", 120)?;
    normalize_service_provider_identity(
        &input.label,
        &input.service_provider_key,
        &input.service_provider_label,
    )?;
    normalize_required_text(&input.adapter, "adapter", 120)?;
    normalize_required_text(
        canonicalize_protocol_family_key(&input.protocol_family).as_str(),
        "protocolFamily",
        120,
    )?;

    let payload_adapter = input
        .payload
        .get("adapter")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default();
    if payload_adapter.is_empty() {
        return Err(GatewayError::bad_request("payload.adapter 不能为空"));
    }
    if payload_adapter != input.adapter.trim() {
        return Err(GatewayError::conflict(
            "payload.adapter 必须与 provider account adapter 一致",
        ));
    }
    let execution_mode =
        normalize_gateway_execution_mode(&input.adapter, input.execution_mode.as_deref())?;
    let endpoint_execution_modes =
        normalize_endpoint_execution_modes(&input.adapter, input.endpoint_execution_modes.clone())?;
    if execution_mode == ProviderExecutionMode::BrowserBacked
        && !adapter_supports_browser_backed_execution(&input.adapter)
    {
        return Err(GatewayError::conflict(format!(
            "{} 当前不支持 executionMode=browser_backed",
            input.adapter
        )));
    }
    if endpoint_execution_modes.as_ref().is_some_and(|values| {
        values
            .values()
            .any(|mode| *mode == ProviderExecutionMode::BrowserBacked)
    }) && !adapter_supports_browser_backed_execution(&input.adapter)
    {
        return Err(GatewayError::conflict(format!(
            "{} 当前不支持 endpointExecutionModes.*=browser_backed",
            input.adapter
        )));
    }
    Ok(())
}

fn resolve_protocol_profile(
    protocol_profile: Option<&str>,
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    if base_url
        .is_some_and(crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_base_url)
    {
        return crate::protocol::chatgpt::official_api::CHATGPT_CODEX_BACKEND_PROFILE.to_string();
    }
    protocol_profile
        .map(str::trim)
        .filter(|value| !value.trim().is_empty())
        .map(|value| infer_protocol_profile(adapter, Some(value), base_url))
        .unwrap_or_else(|| infer_protocol_profile(adapter, provider_hint, base_url))
}

fn resolve_protocol_family(
    protocol_family: &str,
    protocol_profile: Option<&str>,
    adapter: &str,
    provider_hint: Option<&str>,
    base_url: Option<&str>,
) -> String {
    infer_protocol_family(
        Some(protocol_family),
        adapter,
        protocol_profile.or(provider_hint),
        base_url,
    )
}

fn payload_base_url(payload: &Value) -> Option<&str> {
    payload
        .get("baseUrl")
        .or_else(|| payload.get("base_url"))
        .and_then(|value| value.as_str())
}

fn adapter_supports_browser_backed_execution(adapter: &str) -> bool {
    matches!(
        adapter.trim(),
        "lumalabs_compatible"
            | "gemini_canvas_compatible"
            | "gemini_canvas_web_reverse_compatible"
            | "gemini_canvas_program_web_reverse_compatible"
            | "aistudio_web_reverse_compatible"
            | "producer_compatible"
            | "suno_compatible"
            | "udio_compatible"
    )
}

fn default_execution_mode_for_adapter(adapter: &str) -> ProviderExecutionMode {
    match adapter.trim() {
        "lumalabs_compatible"
        | "gemini_canvas_compatible"
        | "gemini_canvas_web_reverse_compatible"
        | "gemini_canvas_program_web_reverse_compatible"
        | "aistudio_web_reverse_compatible"
        | "suno_compatible"
        | "udio_compatible" => ProviderExecutionMode::BrowserBacked,
        _ => ProviderExecutionMode::DirectHttp,
    }
}

#[cfg(test)]
mod qwen_tests {
    use super::*;

    #[test]
    fn resolve_protocol_profile_canonicalizes_qwen_webui_replay_aliases() {
        assert_eq!(
            resolve_protocol_profile(
                Some("qwen-web"),
                "qwen_web_compatible",
                Some("qwen-web"),
                Some("https://chat.qwen.ai"),
            ),
            "qwen_web_chat"
        );
        assert_eq!(
            resolve_protocol_profile(
                Some("qwen-webui"),
                "qwen_web_compatible",
                Some("qwen-webui"),
                Some("https://chat.qwen.ai"),
            ),
            "qwen_web_chat"
        );
        assert_eq!(
            resolve_protocol_profile(
                Some("qwen-webui-replay"),
                "qwen_web_compatible",
                Some("qwen-webui-replay"),
                Some("https://chat.qwen.ai"),
            ),
            "qwen_web_chat"
        );
        assert_eq!(
            resolve_protocol_profile(
                Some("qwen-webui-replay-live"),
                "qwen_web_compatible",
                Some("qwen-webui-replay-live"),
                Some("https://chat.qwen.ai"),
            ),
            "qwen_web_chat"
        );
    }

    #[test]
    fn resolve_protocol_profile_infers_qwen_official_and_web_lines() {
        assert_eq!(
            resolve_protocol_profile(
                None,
                "openai_compatible",
                Some("qwen"),
                Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
            ),
            "qwen_dashscope_openai"
        );
        assert_eq!(
            resolve_protocol_profile(
                None,
                "openai_compatible",
                Some("qwen-coding-plan-openai"),
                Some("https://coding.dashscope.aliyuncs.com/v1"),
            ),
            "qwen_coding_plan_openai"
        );
        assert_eq!(
            resolve_protocol_profile(
                None,
                "anthropic_compatible",
                Some("qwen-coding-plan-anthropic"),
                Some("https://coding.dashscope.aliyuncs.com/apps/anthropic"),
            ),
            "qwen_coding_plan_anthropic"
        );
        assert_eq!(
            resolve_protocol_profile(
                None,
                "qwen_web_compatible",
                Some("qwen-web-chat"),
                Some("https://chat.qwen.ai"),
            ),
            "qwen_web_chat"
        );
    }
}

fn normalize_gateway_execution_mode(
    adapter: &str,
    execution_mode: Option<&str>,
) -> Result<ProviderExecutionMode, GatewayError> {
    match execution_mode.map(|value| value.trim().to_lowercase()) {
        Some(value) if value == "direct_http" => Ok(ProviderExecutionMode::DirectHttp),
        Some(value) if value == "browser_backed" => {
            if !adapter_supports_browser_backed_execution(adapter) {
                Err(GatewayError::conflict(format!(
                    "{adapter} 当前不支持 executionMode=browser_backed"
                )))
            } else {
                Ok(ProviderExecutionMode::BrowserBacked)
            }
        }
        Some(_) => Err(GatewayError::bad_request("executionMode 不合法")),
        None => Ok(default_execution_mode_for_adapter(adapter)),
    }
}

fn normalize_endpoint_execution_modes(
    adapter: &str,
    endpoint_execution_modes: Option<HashMap<String, String>>,
) -> Result<Option<HashMap<String, ProviderExecutionMode>>, GatewayError> {
    let mut normalized = HashMap::new();
    for (endpoint_kind, mode) in endpoint_execution_modes.unwrap_or_default() {
        let endpoint_kind = endpoint_kind.trim().to_lowercase();
        if endpoint_kind.is_empty() {
            continue;
        }
        normalized.insert(
            endpoint_kind,
            normalize_gateway_execution_mode(adapter, Some(mode.as_str()))?,
        );
    }
    if adapter.trim() == "producer_compatible" {
        normalized
            .entry("videos_generations".to_string())
            .or_insert(ProviderExecutionMode::BrowserBacked);
    }
    Ok((!normalized.is_empty()).then_some(normalized))
}

fn parse_execution_mode(value: &str) -> Result<ProviderExecutionMode, GatewayError> {
    match value.trim().to_lowercase().as_str() {
        "direct_http" => Ok(ProviderExecutionMode::DirectHttp),
        "browser_backed" => Ok(ProviderExecutionMode::BrowserBacked),
        _ => Err(GatewayError::server_error("provider execution mode 非法")),
    }
}

fn execution_mode_string(mode: ProviderExecutionMode) -> &'static str {
    match mode {
        ProviderExecutionMode::DirectHttp => "direct_http",
        ProviderExecutionMode::BrowserBacked => "browser_backed",
    }
}

fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.len() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_len).collect())
}

fn normalize_service_provider_identity(
    provider_label: &str,
    service_provider_key: &Option<String>,
    service_provider_label: &Option<String>,
) -> Result<(String, String), GatewayError> {
    let normalized_label = normalize_optional_text(service_provider_label.as_deref(), 120)
        .unwrap_or_else(|| provider_label.trim().to_string());
    if normalized_label.is_empty() {
        return Err(GatewayError::bad_request("serviceProviderLabel 不能为空"));
    }
    let normalized_key = match service_provider_key.as_deref() {
        Some(value) => normalize_service_provider_key(value)?,
        None => derive_service_provider_key_from_label(&normalized_label),
    };
    Ok((normalized_key, normalized_label))
}

fn normalize_service_provider_key(value: &str) -> Result<String, GatewayError> {
    let normalized = sanitize_service_provider_key(value);
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(
            "serviceProviderKey 只允许字母、数字与分隔符，且归一化后不能为空",
        ));
    }
    if normalized.len() > 120 {
        return Err(GatewayError::bad_request(
            "serviceProviderKey 不能超过 120 个字符",
        ));
    }
    Ok(normalized)
}

fn derive_service_provider_key_from_label(label: &str) -> String {
    let normalized = sanitize_service_provider_key(label);
    if !normalized.is_empty() {
        return normalized;
    }
    format!("sp_{}", stable_service_provider_hash(label))
}

fn sanitize_service_provider_key(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;
    for ch in value.trim().chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            normalized.push(lowered);
            previous_was_separator = false;
            continue;
        }
        if matches!(ch, ' ' | '-' | '_' | '/' | '.' | ':') && !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    normalized.trim_matches('_').chars().take(120).collect()
}

fn stable_service_provider_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_protocol_profile_reconciles_legacy_openai_with_codex_backend_url() {
        assert_eq!(
            resolve_protocol_profile(
                Some("openai"),
                "openai_compatible",
                None,
                Some("https://chatgpt.com/backend-api/codex"),
            ),
            "chatgpt_codex_backend"
        );
    }

    #[test]
    fn resolve_protocol_profile_reconciles_legacy_codex_with_official_api_url() {
        assert_eq!(
            resolve_protocol_profile(
                Some("codex"),
                "openai_compatible",
                None,
                Some("https://api.openai.com/v1"),
            ),
            "chatgpt_official_api"
        );
    }
}
