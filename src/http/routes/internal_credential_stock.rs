use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::Value;

use crate::credential_stock::{
    get_credential_stock_status_report, list_credential_stock_policies,
    sweep_credential_stock_signals_once, upsert_credential_stock_policy,
    CredentialStockStatusFilters, UpsertCredentialStockPolicyInput,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

pub async fn list_credential_stock_status(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CredentialStockStatusFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let report = get_credential_stock_status_report(pg_pool, &state.redis_pool, query).await?;
    Ok(Json(serde_json::json!(report)))
}

pub async fn list_credential_stock_policies_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CredentialStockStatusFilters>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let policies =
        list_credential_stock_policies(required_pg_pool(state.as_ref())?, &query).await?;
    Ok(Json(serde_json::json!({ "policies": policies })))
}

pub async fn upsert_credential_stock_policy_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<UpsertCredentialStockPolicyInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let policy = upsert_credential_stock_policy(required_pg_pool(state.as_ref())?, body).await?;
    Ok(Json(serde_json::json!({ "policy": policy })))
}

pub async fn sweep_credential_stock_signals_route(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let result = sweep_credential_stock_signals_once(state.as_ref()).await?;
    Ok(Json(serde_json::json!(result)))
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
