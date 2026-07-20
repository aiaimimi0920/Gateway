use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use rquest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use crate::db;
use crate::db::map_db_error;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::implementation_lines;
use crate::provider_credential_folder_sync;
use crate::provider_quota;
use crate::provider_runtime;
use crate::redis::provider_sync;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use crate::upstream::headers::build_upstream_headers;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccountBody {
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
    pub endpoint_execution_modes: Option<std::collections::HashMap<String, String>>,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfileBody {
    pub source_kind: String,
    #[serde(default)]
    pub aggregator_api_mode: Option<String>,
    #[serde(default)]
    pub web_reverse_access_mode: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfilePatchBody {
    pub source_profile: ProviderSourceProfileBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPricingEntryBody {
    pub model: String,
    #[serde(default)]
    pub prompt_micros_per_1k_tokens: Option<i64>,
    #[serde(default)]
    pub completion_micros_per_1k_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPricingPatchBody {
    pub entries: Vec<ProviderModelPricingEntryBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelTieringSaveBody {
    pub model: String,
    pub platform_tier: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderModelTieringCardView {
    model: String,
    platform_tier: String,
    enabled: bool,
    source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderModelTieringView {
    provider_account_id: String,
    provider_label: String,
    models: Vec<ProviderModelTieringCardView>,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderCapabilityTieringRow {
    id: String,
    provider_account_id: String,
    model_code: String,
    endpoint_kind: String,
    upstream_model: Option<String>,
    enabled: bool,
}

#[derive(Debug, Clone, FromRow)]
struct PlatformAccessTieringRow {
    id: String,
    provider_capability_id: String,
    model_code: String,
    endpoint_kind: String,
    upstream_model: Option<String>,
    platform_tier: String,
    status: String,
    operator_weight: i32,
    routing_priority: i32,
    enabled_for_sale: bool,
    notes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfileBackfillBody {
    #[serde(default)]
    pub provider_account_ids: Option<Vec<String>>,
    #[serde(default)]
    pub only_missing: Option<bool>,
}

pub async fn list_provider_accounts(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_accounts = db::list_provider_accounts(required_pg_pool(state.as_ref())?).await?;
    Ok(Json(serde_json::json!({
        "providerAccounts": provider_accounts.into_iter().map(mask_provider_account_view).collect::<Vec<_>>(),
    })))
}

pub async fn get_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account_id.trim(),
            false,
        )
        .await?,
    })))
}

pub async fn get_provider_model_tiering(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account_id = provider_account_id.trim();
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let tiering = load_provider_model_tiering_view(state.as_ref(), &provider_account).await?;
    Ok(Json(serde_json::json!({
        "result": tiering,
    })))
}

pub async fn save_provider_model_tiering(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderModelTieringSaveBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account_id = provider_account_id.trim();
    let model = body.model.trim();
    if model.is_empty() {
        return Err(GatewayError::bad_request("模型不能为空"));
    }
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let payload = deserialize_provider_payload_for_model_tiering(&provider_account)?;
    let capabilities = load_provider_capability_tiering_rows(pg_pool, provider_account_id).await?;
    let access_rows = load_platform_access_tiering_rows(pg_pool, provider_account_id).await?;
    let matching_capabilities = capabilities
        .iter()
        .filter(|row| provider_model_key(&row.model_code, row.upstream_model.as_deref()) == model)
        .cloned()
        .collect::<Vec<_>>();
    let matching_access_rows = access_rows
        .iter()
        .filter(|row| provider_model_key(&row.model_code, row.upstream_model.as_deref()) == model)
        .cloned()
        .collect::<Vec<_>>();

    let endpoint_kind =
        preferred_endpoint_kind(&payload, &matching_capabilities, &matching_access_rows);

    let mut updated_capabilities = Vec::new();
    if matching_capabilities.is_empty() {
        updated_capabilities.push(
            db::save_provider_capability(
                pg_pool,
                &state.redis_pool,
                None,
                db::UpsertProviderCapabilityInput {
                    provider_account_id: provider_account_id.to_string(),
                    model_code: model.to_string(),
                    endpoint_kind: endpoint_kind.clone(),
                    upstream_model: Some(model.to_string()),
                    enabled: body.enabled,
                },
            )
            .await?,
        );
    } else {
        for capability in &matching_capabilities {
            updated_capabilities.push(
                db::save_provider_capability(
                    pg_pool,
                    &state.redis_pool,
                    Some(capability.id.as_str()),
                    db::UpsertProviderCapabilityInput {
                        provider_account_id: capability.provider_account_id.clone(),
                        model_code: capability.model_code.clone(),
                        endpoint_kind: capability.endpoint_kind.clone(),
                        upstream_model: capability.upstream_model.clone(),
                        enabled: body.enabled,
                    },
                )
                .await?,
            );
        }
    }

    if matching_access_rows.is_empty() {
        let capability = updated_capabilities
            .first()
            .ok_or_else(|| GatewayError::server_error("创建服务商模型能力失败"))?;
        db::save_platform_access(
            pg_pool,
            &state.redis_pool,
            None,
            db::UpsertPlatformAccessInput {
                provider_capability_id: capability.id.clone(),
                model_code: capability.model_code.clone(),
                endpoint_kind: capability.endpoint_kind.clone(),
                upstream_model: capability.upstream_model.clone(),
                platform_tier: body.platform_tier.clone(),
                status: if body.enabled {
                    "active".to_string()
                } else {
                    "disabled".to_string()
                },
                operator_weight: 100,
                routing_priority: 100,
                enabled_for_sale: body.enabled,
                notes: None,
            },
        )
        .await?;
    } else {
        for row in &matching_access_rows {
            db::save_platform_access(
                pg_pool,
                &state.redis_pool,
                Some(row.id.as_str()),
                db::UpsertPlatformAccessInput {
                    provider_capability_id: row.provider_capability_id.clone(),
                    model_code: row.model_code.clone(),
                    endpoint_kind: row.endpoint_kind.clone(),
                    upstream_model: row.upstream_model.clone(),
                    platform_tier: body.platform_tier.clone(),
                    status: if body.enabled {
                        "active".to_string()
                    } else {
                        "disabled".to_string()
                    },
                    operator_weight: row.operator_weight,
                    routing_priority: row.routing_priority,
                    enabled_for_sale: body.enabled,
                    notes: row.notes.clone(),
                },
            )
            .await?;
        }
    }

    let refreshed_provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let result =
        load_provider_model_tiering_view(state.as_ref(), &refreshed_provider_account).await?;
    Ok(Json(serde_json::json!({
        "result": result,
    })))
}

pub async fn create_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ProviderAccountBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    implementation_lines::assert_adapter_compiled(
        body.adapter.as_str(),
        &format!(
            "provider account create requested for adapter {}",
            body.adapter.trim()
        ),
    )?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    let provider_account =
        db::create_provider_account(required_pg_pool(state.as_ref())?, into_input(body)).await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        &provider_account.id,
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub async fn update_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderAccountBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    implementation_lines::assert_adapter_compiled(
        body.adapter.as_str(),
        &format!(
            "provider account update requested for adapter {}",
            body.adapter.trim()
        ),
    )?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    let provider_account = db::update_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
        into_input(body),
    )
    .await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        provider_account_id.trim(),
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub async fn delete_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account_id = provider_account_id.trim();
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let credentials = db::list_provider_credentials(pg_pool, Some(provider_account_id)).await?;

    for credential in &credentials {
        if credential.sync_mode == "folder_sync" {
            if let Some(source_path) = credential.source_path.as_deref() {
                let _ = provider_credential_folder_sync::delete_synced_credential_file(
                    state.as_ref(),
                    source_path,
                )?;
            }
        }
        db::delete_provider_credential(pg_pool, credential.id.as_str()).await?;
        provider_runtime::clear_provider_credential_runtime_keys(
            &state.redis_pool,
            credential.id.as_str(),
        )
        .await?;
    }

    db::delete_provider_account(pg_pool, provider_account_id).await?;
    provider_sync::delete_cached_provider_payload(&state.redis_pool, provider_account_id)
        .await
        .map_err(|error| GatewayError::server_error(format!("delete provider cache: {error}")))?;
    db::clear_provider_runtime_keys(&state.redis_pool, provider_account_id).await?;

    Ok(Json(serde_json::json!({
        "deleted": true,
        "providerAccountId": provider_account.id,
        "label": provider_account.label,
        "deletedCredentialCount": credentials.len(),
    })))
}

pub async fn patch_provider_source_profile(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderSourceProfilePatchBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let normalized = normalize_explicit_source_profile(body.source_profile)?;
    let existing = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let mut input = upsert_input_from_existing(&existing);
    input.source_kind = Some(normalized.source_kind);
    input.aggregator_api_mode = normalized.aggregator_api_mode;
    input.web_reverse_access_mode = normalized.web_reverse_access_mode;
    input.source_notes = normalized.notes;
    let provider_account = db::update_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
        input,
    )
    .await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        provider_account_id.trim(),
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub async fn patch_provider_model_pricing(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderModelPricingPatchBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let existing = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let mut input = upsert_input_from_existing(&existing);
    input.payload = merge_provider_model_pricing(existing.payload.clone(), body.entries)?;

    let provider_account = db::update_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
        input,
    )
    .await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        provider_account_id.trim(),
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;

    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub async fn backfill_provider_source_profiles(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ProviderSourceProfileBackfillBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account_ids = body
        .provider_account_ids
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let only_missing = body.only_missing.unwrap_or(true);
    let provider_accounts = db::list_provider_accounts(required_pg_pool(state.as_ref())?).await?;
    let mut updated_provider_accounts = Vec::new();
    let mut skipped_count = 0usize;
    let scoped_provider_accounts = provider_accounts
        .into_iter()
        .filter(|provider| {
            provider_account_ids.is_empty()
                || provider_account_ids
                    .iter()
                    .any(|value| value == &provider.id)
        })
        .collect::<Vec<_>>();

    for existing in scoped_provider_accounts.iter() {
        if only_missing && existing.source_kind.is_some() {
            skipped_count += 1;
            continue;
        }
        let inferred = infer_source_profile(existing);
        let mut input = upsert_input_from_existing(existing);
        input.source_kind = Some(inferred.source_kind);
        input.aggregator_api_mode = inferred.aggregator_api_mode;
        input.web_reverse_access_mode = inferred.web_reverse_access_mode;
        input.source_notes = inferred.notes;
        let updated = db::update_provider_account(
            required_pg_pool(state.as_ref())?,
            existing.id.as_str(),
            input,
        )
        .await?;
        provider_sync::set_cached_provider_payload(
            &state.redis_pool,
            existing.id.as_str(),
            &updated.payload,
            None,
        )
        .await
        .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
        updated_provider_accounts.push(updated);
    }

    Ok(Json(serde_json::json!({
        "result": {
            "scannedCount": scoped_provider_accounts.len(),
            "updatedCount": updated_provider_accounts.len(),
            "skippedCount": skipped_count,
            "providerAccounts": updated_provider_accounts
                .into_iter()
                .map(mask_provider_account_view)
                .collect::<Vec<_>>(),
        }
    })))
}

pub async fn get_provider_inventory(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let inventory = db::get_provider_inventory(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "inventory": mask_provider_inventory_view(inventory),
    })))
}

pub async fn get_provider_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    Ok(Json(serde_json::json!({
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account.id.as_str(),
            false,
        )
        .await?,
    })))
}

pub async fn refresh_provider_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    Ok(Json(serde_json::json!({
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account.id.as_str(),
            true,
        )
        .await?,
    })))
}

pub async fn probe_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let outcome =
        provider_runtime::probe_provider_account_for_management(&state, provider_account_id.trim())
            .await?;

    Ok(Json(serde_json::json!({
        "result": {
            "ok": outcome.ok,
            "providerAccount": mask_provider_account_view(outcome.provider_account),
            "errorMessage": outcome.error_message,
            "providerQuota": outcome.provider_quota,
        },
    })))
}

pub async fn sweep_cooling_provider_accounts(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let outcomes = provider_runtime::sweep_cooling_provider_accounts(&state, 50).await?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let inventory = db::get_provider_inventory(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
    )
    .await?;
    Ok(Json(serde_json::json!({
      "sweep": {
          "scanned": outcomes.len(),
          "reactivated": outcomes.iter().filter(|outcome| outcome.ok).count(),
          "extendedCooling": outcomes.iter().filter(|outcome| !outcome.ok).count(),
          "results": outcomes.into_iter().map(|outcome| serde_json::json!({
              "ok": outcome.ok,
              "providerAccount": mask_provider_account_view(outcome.provider_account),
              "errorMessage": outcome.error_message,
              "providerQuota": outcome.provider_quota,
          })).collect::<Vec<_>>(),
      },
        "providerInventory": mask_provider_inventory_view(inventory),
    })))
}

async fn load_provider_model_tiering_view(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
) -> Result<ProviderModelTieringView, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let capabilities =
        load_provider_capability_tiering_rows(pg_pool, provider_account.id.as_str()).await?;
    let access_rows =
        load_platform_access_tiering_rows(pg_pool, provider_account.id.as_str()).await?;
    let payload = deserialize_provider_payload_for_model_tiering(provider_account)?;

    let mut model_sources = BTreeMap::<String, String>::new();
    let (discovered_models, discovered_source) =
        discover_provider_models(state, provider_account, &payload, &capabilities).await;
    for model in discovered_models {
        model_sources.insert(model, discovered_source.clone());
    }
    for capability in &capabilities {
        let model =
            provider_model_key(&capability.model_code, capability.upstream_model.as_deref());
        if !model.is_empty() {
            model_sources
                .entry(model)
                .or_insert_with(|| "capability_catalog".to_string());
        }
    }

    let mut cards = Vec::new();
    for (model, source) in model_sources {
        let matching_capabilities = capabilities
            .iter()
            .filter(|row| {
                provider_model_key(&row.model_code, row.upstream_model.as_deref()) == model
            })
            .collect::<Vec<_>>();
        let matching_access_rows = access_rows
            .iter()
            .filter(|row| {
                provider_model_key(&row.model_code, row.upstream_model.as_deref()) == model
            })
            .collect::<Vec<_>>();
        let platform_tier = matching_access_rows
            .iter()
            .find(|row| row.enabled_for_sale && row.status == "active")
            .map(|row| row.platform_tier.clone())
            .or_else(|| {
                matching_access_rows
                    .first()
                    .map(|row| row.platform_tier.clone())
            })
            .map(|value| normalize_platform_tier_value(value.as_str()))
            .unwrap_or_else(|| "mid".to_string());
        let capability_enabled = matching_capabilities.iter().any(|row| row.enabled);
        let access_enabled = if matching_access_rows.is_empty() {
            capability_enabled
        } else {
            matching_access_rows.iter().any(|row| {
                row.enabled_for_sale && row.status != "disabled" && row.status != "archived"
            })
        };
        cards.push(ProviderModelTieringCardView {
            model,
            platform_tier,
            enabled: capability_enabled && access_enabled,
            source,
        });
    }

    Ok(ProviderModelTieringView {
        provider_account_id: provider_account.id.clone(),
        provider_label: provider_account.label.clone(),
        models: cards,
    })
}

pub(crate) fn make_provider_payload_serde_compatible(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("adapter") {
            object.insert(
                "adapter".to_string(),
                Value::String(provider_account.adapter.clone()),
            );
        }
        if !object.contains_key("baseUrl") && !object.contains_key("base_url") {
            object.insert("baseUrl".to_string(), Value::String(String::new()));
        }
        if !object.contains_key("apiKey") && !object.contains_key("api_key") {
            object.insert("apiKey".to_string(), Value::String(String::new()));
        }
    }
    payload
}

fn deserialize_provider_payload_for_model_tiering(
    provider_account: &db::GatewayProviderAccountView,
) -> Result<ProviderAccountPayload, GatewayError> {
    let payload =
        make_provider_payload_serde_compatible(provider_account, &provider_account.payload);
    serde_json::from_value(payload).map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for model tiering {}: {error}",
            provider_account.id
        ))
    })
}

async fn discover_provider_models(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
    payload: &ProviderAccountPayload,
    capabilities: &[ProviderCapabilityTieringRow],
) -> (Vec<String>, String) {
    if let Some(models) = provider_runtime::fixed_models_for_payload(payload) {
        let normalized = normalize_model_names(models);
        if !normalized.is_empty() {
            return (normalized, "fixed_models".to_string());
        }
    }

    if payload.canonical_adapter() == "accio_compatible" {
        let accio_models = fetch_accio_models_for_provider(state, provider_account)
            .await
            .unwrap_or_default();
        if !accio_models.is_empty() {
            return (accio_models, "accio_catalog_live".to_string());
        }
    }

    let upstream_models = fetch_provider_models_from_upstream(state, provider_account, payload)
        .await
        .unwrap_or_default();
    if !upstream_models.is_empty() {
        return (upstream_models, "upstream_models".to_string());
    }

    let configured_models = read_configured_supported_models(&provider_account.payload);
    if !configured_models.is_empty() {
        return (configured_models, "configured_supported_models".to_string());
    }

    let capability_models = normalize_model_names(
        capabilities
            .iter()
            .map(|row| provider_model_key(&row.model_code, row.upstream_model.as_deref()))
            .collect(),
    );
    if !capability_models.is_empty() {
        return (capability_models, "capability_catalog".to_string());
    }

    let default_model = read_default_model_from_payload(&provider_account.payload, payload)
        .map(|model| vec![model])
        .unwrap_or_default();
    if !default_model.is_empty() {
        return (default_model, "default_model".to_string());
    }

    (Vec::new(), "capability_catalog".to_string())
}

async fn fetch_provider_models_from_upstream(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
    payload: &ProviderAccountPayload,
) -> Result<Vec<String>, GatewayError> {
    let base_url = payload.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Ok(Vec::new());
    }
    let configured_path = provider_account
        .payload
        .get("modelsPath")
        .or_else(|| provider_account.payload.get("models_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let path = configured_path.or_else(|| match payload.canonical_adapter() {
        "openai_compatible" | "anthropic_compatible" => Some("/models".to_string()),
        "chatgpt_web_reverse_compatible" => {
            Some(crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_MODELS_PATH.to_string())
        }
        _ => None,
    });
    let Some(path) = path else {
        return Ok(Vec::new());
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build provider models client: {error}"))
        })?;
    let url = build_absolute_url(base_url, path.as_str());
    let response = client
        .get(url)
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("fetch provider models failed: {error}"))
        })?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    let body = response.json::<Value>().await.map_err(|error| {
        GatewayError::service_unavailable(format!("decode provider models failed: {error}"))
    })?;
    Ok(extract_model_names_from_value(&body))
}

async fn fetch_accio_models_for_provider(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
) -> Result<Vec<String>, GatewayError> {
    implementation_lines::assert_accio_web_reverse_api_compiled(
        "Accio model catalog probe requested",
    )?;
    let mut merged_payloads = Vec::new();
    if let Some(pg_pool) = state.pg_pool.as_ref() {
        let provider_credentials = db::list_active_provider_credentials_for_accounts(
            pg_pool,
            &[provider_account.id.clone()],
        )
        .await?;
        for credential in provider_credentials {
            merged_payloads.push(db::merge_provider_account_and_credential_payloads(
                &provider_account.payload,
                &credential.payload,
            ));
        }
    }
    if merged_payloads.is_empty() {
        merged_payloads.push(provider_account.payload.clone());
    }

    let mut models = Vec::new();
    for merged_payload in merged_payloads {
        let Ok(payload) = serde_json::from_value::<ProviderAccountPayload>(merged_payload.clone())
        else {
            continue;
        };
        let fetched = fetch_accio_models_from_upstream(state, &payload, &merged_payload)
            .await
            .unwrap_or_default();
        if fetched.is_empty() {
            let configured = filter_model_names_by_payload_constraints(
                &merged_payload,
                read_configured_supported_models(&merged_payload),
            );
            if !configured.is_empty() {
                models.extend(configured);
            }
            continue;
        }
        models.extend(filter_model_names_by_payload_constraints(
            &merged_payload,
            fetched,
        ));
    }

    Ok(normalize_model_names(models))
}

async fn fetch_accio_models_from_upstream(
    state: &AppState,
    payload: &ProviderAccountPayload,
    raw_payload: &Value,
) -> Result<Vec<String>, GatewayError> {
    let base_url = payload.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Ok(Vec::new());
    }
    let token = payload.api_key.trim().to_string();
    if token.is_empty() {
        return Ok(Vec::new());
    }
    let path = raw_payload
        .get("modelsPath")
        .or_else(|| raw_payload.get("models_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/api/llm/config");

    let client = Client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| {
            GatewayError::server_error(format!("build accio catalog client: {error}"))
        })?;
    let response = client
        .post(build_absolute_url(base_url, path))
        .json(&serde_json::json!({ "token": token }))
        .headers(build_accio_control_plane_headers(payload, false))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("fetch accio catalog failed: {error}"))
        })?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }
    let body = response.json::<Value>().await.map_err(|error| {
        GatewayError::service_unavailable(format!("decode accio catalog failed: {error}"))
    })?;
    Ok(extract_accio_catalog_model_names(&body))
}

fn build_accio_control_plane_headers(
    payload: &ProviderAccountPayload,
    quota_request: bool,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "accept",
        HeaderValue::from_static(if quota_request {
            "*/*"
        } else {
            "application/json"
        }),
    );
    headers.insert("user-agent", HeaderValue::from_static("node"));
    headers.insert("x-language", HeaderValue::from_static("zh"));
    headers.insert("x-os", HeaderValue::from_static("win32"));
    if quota_request {
        headers.insert("accept-language", HeaderValue::from_static("*"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
    }
    let version = payload
        .headers
        .get("x-app-version")
        .or_else(|| payload.headers.get("version"))
        .map(String::as_str)
        .unwrap_or("0.5.6");
    insert_control_header(&mut headers, "x-app-version", version);

    if let Some(utdid) = payload
        .headers
        .get("x-utdid")
        .or_else(|| payload.headers.get("utdid"))
        .map(String::as_str)
    {
        insert_control_header(&mut headers, "x-utdid", utdid);
    }
    if let Some(cna) = payload
        .headers
        .get("x-cna")
        .map(String::as_str)
        .or_else(|| {
            payload
                .headers
                .get("Cookie")
                .or_else(|| payload.headers.get("cookie"))
                .and_then(|value| extract_cookie_value(value, "cna"))
        })
    {
        insert_control_header(&mut headers, "x-cna", cna);
    }
    headers
}

fn insert_control_header(headers: &mut HeaderMap, name: &str, value: &str) {
    let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(header_value) = HeaderValue::from_str(value) else {
        return;
    };
    headers.insert(header_name, header_value);
}

fn extract_cookie_value<'a>(cookie_header: &'a str, cookie_name: &str) -> Option<&'a str> {
    cookie_header
        .split(';')
        .filter_map(|segment| segment.split_once('='))
        .find_map(|(name, value)| {
            if name.trim().eq_ignore_ascii_case(cookie_name) {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then_some(trimmed)
            } else {
                None
            }
        })
}

fn extract_model_names_from_value(value: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(array) = value.as_array() {
        for item in array {
            push_model_name(item, &mut names);
        }
    }
    for key in ["data", "models", "items", "modelList", "model_list"] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            for item in array {
                push_model_name(item, &mut names);
            }
        }
    }
    if let Some(result) = value.get("result") {
        names.extend(extract_model_names_from_value(result));
    }
    normalize_model_names(names)
}

fn push_model_name(value: &Value, names: &mut Vec<String>) {
    if let Some(model) = value.as_str() {
        names.push(model.to_string());
        return;
    }
    if let Some(object) = value.as_object() {
        for key in ["id", "model", "name", "modelName"] {
            if let Some(model) = object.get(key).and_then(Value::as_str) {
                names.push(model.to_string());
                return;
            }
        }
    }
}

fn extract_accio_catalog_model_names(value: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        for provider in data {
            if let Some(model_list) = provider.get("modelList").and_then(Value::as_array) {
                for item in model_list {
                    push_model_name(item, &mut names);
                }
            }
        }
    }
    if names.is_empty() {
        return extract_model_names_from_value(value);
    }
    normalize_model_names(names)
}

fn read_configured_supported_models(payload: &Value) -> Vec<String> {
    let values = payload
        .get("supportedModels")
        .or_else(|| payload.get("supported_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_model_names(values)
}

fn read_allowed_models(payload: &Value) -> Vec<String> {
    let values = payload
        .get("allowedModels")
        .or_else(|| payload.get("allowed_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_model_names(values)
}

fn read_excluded_models(payload: &Value) -> Vec<String> {
    let mut values = payload
        .get("excludedModels")
        .or_else(|| payload.get("excluded_models"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(disabled) = payload
        .get("disabledModels")
        .or_else(|| payload.get("disabled_models"))
        .and_then(Value::as_object)
    {
        values.extend(disabled.keys().cloned());
    }
    normalize_model_names(values)
}

fn filter_model_names_by_payload_constraints(payload: &Value, models: Vec<String>) -> Vec<String> {
    let supported = read_configured_supported_models(payload);
    let allowed = read_allowed_models(payload);
    let excluded = read_excluded_models(payload)
        .into_iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();

    let supported_set = (!supported.is_empty()).then(|| {
        supported
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>()
    });
    let allowed_set = (!allowed.is_empty()).then(|| {
        allowed
            .into_iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<std::collections::HashSet<_>>()
    });

    normalize_model_names(
        models
            .into_iter()
            .filter(|model| {
                let key = model.to_ascii_lowercase();
                if excluded.contains(&key) {
                    return false;
                }
                if let Some(supported_set) = supported_set.as_ref() {
                    if !supported_set.contains(&key) {
                        return false;
                    }
                }
                if let Some(allowed_set) = allowed_set.as_ref() {
                    if !allowed_set.contains(&key) {
                        return false;
                    }
                }
                true
            })
            .collect(),
    )
}

fn read_default_model_from_payload(
    raw_payload: &Value,
    payload: &ProviderAccountPayload,
) -> Option<String> {
    raw_payload
        .get("defaultModel")
        .or_else(|| raw_payload.get("default_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| payload.default_model.clone())
}

fn provider_model_key(model_code: &str, upstream_model: Option<&str>) -> String {
    upstream_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| model_code.trim())
        .to_string()
}

fn normalize_model_names(models: Vec<String>) -> Vec<String> {
    let mut map = BTreeMap::<String, String>::new();
    for model in models {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        map.entry(key).or_insert_with(|| trimmed.to_string());
    }
    map.into_values().collect()
}

fn preferred_endpoint_kind(
    payload: &ProviderAccountPayload,
    capabilities: &[ProviderCapabilityTieringRow],
    access_rows: &[PlatformAccessTieringRow],
) -> String {
    if let Some(row) = access_rows.first() {
        return row.endpoint_kind.clone();
    }
    if let Some(row) = capabilities.first() {
        return row.endpoint_kind.clone();
    }
    if payload.responses_path.is_some() {
        return "responses".to_string();
    }
    if payload.messages_path.is_some() || payload.canonical_adapter() == "anthropic_compatible" {
        return "messages".to_string();
    }
    if payload.chat_completions_path.is_some() {
        return "chat_completions".to_string();
    }
    "chat_completions".to_string()
}

fn normalize_platform_tier_value(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => "high".to_string(),
        "medium" | "mid" => "mid".to_string(),
        _ => "low".to_string(),
    }
}

async fn load_provider_capability_tiering_rows(
    pool: &sqlx::PgPool,
    provider_account_id: &str,
) -> Result<Vec<ProviderCapabilityTieringRow>, GatewayError> {
    sqlx::query_as::<_, ProviderCapabilityTieringRow>(
        r#"
        select
          id,
          provider_account_id,
          model_code,
          endpoint_kind,
          upstream_model,
          enabled
        from gateway_provider_capability_catalog
        where provider_account_id = $1
        order by enabled desc, updated_at desc, model_code asc, endpoint_kind asc
        "#,
    )
    .bind(provider_account_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

async fn load_platform_access_tiering_rows(
    pool: &sqlx::PgPool,
    provider_account_id: &str,
) -> Result<Vec<PlatformAccessTieringRow>, GatewayError> {
    sqlx::query_as::<_, PlatformAccessTieringRow>(
        r#"
        select
          pac.id,
          pac.provider_capability_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where pc.provider_account_id = $1
        order by pac.enabled_for_sale desc, pac.updated_at desc, pac.model_code asc, pac.endpoint_kind asc
        "#,
    )
    .bind(provider_account_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

async fn load_provider_account_quota(
    state: &AppState,
    provider_account_id: &str,
    force_refresh: bool,
) -> Result<Option<provider_quota::GatewayProviderQuotaView>, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let provider_credentials = db::list_active_provider_credentials_for_accounts(
        pg_pool,
        &[provider_account_id.to_string()],
    )
    .await?;
    if provider_credentials.is_empty() {
        let payload = crate::routing::candidate::deserialize_provider_payload(
            &provider_account.payload,
            Some(&provider_account.adapter),
        )
        .map_err(|error| {
            GatewayError::server_error(format!(
                "deserialize provider payload for quota {}: {error}",
                provider_account.id
            ))
        })?;
        return if force_refresh {
            provider_quota::refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                &payload,
            )
            .await
        } else {
            Ok(provider_quota::get_or_refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                &payload,
            )
            .await)
        };
    }

    let mut snapshots = Vec::new();
    for provider_credential in provider_credentials {
        let merged_payload = db::merge_provider_account_and_credential_payloads(
            &provider_account.payload,
            &provider_credential.payload,
        );
        let payload = crate::routing::candidate::deserialize_provider_payload(
            &merged_payload,
            Some(&provider_account.adapter),
        )
        .map_err(|error| {
            GatewayError::server_error(format!(
                "deserialize provider payload for provider credential quota {}: {error}",
                provider_credential.id
            ))
        })?;
        let snapshot = if force_refresh {
            provider_quota::refresh_runtime_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                Some(provider_credential.id.as_str()),
                &payload,
            )
            .await?
        } else {
            provider_quota::get_or_refresh_runtime_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                Some(provider_credential.id.as_str()),
                &payload,
            )
            .await
        };
        if let Some(snapshot) = snapshot {
            snapshots.push(snapshot);
        }
    }
    Ok(provider_quota::aggregate_provider_quota_snapshots(
        provider_account.id.as_str(),
        &snapshots,
    ))
}

struct NormalizedSourceProfileInput {
    source_kind: String,
    aggregator_api_mode: Option<String>,
    web_reverse_access_mode: Option<String>,
    notes: Option<String>,
}

fn normalize_explicit_source_profile(
    input: ProviderSourceProfileBody,
) -> Result<NormalizedSourceProfileInput, GatewayError> {
    let source_kind = input.source_kind.trim().to_lowercase();
    if !matches!(
        source_kind.as_str(),
        "official_model_api" | "official_vendor_api" | "aggregator_api" | "web_reverse_api"
    ) {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.sourceKind 不合法。",
        ));
    }

    let aggregator_api_mode = input
        .aggregator_api_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);
    if aggregator_api_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "hosted_compute" | "upstream_forward"))
    {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.aggregatorApiMode 不合法。",
        ));
    }

    let web_reverse_access_mode = input
        .web_reverse_access_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);
    if web_reverse_access_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "direct_http_replay" | "browser_challenge"))
    {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.webReverseAccessMode 不合法。",
        ));
    }

    if source_kind != "aggregator_api" && aggregator_api_mode.is_some() {
        return Err(GatewayError::bad_request(
            "只有 aggregator_api 允许设置 aggregatorApiMode。",
        ));
    }
    if source_kind != "web_reverse_api" && web_reverse_access_mode.is_some() {
        return Err(GatewayError::bad_request(
            "只有 web_reverse_api 允许设置 webReverseAccessMode。",
        ));
    }

    let notes = input
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>());

    Ok(NormalizedSourceProfileInput {
        source_kind,
        aggregator_api_mode,
        web_reverse_access_mode,
        notes,
    })
}

fn upsert_input_from_existing(
    existing: &db::GatewayProviderAccountView,
) -> db::UpsertProviderAccountInput {
    db::UpsertProviderAccountInput {
        label: existing.label.clone(),
        service_provider_key: Some(existing.service_provider_key.clone()),
        service_provider_label: Some(existing.service_provider_label.clone()),
        adapter: existing.adapter.clone(),
        protocol_family: existing.protocol_family.clone(),
        protocol_profile: Some(existing.protocol_profile.clone()),
        status: Some(existing.status.clone()),
        source_kind: existing.source_kind.clone(),
        aggregator_api_mode: existing.aggregator_api_mode.clone(),
        web_reverse_access_mode: existing.web_reverse_access_mode.clone(),
        source_notes: existing.source_notes.clone(),
        execution_mode: Some(match existing.execution_mode {
            crate::routing::candidate::ProviderExecutionMode::DirectHttp => {
                "direct_http".to_string()
            }
            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => {
                "browser_backed".to_string()
            }
        }),
        endpoint_execution_modes: existing.endpoint_execution_modes.as_ref().map(|modes| {
            modes
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        match value {
                            crate::routing::candidate::ProviderExecutionMode::DirectHttp => {
                                "direct_http".to_string()
                            }
                            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => {
                                "browser_backed".to_string()
                            }
                        },
                    )
                })
                .collect::<HashMap<_, _>>()
        }),
        payload: existing.payload.clone(),
    }
}

fn infer_source_profile(existing: &db::GatewayProviderAccountView) -> NormalizedSourceProfileInput {
    if has_chatgpt_codex_backend_base_url(&existing.payload) {
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some("自动推导：ChatGPT site Codex backend special case".to_string()),
        };
    }

    let hostname = read_provider_hostname(&existing.payload);
    if adapter_belongs_to_web_reverse_source(existing.adapter.as_str()) {
        return NormalizedSourceProfileInput {
            source_kind: "web_reverse_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: Some(resolve_source_access_mode_from_execution(
                existing.execution_mode,
                existing.endpoint_execution_modes.as_ref(),
            )),
            notes: Some(format!("自动推导：{}", existing.adapter)),
        };
    }

    if matches!(
        existing.adapter.as_str(),
        "search_api_compatible"
            | "linkup_compatible"
            | "kiro_compatible"
            | "codex_cli"
            | "claude_code"
    ) {
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：{}", existing.adapter)),
        };
    }

    if hostname
        .as_deref()
        .is_some_and(is_known_official_vendor_host)
    {
        let host = hostname.unwrap_or_default();
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：official host {host}")),
        };
    }

    if hostname
        .as_deref()
        .is_some_and(is_known_hosted_aggregator_host)
    {
        let host = hostname.unwrap_or_default();
        return NormalizedSourceProfileInput {
            source_kind: "aggregator_api".to_string(),
            aggregator_api_mode: Some("hosted_compute".to_string()),
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：hosted aggregator {host}")),
        };
    }

    NormalizedSourceProfileInput {
        source_kind: "aggregator_api".to_string(),
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        notes: Some(match hostname {
            Some(host) => format!("自动推导：compatible upstream {host}"),
            None => "自动推导：generic compatible provider".to_string(),
        }),
    }
}

fn merge_provider_model_pricing(
    payload: Value,
    entries: Vec<ProviderModelPricingEntryBody>,
) -> Result<Value, GatewayError> {
    let mut payload_map = payload
        .as_object()
        .cloned()
        .ok_or_else(|| GatewayError::bad_request("Provider payload 必须是 JSON object"))?;

    let mut pricing_map = payload_map
        .remove("modelPricing")
        .or_else(|| payload_map.remove("model_pricing"))
        .or_else(|| payload_map.remove("pricingByModel"))
        .or_else(|| payload_map.remove("pricing_by_model"))
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    for entry in entries {
        let model = entry.model.trim();
        if model.is_empty() {
            continue;
        }

        let prompt = entry.prompt_micros_per_1k_tokens.map(|value| value.max(0));
        let completion = entry
            .completion_micros_per_1k_tokens
            .map(|value| value.max(0));

        if prompt.is_none() && completion.is_none() {
            pricing_map.remove(model);
            continue;
        }

        let mut rate = serde_json::Map::new();
        if let Some(prompt) = prompt {
            rate.insert(
                "staticInputMicrosPer1kTokens".to_string(),
                Value::Number(prompt.into()),
            );
        }
        if let Some(completion) = completion {
            rate.insert(
                "staticOutputMicrosPer1kTokens".to_string(),
                Value::Number(completion.into()),
            );
        }
        pricing_map.insert(model.to_string(), Value::Object(rate));
    }

    if pricing_map.is_empty() {
        payload_map.remove("modelPricing");
    } else {
        payload_map.insert("modelPricing".to_string(), Value::Object(pricing_map));
    }

    Ok(Value::Object(payload_map))
}

fn read_provider_hostname(payload: &Value) -> Option<String> {
    let base_url = read_provider_base_url(payload)?;
    let without_scheme = base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or(base_url)
        .trim();
    let host = without_scheme
        .split('@')
        .next_back()
        .unwrap_or(without_scheme)
        .split(':')
        .next()
        .unwrap_or(without_scheme)
        .trim()
        .to_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(host)
}

fn read_provider_base_url(payload: &Value) -> Option<&str> {
    let obj = payload.as_object()?;
    ["baseUrl", "base_url", "endpoint", "url"]
        .iter()
        .find_map(|key| obj.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn adapter_belongs_to_web_reverse_source(adapter: &str) -> bool {
    matches!(
        adapter,
        "accio_compatible"
            | "chatgpt_web_reverse_compatible"
            | "producer_compatible"
            | "gemini_business_compatible"
            | "chataibot_compatible"
            | "lumalabs_compatible"
            | "gemini_canvas_compatible"
            | "suno_compatible"
            | "udio_compatible"
    )
}

fn has_chatgpt_codex_backend_base_url(payload: &Value) -> bool {
    read_provider_base_url(payload)
        .is_some_and(crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_base_url)
}

fn resolve_source_access_mode_from_execution(
    execution_mode: crate::routing::candidate::ProviderExecutionMode,
    endpoint_execution_modes: Option<
        &HashMap<String, crate::routing::candidate::ProviderExecutionMode>,
    >,
) -> String {
    let has_browser_backed_endpoint = execution_mode
        == crate::routing::candidate::ProviderExecutionMode::BrowserBacked
        || endpoint_execution_modes
            .map(|modes| {
                modes.values().any(|mode| {
                    *mode == crate::routing::candidate::ProviderExecutionMode::BrowserBacked
                })
            })
            .unwrap_or(false);
    if has_browser_backed_endpoint {
        "browser_challenge".to_string()
    } else {
        "direct_http_replay".to_string()
    }
}

fn is_known_hosted_aggregator_host(hostname: &str) -> bool {
    hostname == "api.siliconflow.cn" || hostname == "ai.gitee.com"
}

fn is_known_official_vendor_host(hostname: &str) -> bool {
    [
        "openai.com",
        "anthropic.com",
        "x.ai",
        "groq.com",
        "googleapis.com",
        "generativelanguage.googleapis.com",
        "nvidia.com",
        "mistral.ai",
        "cohere.ai",
        "moonshot.cn",
        "zhipuai.cn",
        "linkup.so",
        "tavily.com",
        "exa.ai",
        "ydc-index.io",
        "websearchapi.ai",
        "jina.ai",
    ]
    .iter()
    .any(|suffix| hostname == *suffix || hostname.ends_with(&format!(".{suffix}")))
}

fn into_input(body: ProviderAccountBody) -> db::UpsertProviderAccountInput {
    db::UpsertProviderAccountInput {
        label: body.label,
        service_provider_key: body.service_provider_key,
        service_provider_label: body.service_provider_label,
        adapter: body.adapter,
        protocol_family: body.protocol_family,
        protocol_profile: body.protocol_profile,
        status: body.status,
        source_kind: body.source_kind,
        aggregator_api_mode: body.aggregator_api_mode,
        web_reverse_access_mode: body.web_reverse_access_mode,
        source_notes: body.source_notes,
        execution_mode: body.execution_mode,
        endpoint_execution_modes: body.endpoint_execution_modes,
        payload: body.payload,
    }
}

fn mask_provider_account_view(view: db::GatewayProviderAccountView) -> Value {
    serde_json::json!({
        "id": view.id,
        "label": view.label,
        "serviceProviderKey": view.service_provider_key,
        "serviceProviderLabel": view.service_provider_label,
        "adapter": view.adapter,
        "protocolFamily": view.protocol_family,
        "protocolProfile": view.protocol_profile,
        "status": view.status,
        "sourceProfile": {
            "sourceKind": view.source_kind.clone().unwrap_or_else(|| "unknown".to_string()),
            "aggregatorApiMode": view.aggregator_api_mode,
            "webReverseAccessMode": view.web_reverse_access_mode,
            "sourceNotes": view.source_notes,
            "derived": view.source_kind.is_none(),
        },
        "sourceKind": view.source_kind,
        "aggregatorApiMode": view.aggregator_api_mode,
        "webReverseAccessMode": view.web_reverse_access_mode,
        "sourceNotes": view.source_notes,
        "executionMode": view.execution_mode,
        "endpointExecutionModes": view.endpoint_execution_modes,
        "payload": mask_provider_payload_secrets(view.payload),
        "storageMode": view.storage_mode,
        "cooldownUntil": view.cooldown_until,
        "lastError": view.last_error,
        "failureCount": view.failure_count,
        "lastHealthCheckAt": view.last_health_check_at,
        "createdAt": view.created_at,
        "updatedAt": view.updated_at,
    })
}

fn mask_provider_inventory_view(view: db::GatewayProviderInventoryView) -> Value {
    serde_json::json!({
        "providers": view.providers.into_iter().map(|entry| {
            serde_json::json!({
                "providerAccount": mask_provider_account_view(entry.provider_account),
                "providerHealth": entry.provider_health,
                "costHints": entry.cost_hints,
                "providerQuota": entry.provider_quota,
            })
        }).collect::<Vec<_>>(),
        "summary": view.summary,
    })
}

fn mask_provider_payload_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let normalized = key.to_lowercase();
                    if matches!(
                        normalized.as_str(),
                        "apikey"
                            | "api_key"
                            | "apisecret"
                            | "api_secret"
                            | "auth_token"
                            | "authorization"
                            | "token"
                            | "cookie"
                    ) {
                        (key, mask_secret_value(value))
                    } else if normalized == "headers" {
                        (key, mask_header_map(value))
                    } else {
                        (key, mask_provider_payload_secrets(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(mask_provider_payload_secrets)
                .collect(),
        ),
        other => other,
    }
}

fn mask_header_map(value: Value) -> Value {
    match value {
        Value::Object(headers) => Value::Object(
            headers
                .into_iter()
                .map(|(key, value)| {
                    let normalized = key.to_lowercase();
                    if matches!(
                        normalized.as_str(),
                        "authorization" | "cookie" | "x-api-key" | "x-goog-api-key"
                    ) {
                        (key, mask_secret_value(value))
                    } else {
                        (key, value)
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

fn mask_secret_value(value: Value) -> Value {
    let Some(raw) = value.as_str().map(str::trim) else {
        return Value::String("***".to_string());
    };
    if raw.is_empty() {
        return Value::String(String::new());
    }
    if raw.len() <= 8 {
        return Value::String(format!("{}***", &raw[..raw.len().min(2)]));
    }
    Value::String(format!("{}***{}", &raw[..4], &raw[raw.len() - 2..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_provider_payload_for_model_tiering_backfills_missing_transport_fields() {
        let provider = db::GatewayProviderAccountView {
            id: "provider-without-auth".to_string(),
            label: "Provider Without Auth".to_string(),
            service_provider_key: "provider_without_auth".to_string(),
            service_provider_label: "Provider Without Auth".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            status: "active".to_string(),
            source_kind: None,
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: serde_json::json!({
                "defaultModel": "gpt-5.4-mini"
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-20T00:00:00Z".to_string(),
            updated_at: "2026-04-20T00:00:00Z".to_string(),
        };

        let payload = deserialize_provider_payload_for_model_tiering(&provider)
            .expect("tiering payload should accept missing apiKey");

        assert_eq!(payload.adapter, "openai_compatible");
        assert_eq!(payload.base_url, "");
        assert_eq!(payload.api_key, "");
        assert_eq!(payload.default_model.as_deref(), Some("gpt-5.4-mini"));
    }

    #[test]
    fn infer_codex_backend_as_official_vendor_api() {
        let provider = db::GatewayProviderAccountView {
            id: "codex-platform-provider".to_string(),
            label: "Codex Platform".to_string(),
            service_provider_key: "codex_platform".to_string(),
            service_provider_label: "Codex Platform".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "codex".to_string(),
            status: "active".to_string(),
            source_kind: None,
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: serde_json::json!({
                "adapter": "openai_compatible",
                "base_url": "https://chatgpt.com/backend-api/codex",
                "default_model": "gpt-5.4"
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-16T00:00:00Z".to_string(),
            updated_at: "2026-04-16T00:00:00Z".to_string(),
        };

        let inferred = infer_source_profile(&provider);
        assert_eq!(inferred.source_kind, "official_vendor_api");
        assert_eq!(inferred.web_reverse_access_mode, None);
    }

    #[test]
    fn infer_accio_as_web_reverse_direct_http_replay() {
        let provider = db::GatewayProviderAccountView {
            id: "accio-provider".to_string(),
            label: "Accio Live".to_string(),
            service_provider_key: "accio_platform".to_string(),
            service_provider_label: "Accio".to_string(),
            adapter: "accio_compatible".to_string(),
            protocol_family: "openai_responses".to_string(),
            protocol_profile: "accio".to_string(),
            status: "active".to_string(),
            source_kind: None,
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: serde_json::json!({
                "adapter": "accio_compatible",
                "base_url": "https://phoenix-gw.alibaba.com",
                "default_model": "claude-sonnet-4-6"
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-20T00:00:00Z".to_string(),
            updated_at: "2026-04-20T00:00:00Z".to_string(),
        };

        let inferred = infer_source_profile(&provider);
        assert_eq!(inferred.source_kind, "web_reverse_api");
        assert_eq!(
            inferred.web_reverse_access_mode.as_deref(),
            Some("direct_http_replay")
        );
    }

    #[test]
    fn extracts_accio_catalog_model_list_and_filters_disabled_entries() {
        let raw_catalog = serde_json::json!({
            "success": true,
            "data": [
                {
                    "provider": "claude",
                    "modelList": [
                        {"modelName": "claude-sonnet-4-6", "visible": true},
                        {"modelName": "claude-opus-4-6", "visible": true}
                    ]
                },
                {
                    "provider": "gemini",
                    "modelList": [
                        {"modelName": "gemini-3.1-pro-preview", "visible": true}
                    ]
                }
            ]
        });
        assert_eq!(
            extract_accio_catalog_model_names(&raw_catalog),
            vec![
                "claude-opus-4-6".to_string(),
                "claude-sonnet-4-6".to_string(),
                "gemini-3.1-pro-preview".to_string()
            ]
        );

        let filtered = filter_model_names_by_payload_constraints(
            &serde_json::json!({
                "disabledModels": {
                    "claude-opus-4-6": "quota_empty"
                }
            }),
            extract_accio_catalog_model_names(&raw_catalog),
        );
        assert_eq!(
            filtered,
            vec![
                "claude-sonnet-4-6".to_string(),
                "gemini-3.1-pro-preview".to_string()
            ]
        );
    }
}
