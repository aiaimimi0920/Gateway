//! Management rate-limit hotspot reports and snapshots.

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
