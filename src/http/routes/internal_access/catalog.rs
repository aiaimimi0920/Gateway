//! Management provider-capability and platform-access catalog mutations.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub include_tokens: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilityBody {
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAccessBody {
    pub provider_capability_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub status: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
    pub enabled_for_sale: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AccessPath {
    pub access_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessCatalogResponse {
    #[serde(flatten)]
    catalog: db::GatewayAccessCatalogView,
    // Match AccessStore's authority; a loopback URL does not imply local storage.
    storage_mode: &'static str,
    cash_quota_supported: bool,
    account_groups: Vec<AccessKeyGroupOption>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessKeyGroupOption {
    id: String,
    name: String,
    enabled: bool,
    member_count: usize,
}

pub async fn get_access_catalog(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(_query): Query<CatalogQuery>,
) -> Result<Json<AccessCatalogResponse>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let catalog = crate::access_store::AccessStore(&state).catalog().await?;
    Ok(Json(AccessCatalogResponse {
        cash_quota_supported: crate::access_balance::AccessBalanceStore::from_state(&state)?
            .cash_supported()
            .await?,
        catalog,
        account_groups: state
            .route_config
            .snapshot()
            .account_group_inventory()
            .account_groups
            .into_iter()
            .map(|group| AccessKeyGroupOption {
                id: group.id,
                name: group.name,
                enabled: group.enabled,
                member_count: group.member_count,
            })
            .collect(),
        storage_mode: if state.local_runtime.is_some() {
            "local"
        } else {
            "server"
        },
    }))
}

pub async fn create_provider_capability(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ProviderCapabilityBody>,
) -> Result<Json<db::GatewayProviderCapabilityView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_provider_capability(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            db::UpsertProviderCapabilityInput {
                provider_account_id: body.provider_account_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                enabled: body.enabled,
            },
        )
        .await?,
    ))
}

pub async fn update_provider_capability(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessPath>,
    Json(body): Json<ProviderCapabilityBody>,
) -> Result<Json<db::GatewayProviderCapabilityView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_provider_capability(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.access_id.as_str()),
            db::UpsertProviderCapabilityInput {
                provider_account_id: body.provider_account_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                enabled: body.enabled,
            },
        )
        .await?,
    ))
}

pub async fn create_platform_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<PlatformAccessBody>,
) -> Result<Json<db::GatewayPlatformAccessView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_platform_access(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            None,
            db::UpsertPlatformAccessInput {
                provider_capability_id: body.provider_capability_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                platform_tier: body.platform_tier,
                status: body.status,
                operator_weight: body.operator_weight,
                routing_priority: body.routing_priority,
                enabled_for_sale: body.enabled_for_sale,
                notes: body.notes,
            },
        )
        .await?,
    ))
}

pub async fn update_platform_access(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<AccessPath>,
    Json(body): Json<PlatformAccessBody>,
) -> Result<Json<db::GatewayPlatformAccessView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::save_platform_access(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            Some(path.access_id.as_str()),
            db::UpsertPlatformAccessInput {
                provider_capability_id: body.provider_capability_id,
                model_code: body.model_code,
                endpoint_kind: body.endpoint_kind,
                upstream_model: body.upstream_model,
                platform_tier: body.platform_tier,
                status: body.status,
                operator_weight: body.operator_weight,
                routing_priority: body.routing_priority,
                enabled_for_sale: body.enabled_for_sale,
                notes: body.notes,
            },
        )
        .await?,
    ))
}
