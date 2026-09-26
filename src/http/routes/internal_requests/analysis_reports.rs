//! Management reports over persisted analysis exports.

use super::super::internal_gateway::assert_management_access;
use super::analysis_exports::{
    into_persisted_analysis_export_filters, PersistedAnalysisExportQuery,
};
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisExportDiffQuery {
    pub left_export_id: Option<String>,
    pub right_export_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisExportAnomalyQuery {
    pub policy_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub profile_key: Option<String>,
    pub failure_rate_warning_threshold: Option<f64>,
    pub failure_rate_critical_threshold: Option<f64>,
    pub failure_rate_delta_ratio_threshold: Option<f64>,
    pub completion_rate_warning_threshold: Option<f64>,
    pub completion_rate_critical_threshold: Option<f64>,
    pub completion_rate_delta_value_threshold: Option<f64>,
    pub response_artifact_coverage_warning_threshold: Option<f64>,
    pub response_artifact_coverage_critical_threshold: Option<f64>,
    pub response_artifact_coverage_delta_value_threshold: Option<f64>,
    pub request_artifact_coverage_warning_threshold: Option<f64>,
    pub request_artifact_coverage_critical_threshold: Option<f64>,
    pub request_artifact_coverage_delta_value_threshold: Option<f64>,
    pub tokens_per_sample_warning_delta_ratio_threshold: Option<f64>,
    pub tokens_per_sample_critical_delta_ratio_threshold: Option<f64>,
    pub tokens_per_sample_critical_absolute_threshold: Option<f64>,
}

pub async fn get_persisted_analysis_export_diff(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnalysisExportDiffQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let diff = db::get_persisted_analysis_export_diff(
        required_pg_pool(state.as_ref())?,
        query.left_export_id.as_deref().unwrap_or_default(),
        query.right_export_id.as_deref().unwrap_or_default(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "diff": diff,
    })))
}

pub async fn get_analysis_export_baseline_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_analysis_export_baseline_report(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_analysis_export_timeline_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_analysis_export_timeline_report(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_analysis_export_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_analysis_export_trend_report(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_analysis_export_anomaly_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnalysisExportAnomalyQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_analysis_export_anomaly_report(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters_from_anomaly(&query),
        query.policy_id.as_deref(),
        query.profile_key.as_deref(),
        db::GatewayAnalysisExportAnomalyOverrides {
            failure_rate_warning_threshold: query.failure_rate_warning_threshold,
            failure_rate_critical_threshold: query.failure_rate_critical_threshold,
            failure_rate_delta_ratio_threshold: query.failure_rate_delta_ratio_threshold,
            completion_rate_warning_threshold: query.completion_rate_warning_threshold,
            completion_rate_critical_threshold: query.completion_rate_critical_threshold,
            completion_rate_delta_value_threshold: query.completion_rate_delta_value_threshold,
            response_artifact_coverage_warning_threshold: query
                .response_artifact_coverage_warning_threshold,
            response_artifact_coverage_critical_threshold: query
                .response_artifact_coverage_critical_threshold,
            response_artifact_coverage_delta_value_threshold: query
                .response_artifact_coverage_delta_value_threshold,
            request_artifact_coverage_warning_threshold: query
                .request_artifact_coverage_warning_threshold,
            request_artifact_coverage_critical_threshold: query
                .request_artifact_coverage_critical_threshold,
            request_artifact_coverage_delta_value_threshold: query
                .request_artifact_coverage_delta_value_threshold,
            tokens_per_sample_warning_delta_ratio_threshold: query
                .tokens_per_sample_warning_delta_ratio_threshold,
            tokens_per_sample_critical_delta_ratio_threshold: query
                .tokens_per_sample_critical_delta_ratio_threshold,
            tokens_per_sample_critical_absolute_threshold: query
                .tokens_per_sample_critical_absolute_threshold,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

fn into_persisted_analysis_export_filters_from_anomaly(
    query: &AnalysisExportAnomalyQuery,
) -> db::GatewayPersistedAnalysisExportFilters {
    db::GatewayPersistedAnalysisExportFilters {
        export_id: None,
        label: query.label.clone(),
        tag: query.tag.clone(),
        project_id: query.project_id.clone(),
        status: query.status.clone(),
        text_mode: query.text_mode.clone(),
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}
