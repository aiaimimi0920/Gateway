//! Management remediation queue, execution and impact capture.

use super::super::internal_gateway::assert_management_access;
use super::{management_actor_user_id, required_pg_pool};
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
pub struct RemediationRunQuery {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub dry_run: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub window_minutes: Option<i32>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemediationQueueQuery {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub status: Option<String>,
    pub follow_up_status: Option<String>,
    pub escalation_status: Option<String>,
    pub code: Option<String>,
    pub severity: Option<String>,
    pub action_key: Option<String>,
    pub execution_mode: Option<String>,
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

pub async fn get_anomaly_incident_remediation_plan(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let plan = db::get_anomaly_incident_remediation_plan(
        required_pg_pool(state.as_ref())?,
        incident_id.trim(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "plan": plan,
    })))
}

pub async fn list_remediation_queue(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationQueueQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let queue = db::list_anomaly_remediation_queue(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &into_remediation_queue_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "queue": queue,
    })))
}

pub async fn sweep_remediation_runs(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RemediationQueueQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let sweep = db::sweep_anomaly_remediations(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        management_actor_user_id(&headers),
        &into_remediation_queue_filters(body),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "sweep": sweep,
    })))
}

pub async fn list_remediation_runs(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let runs = db::list_anomaly_incident_remediation_runs(
        required_pg_pool(state.as_ref())?,
        &into_remediation_run_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "runs": runs,
    })))
}

pub async fn list_incident_remediation_runs(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let mut filters = into_remediation_run_filters(query);
    filters.incident_id = Some(incident_id.trim().to_string());
    let runs =
        db::list_anomaly_incident_remediation_runs(required_pg_pool(state.as_ref())?, &filters)
            .await?;
    Ok(Json(serde_json::json!({
        "runs": runs,
    })))
}

pub async fn execute_incident_remediation_run(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Json(body): Json<db::ExecuteGatewayAnalysisAnomalyIncidentRemediationInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let run = db::execute_anomaly_incident_remediation(
        required_pg_pool(state.as_ref())?,
        management_actor_user_id(&headers),
        incident_id.trim(),
        body,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "run": run,
    })))
}

pub async fn summarize_remediation_runs(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_anomaly_incident_remediation_runs(
        required_pg_pool(state.as_ref())?,
        &into_remediation_run_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_remediation_run_impact(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let impact = db::get_anomaly_incident_remediation_run_impact(
        required_pg_pool(state.as_ref())?,
        run_id.trim(),
        query.window_minutes,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "impact": impact,
    })))
}

pub async fn capture_remediation_run_impact(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let capture = db::capture_anomaly_incident_remediation_run_impact(
        required_pg_pool(state.as_ref())?,
        management_actor_user_id(&headers),
        run_id.trim(),
        query.window_minutes,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "capture": capture,
    })))
}

pub(super) fn into_remediation_run_filters(
    query: RemediationRunQuery,
) -> db::GatewayAnalysisAnomalyRemediationRunFilters {
    db::GatewayAnalysisAnomalyRemediationRunFilters {
        incident_id: query.incident_id,
        policy_id: query.policy_id,
        route_policy_id: query.route_policy_id,
        action_key: query.action_key,
        status: query.status,
        execution_mode: query.execution_mode,
        dry_run: query.dry_run,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}

pub(super) fn into_remediation_queue_filters(
    query: RemediationQueueQuery,
) -> db::GatewayAnalysisAnomalyRemediationQueueFilters {
    db::GatewayAnalysisAnomalyRemediationQueueFilters {
        incident_id: query.incident_id,
        policy_id: query.policy_id,
        project_id: query.project_id,
        route_policy_id: query.route_policy_id,
        owner_user_id: query.owner_user_id,
        tag: query.tag,
        text_mode: query.text_mode,
        status: query.status,
        follow_up_status: query.follow_up_status,
        escalation_status: query.escalation_status,
        code: query.code,
        severity: query.severity,
        action_key: query.action_key,
        execution_mode: query.execution_mode,
        due_only: query.due_only,
        limit: query.limit,
    }
}
