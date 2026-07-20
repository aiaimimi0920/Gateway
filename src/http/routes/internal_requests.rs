use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::object_storage::gateway_object_storage;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuditQuery {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub profile_key: Option<String>,
    pub routing_score_warning_threshold: Option<f64>,
    pub routing_score_critical_threshold: Option<f64>,
    pub degraded_route_warning_threshold: Option<f64>,
    pub degraded_route_critical_threshold: Option<f64>,
    pub saturated_route_warning_threshold: Option<f64>,
    pub saturated_route_critical_threshold: Option<f64>,
    pub breaker_open_route_warning_threshold: Option<f64>,
    pub breaker_open_route_critical_threshold: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCacheQuery {
    #[serde(flatten)]
    pub filters: RequestAuditQuery,
    pub input_price_per_million: Option<f64>,
    pub bucket_size: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisExportQuery {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub text_mode: Option<String>,
    pub max_text_chars: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedAnalysisExportQuery {
    pub export_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistAnalysisExportInput {
    pub label: Option<String>,
    pub tags: Option<Vec<String>>,
    pub retention_expires_at: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub text_mode: Option<String>,
    pub max_text_chars: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitHotspotQuery {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub endpoint_kind: Option<String>,
    pub error_code: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub lookback_hours: Option<i32>,
    pub window_size: Option<usize>,
    pub bucket_size_minutes: Option<usize>,
    pub profile_key: Option<String>,
    pub total_rate_limited_requests_warning_threshold: Option<f64>,
    pub total_rate_limited_requests_critical_threshold: Option<f64>,
    pub total_rate_limited_requests_delta_ratio_threshold: Option<f64>,
    pub top_code_share_warning_threshold: Option<f64>,
    pub top_code_share_critical_threshold: Option<f64>,
    pub top_project_share_warning_threshold: Option<f64>,
    pub top_project_share_critical_threshold: Option<f64>,
    pub top_api_key_share_warning_threshold: Option<f64>,
    pub top_api_key_share_critical_threshold: Option<f64>,
    pub top_requested_model_share_warning_threshold: Option<f64>,
    pub top_requested_model_share_critical_threshold: Option<f64>,
    pub top_endpoint_share_warning_threshold: Option<f64>,
    pub top_endpoint_share_critical_threshold: Option<f64>,
}

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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyIncidentQuery {
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
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertDispatchInput {
    pub alerted_at: Option<String>,
    pub alert_severity: Option<String>,
    pub alert_level: Option<i32>,
    pub note: Option<String>,
    pub mailbox_recipient_count: Option<i32>,
    pub webhook_dispatched: Option<bool>,
    pub webhook_skipped_reason: Option<String>,
    pub remediation_action_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyIncidentSyncInput {
    pub policy_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub profile_key: Option<String>,
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePressureQuery {
    pub project_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub limit: Option<usize>,
}

pub async fn get_model_association_matrix(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let matrix = db::get_model_association_matrix(required_pg_pool(state.as_ref())?).await?;
    Ok(Json(serde_json::json!({
        "matrix": matrix,
    })))
}

pub async fn get_cost_overview(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let overview = db::get_cost_overview(required_pg_pool(state.as_ref())?).await?;
    Ok(Json(serde_json::json!({
        "overview": overview,
    })))
}

pub async fn get_runtime_pressure(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RuntimePressureQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let pressure = db::get_runtime_pressure(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
        &db::GatewayRuntimePressureFilters {
            project_id: query.project_id,
            provider_account_id: query.provider_account_id,
            limit: query.limit,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "pressure": pressure,
    })))
}

pub async fn list_request_audits(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let requests =
        db::list_request_audits(required_pg_pool(state.as_ref())?, &into_filters(query)).await?;
    Ok(Json(serde_json::json!({
        "requests": requests,
    })))
}

pub async fn summarize_request_audits(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary =
        db::summarize_request_audits(required_pg_pool(state.as_ref())?, &into_filters(query))
            .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn list_analysis_samples(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let samples =
        db::list_analysis_samples(required_pg_pool(state.as_ref())?, &into_filters(query)).await?;
    Ok(Json(serde_json::json!({
        "samples": samples,
    })))
}

pub async fn summarize_analysis(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary =
        db::summarize_analysis(required_pg_pool(state.as_ref())?, &into_filters(query)).await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn summarize_prompt_cache(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_prompt_cache(
        required_pg_pool(state.as_ref())?,
        &into_filters(query.filters),
        query.input_price_per_million,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_prompt_cache_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_prompt_cache_trend_report(
        required_pg_pool(state.as_ref())?,
        &into_filters(query.filters),
        query.input_price_per_million,
        query.bucket_size.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn summarize_provider_routing_analysis(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_provider_routing_analysis(
        required_pg_pool(state.as_ref())?,
        &into_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn export_analysis_rows(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::export_analysis_rows(
        required_pg_pool(state.as_ref())?,
        &into_analysis_export_filters(&query),
        query.text_mode.as_deref(),
        query.max_text_chars,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn persist_analysis_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<PersistAnalysisExportInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::persist_analysis_export(
        required_pg_pool(state.as_ref())?,
        &into_analysis_export_filters_from_body(&body),
        body.text_mode.as_deref(),
        body.max_text_chars,
        db::PersistGatewayAnalysisExportInput {
            label: body.label,
            tags: body.tags,
            retention_expires_at: body.retention_expires_at,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn list_persisted_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let exports = db::list_persisted_analysis_exports(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "exports": exports,
    })))
}

pub async fn summarize_persisted_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_persisted_analysis_exports(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_persisted_analysis_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(export_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export =
        db::get_persisted_analysis_export(required_pg_pool(state.as_ref())?, export_id.trim())
            .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn update_persisted_analysis_export_metadata(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(export_id): Path<String>,
    Json(body): Json<db::GatewayAnalysisExportMetadataUpdateInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::update_persisted_analysis_export_metadata(
        required_pg_pool(state.as_ref())?,
        export_id.trim(),
        body,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn cleanup_expired_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<db::GatewayAnalysisExportCleanupInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let cleanup =
        db::cleanup_expired_analysis_exports(required_pg_pool(state.as_ref())?, body).await?;
    Ok(Json(serde_json::json!({
        "cleanup": cleanup,
    })))
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

pub async fn summarize_rate_limit_hotspots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_rate_limit_hotspots(
        required_pg_pool(state.as_ref())?,
        &into_rate_limit_hotspot_filters(&query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_rate_limit_hotspot_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_rate_limit_hotspot_trend_report(
        required_pg_pool(state.as_ref())?,
        &into_rate_limit_hotspot_filters(&query),
        query.window_size,
        query.bucket_size_minutes,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_rate_limit_hotspot_anomaly_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let overrides = db::GatewayRateLimitHotspotAnomalyOverrides {
        total_rate_limited_requests_warning_threshold: query
            .total_rate_limited_requests_warning_threshold,
        total_rate_limited_requests_critical_threshold: query
            .total_rate_limited_requests_critical_threshold,
        total_rate_limited_requests_delta_ratio_threshold: query
            .total_rate_limited_requests_delta_ratio_threshold,
        top_code_share_warning_threshold: query.top_code_share_warning_threshold,
        top_code_share_critical_threshold: query.top_code_share_critical_threshold,
        top_project_share_warning_threshold: query.top_project_share_warning_threshold,
        top_project_share_critical_threshold: query.top_project_share_critical_threshold,
        top_api_key_share_warning_threshold: query.top_api_key_share_warning_threshold,
        top_api_key_share_critical_threshold: query.top_api_key_share_critical_threshold,
        top_requested_model_share_warning_threshold: query
            .top_requested_model_share_warning_threshold,
        top_requested_model_share_critical_threshold: query
            .top_requested_model_share_critical_threshold,
        top_endpoint_share_warning_threshold: query.top_endpoint_share_warning_threshold,
        top_endpoint_share_critical_threshold: query.top_endpoint_share_critical_threshold,
    };
    let report = db::get_rate_limit_hotspot_anomaly_report(
        required_pg_pool(state.as_ref())?,
        &into_rate_limit_hotspot_filters(&query),
        query.window_size,
        query.bucket_size_minutes,
        query.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn persist_rate_limit_hotspot_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = db::persist_rate_limit_hotspot_snapshot(
        required_pg_pool(state.as_ref())?,
        &into_rate_limit_hotspot_filters(&body),
        body.label.as_deref(),
        body.lookback_hours,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn list_rate_limit_hotspot_snapshots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshots =
        db::list_rate_limit_hotspot_snapshots(&into_rate_limit_hotspot_snapshot_filters(&query))
            .await?;
    Ok(Json(serde_json::json!({
        "snapshots": snapshots,
    })))
}

pub async fn get_rate_limit_hotspot_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(snapshot_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = db::get_rate_limit_hotspot_snapshot(snapshot_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn summarize_rate_limit_hotspot_snapshot_inventory(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_rate_limit_hotspot_snapshot_inventory(
        &into_rate_limit_hotspot_snapshot_filters(&query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_rate_limit_hotspot_snapshot_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_rate_limit_hotspot_snapshot_trend_report(
        &into_rate_limit_hotspot_snapshot_filters(&query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn persist_rate_limit_hotspot_anomaly_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let overrides = db::GatewayRateLimitHotspotAnomalyOverrides {
        total_rate_limited_requests_warning_threshold: body
            .total_rate_limited_requests_warning_threshold,
        total_rate_limited_requests_critical_threshold: body
            .total_rate_limited_requests_critical_threshold,
        total_rate_limited_requests_delta_ratio_threshold: body
            .total_rate_limited_requests_delta_ratio_threshold,
        top_code_share_warning_threshold: body.top_code_share_warning_threshold,
        top_code_share_critical_threshold: body.top_code_share_critical_threshold,
        top_project_share_warning_threshold: body.top_project_share_warning_threshold,
        top_project_share_critical_threshold: body.top_project_share_critical_threshold,
        top_api_key_share_warning_threshold: body.top_api_key_share_warning_threshold,
        top_api_key_share_critical_threshold: body.top_api_key_share_critical_threshold,
        top_requested_model_share_warning_threshold: body
            .top_requested_model_share_warning_threshold,
        top_requested_model_share_critical_threshold: body
            .top_requested_model_share_critical_threshold,
        top_endpoint_share_warning_threshold: body.top_endpoint_share_warning_threshold,
        top_endpoint_share_critical_threshold: body.top_endpoint_share_critical_threshold,
    };
    let snapshot = db::persist_rate_limit_hotspot_anomaly_snapshot(
        required_pg_pool(state.as_ref())?,
        &into_rate_limit_hotspot_filters(&body),
        body.label.as_deref(),
        body.lookback_hours,
        body.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn list_rate_limit_hotspot_anomaly_snapshots(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshots = db::list_rate_limit_hotspot_anomaly_snapshots(
        &into_rate_limit_hotspot_anomaly_snapshot_filters(&query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "snapshots": snapshots,
    })))
}

pub async fn get_rate_limit_hotspot_anomaly_snapshot(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(snapshot_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot = db::get_rate_limit_hotspot_anomaly_snapshot(snapshot_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "snapshot": snapshot,
    })))
}

pub async fn sync_rate_limit_hotspot_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RateLimitHotspotQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let snapshot_id = body
        .snapshot_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GatewayError::bad_request("snapshotId 不能为空"))?;
    let result = db::sync_rate_limit_hotspot_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        snapshot_id,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "result": result,
    })))
}

pub async fn get_provider_routing_anomaly_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(query.clone());
    let overrides = db::GatewayProviderRoutingAnalysisAnomalyOverrides {
        routing_score_warning_threshold: query.routing_score_warning_threshold,
        routing_score_critical_threshold: query.routing_score_critical_threshold,
        degraded_route_warning_threshold: query.degraded_route_warning_threshold,
        degraded_route_critical_threshold: query.degraded_route_critical_threshold,
        saturated_route_warning_threshold: query.saturated_route_warning_threshold,
        saturated_route_critical_threshold: query.saturated_route_critical_threshold,
        breaker_open_route_warning_threshold: query.breaker_open_route_warning_threshold,
        breaker_open_route_critical_threshold: query.breaker_open_route_critical_threshold,
    };
    let report = db::get_provider_routing_anomaly_report(
        required_pg_pool(state.as_ref())?,
        &filters,
        query.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn sync_provider_routing_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(body.clone());
    let overrides = db::GatewayProviderRoutingAnalysisAnomalyOverrides {
        routing_score_warning_threshold: body.routing_score_warning_threshold,
        routing_score_critical_threshold: body.routing_score_critical_threshold,
        degraded_route_warning_threshold: body.degraded_route_warning_threshold,
        degraded_route_critical_threshold: body.degraded_route_critical_threshold,
        saturated_route_warning_threshold: body.saturated_route_warning_threshold,
        saturated_route_critical_threshold: body.saturated_route_critical_threshold,
        breaker_open_route_warning_threshold: body.breaker_open_route_warning_threshold,
        breaker_open_route_critical_threshold: body.breaker_open_route_critical_threshold,
    };
    let result = db::sync_provider_routing_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &filters,
        body.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "result": result,
    })))
}

pub async fn sync_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<AnomalyIncidentSyncInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let sync = db::sync_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        db::GatewayAnalysisAnomalyIncidentSyncInput {
            policy_id: body.policy_id,
            label: body.label,
            tag: body.tag,
            project_id: body.project_id,
            route_policy_id: body.route_policy_id,
            provider_account_id: body.provider_account_id,
            session_id: body.session_id,
            api_key_id: body.api_key_id,
            user_credential_id: body.user_credential_id,
            response_id: body.response_id,
            protocol_family: body.protocol_family,
            status: body.status,
            endpoint_kind: body.endpoint_kind,
            stream: body.stream,
            error_code: body.error_code,
            fallback_eligible: body.fallback_eligible,
            artifact_available: body.artifact_available,
            text_mode: body.text_mode,
            created_from: body.created_from,
            created_to: body.created_to,
            limit: body.limit,
            profile_key: body.profile_key,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "sync": sync,
    })))
}

pub async fn list_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incidents = db::list_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incidents": incidents,
    })))
}

pub async fn summarize_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn list_anomaly_incident_alert_queue(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let queue = db::list_anomaly_incident_alert_queue(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "queue": queue,
    })))
}

pub async fn list_anomaly_incident_history(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let history = db::list_anomaly_incident_history(
        required_pg_pool(state.as_ref())?,
        incident_id.trim(),
        query.limit,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "history": history,
    })))
}

pub async fn acknowledge_anomaly_incident(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident =
        db::acknowledge_anomaly_incident(required_pg_pool(state.as_ref())?, incident_id.trim())
            .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn resolve_anomaly_incident(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident =
        db::resolve_anomaly_incident(required_pg_pool(state.as_ref())?, incident_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn update_anomaly_incident_follow_up(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Json(body): Json<db::GatewayAnalysisAnomalyIncidentFollowUpInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident = db::update_anomaly_incident_follow_up(
        required_pg_pool(state.as_ref())?,
        incident_id.trim(),
        body,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn record_anomaly_incident_alert_dispatch(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Json(body): Json<AlertDispatchInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident = db::record_anomaly_incident_alert_dispatch(
        required_pg_pool(state.as_ref())?,
        management_actor_user_id(&headers),
        incident_id.trim(),
        db::RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput {
            alerted_at: body.alerted_at,
            alert_severity: body.alert_severity,
            alert_level: body.alert_level,
            note: body.note,
            mailbox_recipient_count: body.mailbox_recipient_count,
            webhook_dispatched: body.webhook_dispatched,
            webhook_skipped_reason: body.webhook_skipped_reason,
            remediation_action_keys: body.remediation_action_keys,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
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

pub async fn get_request_audit_by_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(request_audit_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        Some(request_audit_id.trim()),
        None,
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "requestAudit": request_audit,
    })))
}

pub async fn get_request_audit_by_response_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        None,
        Some(response_id.trim()),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "requestAudit": request_audit,
    })))
}

pub async fn get_request_artifacts_by_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(request_audit_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        Some(request_audit_id.trim()),
        None,
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "artifacts": build_request_artifacts_view(&request_audit).await,
    })))
}

pub async fn get_request_artifacts_by_response_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        None,
        Some(response_id.trim()),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "artifacts": build_request_artifacts_view(&request_audit).await,
    })))
}

fn into_filters(query: RequestAuditQuery) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: query.project_id,
        route_policy_id: query.route_policy_id,
        provider_account_id: query.provider_account_id,
        session_id: query.session_id,
        api_key_id: query.api_key_id,
        user_credential_id: query.user_credential_id,
        access_key_id: query.access_key_id,
        source_access_key_id: query.source_access_key_id,
        response_id: query.response_id,
        protocol_family: query.protocol_family,
        status: query.status,
        endpoint_kind: query.endpoint_kind,
        stream: query.stream,
        error_code: query.error_code,
        fallback_eligible: query.fallback_eligible,
        artifact_available: query.artifact_available,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}

fn into_analysis_export_filters(query: &AnalysisExportQuery) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: query.project_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        provider_account_id: query.provider_account_id.clone(),
        session_id: query.session_id.clone(),
        api_key_id: query.api_key_id.clone(),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: query.response_id.clone(),
        protocol_family: query.protocol_family.clone(),
        status: query.status.clone(),
        endpoint_kind: query.endpoint_kind.clone(),
        stream: query.stream,
        error_code: query.error_code.clone(),
        fallback_eligible: query.fallback_eligible,
        artifact_available: query.artifact_available,
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}

fn into_analysis_export_filters_from_body(
    body: &PersistAnalysisExportInput,
) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: body.project_id.clone(),
        route_policy_id: body.route_policy_id.clone(),
        provider_account_id: body.provider_account_id.clone(),
        session_id: body.session_id.clone(),
        api_key_id: body.api_key_id.clone(),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: body.response_id.clone(),
        protocol_family: body.protocol_family.clone(),
        status: body.status.clone(),
        endpoint_kind: body.endpoint_kind.clone(),
        stream: body.stream,
        error_code: body.error_code.clone(),
        fallback_eligible: body.fallback_eligible,
        artifact_available: body.artifact_available,
        created_from: body.created_from.clone(),
        created_to: body.created_to.clone(),
        limit: body.limit,
    }
}

fn into_persisted_analysis_export_filters(
    query: PersistedAnalysisExportQuery,
) -> db::GatewayPersistedAnalysisExportFilters {
    db::GatewayPersistedAnalysisExportFilters {
        export_id: query.export_id,
        label: query.label,
        tag: query.tag,
        project_id: query.project_id,
        status: query.status,
        text_mode: query.text_mode,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
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

fn into_rate_limit_hotspot_filters(query: &RateLimitHotspotQuery) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: query.project_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        provider_account_id: query.provider_account_id.clone(),
        session_id: query.session_id.clone(),
        api_key_id: query.api_key_id.clone(),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: query.response_id.clone(),
        protocol_family: query.protocol_family.clone(),
        status: None,
        endpoint_kind: query.endpoint_kind.clone(),
        stream: None,
        error_code: query.error_code.clone(),
        fallback_eligible: None,
        artifact_available: None,
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}

fn into_rate_limit_hotspot_snapshot_filters(
    query: &RateLimitHotspotQuery,
) -> db::GatewayRateLimitHotspotSnapshotFilters {
    db::GatewayRateLimitHotspotSnapshotFilters {
        snapshot_id: query.snapshot_id.clone(),
        label: query.label.clone(),
        project_id: query.project_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        api_key_id: query.api_key_id.clone(),
        endpoint_kind: query.endpoint_kind.clone(),
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}

fn into_rate_limit_hotspot_anomaly_snapshot_filters(
    query: &RateLimitHotspotQuery,
) -> db::GatewayRateLimitHotspotAnomalySnapshotFilters {
    db::GatewayRateLimitHotspotAnomalySnapshotFilters {
        snapshot_id: query.snapshot_id.clone(),
        label: query.label.clone(),
        project_id: query.project_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        api_key_id: query.api_key_id.clone(),
        endpoint_kind: query.endpoint_kind.clone(),
        profile_key: query.profile_key.clone(),
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}

fn into_incident_filters(query: AnomalyIncidentQuery) -> db::GatewayAnalysisAnomalyIncidentFilters {
    db::GatewayAnalysisAnomalyIncidentFilters {
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
        due_only: query.due_only,
        limit: query.limit,
    }
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

fn into_remediation_run_filters(
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

fn into_remediation_queue_filters(
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

fn management_actor_user_id(headers: &AxumHeaderMap) -> &str {
    headers
        .get("x-operator-user-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            headers
                .get("x-user-id")
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or("management")
}

async fn build_request_artifacts_view(request_audit: &db::GatewayRequestAuditView) -> Value {
    let request_artifact = match request_audit.request_artifact_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };
    let response_artifact = match request_audit.response_artifact_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };

    serde_json::json!({
        "requestAudit": request_audit,
        "requestArtifact": request_artifact,
        "responseArtifact": response_artifact,
    })
}

async fn read_optional_artifact(object_key: &str) -> Option<Value> {
    let storage = gateway_object_storage().ok()?;
    storage.read_json(object_key).await.ok()
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
