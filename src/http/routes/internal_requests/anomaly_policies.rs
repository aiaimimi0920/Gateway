//! Management anomaly policy queries and synchronization.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyPolicyQuery {
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub auto_sync_enabled: Option<bool>,
    pub auto_escalate_enabled: Option<bool>,
    pub auto_remediation_enabled: Option<bool>,
    pub alerting_enabled: Option<bool>,
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

pub async fn list_anomaly_policies(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyPolicyQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let policies = db::list_anomaly_policies(
        required_pg_pool(state.as_ref())?,
        &into_policy_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "policies": policies,
    })))
}

pub async fn summarize_anomaly_policies(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyPolicyQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_anomaly_policies(
        required_pg_pool(state.as_ref())?,
        &into_policy_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn save_anomaly_policy(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<db::UpsertGatewayAnalysisAnomalyPolicyInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let policy = db::save_anomaly_policy(required_pg_pool(state.as_ref())?, body).await?;
    Ok(Json(serde_json::json!({
        "policy": policy,
    })))
}

pub async fn sync_anomaly_policy(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(policy_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let sync = db::sync_anomaly_policy(required_pg_pool(state.as_ref())?, policy_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "policy": sync.policy,
        "syncKind": sync.sync_kind,
        "anomalyCount": sync.anomaly_count,
        "openedIncidentCount": sync.opened_incident_count,
        "updatedIncidentCount": sync.updated_incident_count,
        "resolvedIncidentCount": sync.resolved_incident_count,
        "sync": sync.sync,
    })))
}

pub async fn sweep_anomaly_policies(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<AnomalyPolicyQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let sweep = db::sweep_anomaly_policies(
        required_pg_pool(state.as_ref())?,
        &into_policy_filters(body),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "sweep": sweep,
    })))
}

fn into_policy_filters(query: AnomalyPolicyQuery) -> db::GatewayAnalysisAnomalyPolicyFilters {
    db::GatewayAnalysisAnomalyPolicyFilters {
        policy_id: query.policy_id,
        project_id: query.project_id,
        route_policy_id: query.route_policy_id,
        status: query.status,
        tag: query.tag,
        text_mode: query.text_mode,
        auto_sync_enabled: query.auto_sync_enabled,
        auto_escalate_enabled: query.auto_escalate_enabled,
        auto_remediation_enabled: query.auto_remediation_enabled,
        alerting_enabled: query.alerting_enabled,
        due_only: query.due_only,
        limit: query.limit,
    }
}
