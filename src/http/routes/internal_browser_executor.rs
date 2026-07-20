use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::browser_executor_runtime;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseReleaseBody {
    #[serde(default)]
    pub release_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeasePath {
    pub lease_id: String,
}

pub async fn heartbeat_browser_executor_node(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<browser_executor_runtime::BrowserExecutorNodeHeartbeatInput>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let node =
        browser_executor_runtime::heartbeat_browser_executor_node(state.as_ref(), body).await?;
    Ok(Json(json!({ "node": node })))
}

pub async fn list_browser_executor_nodes(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let nodes = browser_executor_runtime::list_browser_executor_nodes(state.as_ref()).await?;
    Ok(Json(json!({ "nodes": nodes })))
}

pub async fn upsert_browser_capability_slot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<browser_executor_runtime::BrowserCapabilitySlotUpsertInput>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let slot =
        browser_executor_runtime::upsert_browser_capability_slot(state.as_ref(), body).await?;
    Ok(Json(json!({ "slot": slot })))
}

pub async fn list_browser_capability_slots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<browser_executor_runtime::BrowserCapabilitySlotFilters>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let slots =
        browser_executor_runtime::list_browser_capability_slots(state.as_ref(), query).await?;
    Ok(Json(json!({ "slots": slots })))
}

pub async fn acquire_browser_capability_lease(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<browser_executor_runtime::BrowserCapabilityLeaseAcquireInput>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let lease =
        browser_executor_runtime::acquire_browser_capability_lease(state.as_ref(), body).await?;
    Ok(Json(json!({ "lease": lease })))
}

pub async fn release_browser_capability_lease(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(path): Path<LeasePath>,
    Json(body): Json<BrowserCapabilityLeaseReleaseBody>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let lease = browser_executor_runtime::release_browser_capability_lease(
        state.as_ref(),
        browser_executor_runtime::BrowserCapabilityLeaseReleaseInput {
            lease_id: path.lease_id,
            release_reason: body.release_reason,
        },
    )
    .await?;
    Ok(Json(json!({ "lease": lease })))
}

pub async fn list_browser_capability_leases(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<browser_executor_runtime::BrowserCapabilityLeaseFilters>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let leases =
        browser_executor_runtime::list_browser_capability_leases(state.as_ref(), query).await?;
    Ok(Json(json!({ "leases": leases })))
}

pub async fn get_browser_executor_management_health(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<browser_executor_runtime::BrowserExecutorHealthFilters>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let health =
        browser_executor_runtime::get_browser_executor_health(state.as_ref(), query).await?;
    Ok(Json(json!({ "health": health })))
}
