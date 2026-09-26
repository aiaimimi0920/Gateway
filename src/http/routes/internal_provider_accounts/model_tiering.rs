//! Management model-tiering views and capability/access update orchestration.

use super::super::internal_gateway::assert_management_access;
use super::model_catalog::provider_model_key;
use super::model_discovery::{
    deserialize_provider_payload_for_model_tiering, discover_provider_models,
};
use super::required_pg_pool;
use super::tiering_storage::{
    load_platform_access_tiering_rows, load_provider_capability_tiering_rows,
    PlatformAccessTieringRow, ProviderCapabilityTieringRow,
};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

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
