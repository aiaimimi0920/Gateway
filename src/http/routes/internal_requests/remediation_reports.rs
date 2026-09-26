//! Management remediation effectiveness reports and stored snapshots.

use super::super::internal_gateway::assert_management_access;
use super::remediation::{into_remediation_run_filters, RemediationRunQuery};
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
pub struct RemediationSnapshotQuery {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
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
    pub lookback_hours: Option<i32>,
    pub profile_key: Option<String>,
    pub impacted_run_rate_warning_threshold: Option<f64>,
    pub impacted_run_rate_critical_threshold: Option<f64>,
    pub unavailable_run_rate_warning_threshold: Option<f64>,
    pub unavailable_run_rate_critical_threshold: Option<f64>,
    pub completion_rate_regressed_warning_threshold: Option<f64>,
    pub completion_rate_regressed_critical_threshold: Option<f64>,
    pub failure_rate_regressed_warning_threshold: Option<f64>,
    pub failure_rate_regressed_critical_threshold: Option<f64>,
    pub request_artifact_regressed_warning_threshold: Option<f64>,
    pub request_artifact_regressed_critical_threshold: Option<f64>,
    pub response_artifact_regressed_warning_threshold: Option<f64>,
    pub response_artifact_regressed_critical_threshold: Option<f64>,
    pub first_token_latency_regressed_warning_threshold: Option<f64>,
    pub first_token_latency_regressed_critical_threshold: Option<f64>,
    pub total_tokens_regressed_warning_threshold: Option<f64>,
    pub total_tokens_regressed_critical_threshold: Option<f64>,
    pub limit: Option<usize>,
}

pub async fn summarize_remediation_effectiveness(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationRunQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let effectiveness = db::get_anomaly_remediation_effectiveness(
        required_pg_pool(state.as_ref())?,
        &into_remediation_run_filters(query.clone()),
        query.window_minutes,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "effectiveness": effectiveness,
    })))
}

pub async fn persist_remediation_effectiveness_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_snapshot_run_filters(&body);
    let snapshot = db::persist_anomaly_remediation_effectiveness_snapshot(
        required_pg_pool(state.as_ref())?,
        &filters,
        body.label.as_deref(),
        body.window_minutes,
        body.lookback_hours,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn list_remediation_effectiveness_snapshots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshots = db::list_anomaly_remediation_effectiveness_snapshots(
        &into_remediation_snapshot_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshots": snapshots,
    })))
}

pub async fn get_remediation_effectiveness_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(snapshot_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = db::get_anomaly_remediation_effectiveness_snapshot(snapshot_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn summarize_remediation_effectiveness_snapshots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_anomaly_remediation_effectiveness_snapshots(
        &into_remediation_snapshot_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_remediation_effectiveness_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_anomaly_remediation_effectiveness_trend_report(
        &into_remediation_snapshot_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_remediation_effectiveness_snapshot_anomaly_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_anomaly_remediation_effectiveness_snapshot_anomaly_report(
        &into_remediation_snapshot_filters(query.clone()),
        query.profile_key.as_deref(),
        db::GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides {
            impacted_run_rate_warning_threshold: query.impacted_run_rate_warning_threshold,
            impacted_run_rate_critical_threshold: query.impacted_run_rate_critical_threshold,
            unavailable_run_rate_warning_threshold: query.unavailable_run_rate_warning_threshold,
            unavailable_run_rate_critical_threshold: query.unavailable_run_rate_critical_threshold,
            completion_rate_regressed_warning_threshold: query
                .completion_rate_regressed_warning_threshold,
            completion_rate_regressed_critical_threshold: query
                .completion_rate_regressed_critical_threshold,
            failure_rate_regressed_warning_threshold: query
                .failure_rate_regressed_warning_threshold,
            failure_rate_regressed_critical_threshold: query
                .failure_rate_regressed_critical_threshold,
            request_artifact_regressed_warning_threshold: query
                .request_artifact_regressed_warning_threshold,
            request_artifact_regressed_critical_threshold: query
                .request_artifact_regressed_critical_threshold,
            response_artifact_regressed_warning_threshold: query
                .response_artifact_regressed_warning_threshold,
            response_artifact_regressed_critical_threshold: query
                .response_artifact_regressed_critical_threshold,
            first_token_latency_regressed_warning_threshold: query
                .first_token_latency_regressed_warning_threshold,
            first_token_latency_regressed_critical_threshold: query
                .first_token_latency_regressed_critical_threshold,
            total_tokens_regressed_warning_threshold: query
                .total_tokens_regressed_warning_threshold,
            total_tokens_regressed_critical_threshold: query
                .total_tokens_regressed_critical_threshold,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn list_remediation_effectiveness_anomaly_snapshots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshots = db::list_anomaly_remediation_effectiveness_anomaly_snapshots(
        &into_remediation_anomaly_snapshot_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshots": snapshots,
    })))
}

pub async fn get_remediation_effectiveness_anomaly_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(snapshot_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot =
        db::get_anomaly_remediation_effectiveness_anomaly_snapshot(snapshot_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn persist_remediation_effectiveness_anomaly_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RemediationSnapshotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = db::persist_anomaly_remediation_effectiveness_anomaly_snapshot(
        &into_remediation_snapshot_filters(body.clone()),
        body.label.as_deref(),
        body.lookback_hours,
        body.profile_key.as_deref(),
        db::GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides {
            impacted_run_rate_warning_threshold: body.impacted_run_rate_warning_threshold,
            impacted_run_rate_critical_threshold: body.impacted_run_rate_critical_threshold,
            unavailable_run_rate_warning_threshold: body.unavailable_run_rate_warning_threshold,
            unavailable_run_rate_critical_threshold: body.unavailable_run_rate_critical_threshold,
            completion_rate_regressed_warning_threshold: body
                .completion_rate_regressed_warning_threshold,
            completion_rate_regressed_critical_threshold: body
                .completion_rate_regressed_critical_threshold,
            failure_rate_regressed_warning_threshold: body.failure_rate_regressed_warning_threshold,
            failure_rate_regressed_critical_threshold: body
                .failure_rate_regressed_critical_threshold,
            request_artifact_regressed_warning_threshold: body
                .request_artifact_regressed_warning_threshold,
            request_artifact_regressed_critical_threshold: body
                .request_artifact_regressed_critical_threshold,
            response_artifact_regressed_warning_threshold: body
                .response_artifact_regressed_warning_threshold,
            response_artifact_regressed_critical_threshold: body
                .response_artifact_regressed_critical_threshold,
            first_token_latency_regressed_warning_threshold: body
                .first_token_latency_regressed_warning_threshold,
            first_token_latency_regressed_critical_threshold: body
                .first_token_latency_regressed_critical_threshold,
            total_tokens_regressed_warning_threshold: body.total_tokens_regressed_warning_threshold,
            total_tokens_regressed_critical_threshold: body
                .total_tokens_regressed_critical_threshold,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

fn into_remediation_snapshot_filters(
    query: RemediationSnapshotQuery,
) -> db::GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
    db::GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
        snapshot_id: query.snapshot_id,
        label: query.label,
        route_policy_id: query.route_policy_id,
        action_key: query.action_key,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}

fn into_remediation_anomaly_snapshot_filters(
    query: RemediationSnapshotQuery,
) -> db::GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
    db::GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
        snapshot_id: query.snapshot_id,
        label: query.label,
        route_policy_id: query.route_policy_id,
        action_key: query.action_key,
        profile_key: query.profile_key,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}

fn into_snapshot_run_filters(
    query: &RemediationSnapshotQuery,
) -> db::GatewayAnalysisAnomalyRemediationRunFilters {
    db::GatewayAnalysisAnomalyRemediationRunFilters {
        incident_id: query.incident_id.clone(),
        policy_id: query.policy_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        action_key: query.action_key.clone(),
        status: query.status.clone(),
        execution_mode: query.execution_mode.clone(),
        dry_run: query.dry_run,
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}
