use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;

use super::format_timestamp;
use super::request_audits::{
    list_request_audits, GatewayRequestAuditView, GatewaySummaryBucketKeyView, RequestAuditFilters,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSummaryView {
    pub total_rate_limited_requests: usize,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
    pub by_project: Vec<GatewaySummaryBucketKeyView>,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_api_key_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_requested_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_resolved_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_endpoint_kind: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotFilterView {
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
    pub limit: usize,
    pub window_size: usize,
    pub bucket_size_minutes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotTrendPointView {
    pub bucket_start_at: String,
    pub bucket_end_at: String,
    pub total_rate_limited_requests: usize,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
    pub by_project: Vec<GatewaySummaryBucketKeyView>,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_api_key_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_requested_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_resolved_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_endpoint_kind: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotMetricSummaryView {
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotTrendSummaryView {
    pub latest_bucket_start_at: Option<String>,
    pub previous_bucket_start_at: Option<String>,
    pub total_rate_limited_requests: GatewayRateLimitHotspotMetricSummaryView,
    pub top_code_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_project_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_api_key_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_requested_model_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_endpoint_share: GatewayRateLimitHotspotMetricSummaryView,
    pub latest_top_code_key: Option<String>,
    pub latest_top_project_key: Option<String>,
    pub latest_top_api_key_key: Option<String>,
    pub latest_top_requested_model_key: Option<String>,
    pub latest_top_endpoint_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotTrendReportView {
    pub generated_at: String,
    pub filters: GatewayRateLimitHotspotFilterView,
    pub matched_requests_count: usize,
    pub window_size: usize,
    pub bucket_size_minutes: usize,
    pub points: Vec<GatewayRateLimitHotspotTrendPointView>,
    pub summary: Option<GatewayRateLimitHotspotTrendSummaryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotAnomalyThresholdConfig {
    pub total_rate_limited_requests_warning_threshold: f64,
    pub total_rate_limited_requests_critical_threshold: f64,
    pub total_rate_limited_requests_delta_ratio_threshold: f64,
    pub top_code_share_warning_threshold: f64,
    pub top_code_share_critical_threshold: f64,
    pub top_project_share_warning_threshold: f64,
    pub top_project_share_critical_threshold: f64,
    pub top_api_key_share_warning_threshold: f64,
    pub top_api_key_share_critical_threshold: f64,
    pub top_requested_model_share_warning_threshold: f64,
    pub top_requested_model_share_critical_threshold: f64,
    pub top_endpoint_share_warning_threshold: f64,
    pub top_endpoint_share_critical_threshold: f64,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRateLimitHotspotAnomalyOverrides {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub entity_key: Option<String>,
    pub latest_bucket_start_at: Option<String>,
    pub previous_bucket_start_at: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayRateLimitHotspotFilterView,
    pub profile_key: String,
    pub thresholds: GatewayRateLimitHotspotAnomalyThresholdConfig,
    pub trend_summary: Option<GatewayRateLimitHotspotTrendSummaryView>,
    pub latest_point: Option<GatewayRateLimitHotspotTrendPointView>,
    pub previous_point: Option<GatewayRateLimitHotspotTrendPointView>,
    pub anomalies: Vec<GatewayRateLimitHotspotAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRateLimitHotspotSnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub api_key_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRateLimitHotspotAnomalySnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub api_key_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub profile_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotFilterView {
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
    pub limit: usize,
    pub lookback_hours: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayRateLimitHotspotSnapshotFilterView,
    pub summary: GatewayRateLimitHotspotSummaryView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotInventorySummaryView {
    pub total_snapshots: usize,
    pub total_rate_limited_requests: usize,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
    pub by_project: Vec<GatewaySummaryBucketKeyView>,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_api_key_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_requested_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_resolved_model: Vec<GatewaySummaryBucketKeyView>,
    pub by_endpoint_kind: Vec<GatewaySummaryBucketKeyView>,
    pub by_label: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotReportFilterView {
    pub label: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub api_key_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotTrendPointView {
    pub snapshot: GatewayRateLimitHotspotSnapshotView,
    pub total_rate_limited_requests: usize,
    pub top_code_share: Option<f64>,
    pub top_project_share: Option<f64>,
    pub top_api_key_share: Option<f64>,
    pub top_requested_model_share: Option<f64>,
    pub top_endpoint_share: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotTrendSummaryView {
    pub latest_snapshot_id: Option<String>,
    pub previous_snapshot_id: Option<String>,
    pub total_rate_limited_requests: GatewayRateLimitHotspotMetricSummaryView,
    pub top_code_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_project_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_api_key_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_requested_model_share: GatewayRateLimitHotspotMetricSummaryView,
    pub top_endpoint_share: GatewayRateLimitHotspotMetricSummaryView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotSnapshotTrendReportView {
    pub generated_at: String,
    pub filters: GatewayRateLimitHotspotSnapshotReportFilterView,
    pub matched_snapshots_count: usize,
    pub window_size: usize,
    pub inventory_summary: GatewayRateLimitHotspotSnapshotInventorySummaryView,
    pub points: Vec<GatewayRateLimitHotspotSnapshotTrendPointView>,
    pub summary: Option<GatewayRateLimitHotspotSnapshotTrendSummaryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotAnomalySnapshotFilterView {
    pub label: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub api_key_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
    pub lookback_hours: Option<i32>,
    pub profile_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitHotspotAnomalySnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayRateLimitHotspotAnomalySnapshotFilterView,
    pub report: GatewayRateLimitHotspotAnomalyReportView,
}

pub async fn summarize_rate_limit_hotspots(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayRateLimitHotspotSummaryView, GatewayError> {
    let rows = load_rate_limit_audit_rows(pool, filters, 200, 1000).await?;
    Ok(build_rate_limit_hotspot_summary(&rows))
}

pub async fn get_rate_limit_hotspot_trend_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    window_size: Option<usize>,
    bucket_size_minutes: Option<usize>,
) -> Result<GatewayRateLimitHotspotTrendReportView, GatewayError> {
    let limit = filters.limit.unwrap_or(1000).clamp(1, 1000);
    let rows = load_rate_limit_audit_rows(
        pool,
        &RequestAuditFilters {
            limit: Some(limit),
            ..filters.clone()
        },
        1000,
        1000,
    )
    .await?;
    let window_size = window_size.unwrap_or(12).clamp(1, 168);
    let bucket_size_minutes = bucket_size_minutes.unwrap_or(60).clamp(1, 1440);
    let filter_view =
        to_rate_limit_hotspot_filter_view(filters, limit, window_size, bucket_size_minutes);
    Ok(build_rate_limit_hotspot_trend_report(
        format_timestamp(OffsetDateTime::now_utc()),
        filter_view,
        &rows,
    ))
}

pub async fn get_rate_limit_hotspot_anomaly_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    window_size: Option<usize>,
    bucket_size_minutes: Option<usize>,
    profile_key: Option<&str>,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> Result<GatewayRateLimitHotspotAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_profile_key(profile_key);
    let trend_report =
        get_rate_limit_hotspot_trend_report(pool, filters, window_size, bucket_size_minutes)
            .await?;
    let thresholds =
        build_rate_limit_hotspot_anomaly_thresholds(&normalized_profile_key, overrides);
    Ok(build_rate_limit_hotspot_anomaly_report(
        format_timestamp(OffsetDateTime::now_utc()),
        trend_report,
        normalized_profile_key,
        thresholds,
    ))
}

async fn load_rate_limit_audit_rows(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    default_limit: usize,
    max_limit: usize,
) -> Result<Vec<GatewayRequestAuditView>, GatewayError> {
    let limit = filters.limit.unwrap_or(default_limit).clamp(1, max_limit);
    list_request_audits(
        pool,
        &RequestAuditFilters {
            status: Some("failed".to_string()),
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await
}

fn to_rate_limit_hotspot_filter_view(
    filters: &RequestAuditFilters,
    limit: usize,
    window_size: usize,
    bucket_size_minutes: usize,
) -> GatewayRateLimitHotspotFilterView {
    GatewayRateLimitHotspotFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        error_code: filters.error_code.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit,
        window_size,
        bucket_size_minutes,
    }
}

pub(crate) fn build_rate_limit_hotspot_summary(
    rows: &[GatewayRequestAuditView],
) -> GatewayRateLimitHotspotSummaryView {
    let mut by_code = BTreeMap::new();
    let mut by_project = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_api_key_id = BTreeMap::new();
    let mut by_requested_model = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut total_rate_limited_requests = 0usize;

    for row in rows {
        let Some(error_code) = route_trace_error_code(row) else {
            continue;
        };
        if !is_rate_limit_error_code(Some(error_code)) {
            continue;
        }
        total_rate_limited_requests += 1;
        accumulate_bucket(&mut by_code, Some(error_code));
        accumulate_bucket(&mut by_project, Some(row.project_id.as_str()));
        accumulate_bucket(&mut by_route_policy_id, row.route_policy_id.as_deref());
        accumulate_bucket(&mut by_api_key_id, row.api_key_id.as_deref());
        accumulate_bucket(&mut by_requested_model, row.requested_model.as_deref());
        accumulate_bucket(&mut by_resolved_model, row.resolved_model.as_deref());
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
    }

    GatewayRateLimitHotspotSummaryView {
        total_rate_limited_requests,
        by_code: into_key_buckets(by_code),
        by_project: into_key_buckets(by_project),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_api_key_id: into_key_buckets(by_api_key_id),
        by_requested_model: into_key_buckets(by_requested_model),
        by_resolved_model: into_key_buckets(by_resolved_model),
        by_endpoint_kind: into_key_buckets(by_endpoint_kind),
    }
}

pub(crate) fn build_rate_limit_hotspot_trend_report(
    generated_at: String,
    filters: GatewayRateLimitHotspotFilterView,
    rows: &[GatewayRequestAuditView],
) -> GatewayRateLimitHotspotTrendReportView {
    let rate_limit_rows = rows
        .iter()
        .filter(|row| is_rate_limit_error_code(route_trace_error_code(row)))
        .cloned()
        .collect::<Vec<_>>();
    let bucket_size_minutes = filters.bucket_size_minutes.max(1);
    let window_size = filters.window_size.max(1);
    let bucket_size_ms = i128::from(bucket_size_minutes as i64) * 60 * 1000;
    let anchor_ms = rate_limit_rows
        .iter()
        .filter_map(|row| parse_timestamp_ms(row.created_at.as_str()))
        .max()
        .unwrap_or_else(current_time_ms);
    let latest_bucket_start_ms = anchor_ms.div_euclid(bucket_size_ms) * bucket_size_ms;
    let mut points = Vec::with_capacity(window_size);

    for index in 0..window_size {
        let bucket_start_ms = latest_bucket_start_ms - i128::from(index as i64) * bucket_size_ms;
        let bucket_end_ms = bucket_start_ms + bucket_size_ms;
        let bucket_rows = rate_limit_rows
            .iter()
            .filter(|row| {
                parse_timestamp_ms(row.created_at.as_str())
                    .is_some_and(|value| value >= bucket_start_ms && value < bucket_end_ms)
            })
            .cloned()
            .collect::<Vec<_>>();
        let summary = build_rate_limit_hotspot_summary(&bucket_rows);
        points.push(GatewayRateLimitHotspotTrendPointView {
            bucket_start_at: format_timestamp_ms(bucket_start_ms),
            bucket_end_at: format_timestamp_ms(bucket_end_ms),
            total_rate_limited_requests: summary.total_rate_limited_requests,
            by_code: summary.by_code,
            by_project: summary.by_project,
            by_route_policy_id: summary.by_route_policy_id,
            by_api_key_id: summary.by_api_key_id,
            by_requested_model: summary.by_requested_model,
            by_resolved_model: summary.by_resolved_model,
            by_endpoint_kind: summary.by_endpoint_kind,
        });
    }

    GatewayRateLimitHotspotTrendReportView {
        generated_at,
        filters,
        matched_requests_count: rate_limit_rows.len(),
        window_size,
        bucket_size_minutes,
        summary: build_rate_limit_hotspot_trend_summary(&points),
        points,
    }
}

fn build_rate_limit_hotspot_trend_summary(
    points: &[GatewayRateLimitHotspotTrendPointView],
) -> Option<GatewayRateLimitHotspotTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayRateLimitHotspotTrendSummaryView {
        latest_bucket_start_at: Some(latest.bucket_start_at.clone()),
        previous_bucket_start_at: previous.map(|value| value.bucket_start_at.clone()),
        total_rate_limited_requests: build_metric_summary(
            Some(latest.total_rate_limited_requests as f64),
            previous.map(|value| value.total_rate_limited_requests as f64),
        ),
        top_code_share: build_metric_summary(
            top_bucket_share(&latest.by_code, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_code, value.total_rate_limited_requests)
            }),
        ),
        top_project_share: build_metric_summary(
            top_bucket_share(&latest.by_project, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_project, value.total_rate_limited_requests)
            }),
        ),
        top_api_key_share: build_metric_summary(
            top_bucket_share(&latest.by_api_key_id, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_api_key_id, value.total_rate_limited_requests)
            }),
        ),
        top_requested_model_share: build_metric_summary(
            top_bucket_share(
                &latest.by_requested_model,
                latest.total_rate_limited_requests,
            ),
            previous.and_then(|value| {
                top_bucket_share(&value.by_requested_model, value.total_rate_limited_requests)
            }),
        ),
        top_endpoint_share: build_metric_summary(
            top_bucket_share(&latest.by_endpoint_kind, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_endpoint_kind, value.total_rate_limited_requests)
            }),
        ),
        latest_top_code_key: latest.by_code.first().map(|bucket| bucket.key.clone()),
        latest_top_project_key: latest.by_project.first().map(|bucket| bucket.key.clone()),
        latest_top_api_key_key: latest
            .by_api_key_id
            .first()
            .map(|bucket| bucket.key.clone()),
        latest_top_requested_model_key: latest
            .by_requested_model
            .first()
            .map(|bucket| bucket.key.clone()),
        latest_top_endpoint_key: latest
            .by_endpoint_kind
            .first()
            .map(|bucket| bucket.key.clone()),
    })
}

pub(crate) fn build_rate_limit_hotspot_anomaly_thresholds(
    profile_key: &str,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> GatewayRateLimitHotspotAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 25.0,
            total_rate_limited_requests_critical_threshold: 60.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.5,
            top_code_share_warning_threshold: 0.55,
            top_code_share_critical_threshold: 0.7,
            top_project_share_warning_threshold: 0.45,
            top_project_share_critical_threshold: 0.6,
            top_api_key_share_warning_threshold: 0.4,
            top_api_key_share_critical_threshold: 0.55,
            top_requested_model_share_warning_threshold: 0.45,
            top_requested_model_share_critical_threshold: 0.6,
            top_endpoint_share_warning_threshold: 0.6,
            top_endpoint_share_critical_threshold: 0.8,
        },
        "aggressive" => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 8.0,
            total_rate_limited_requests_critical_threshold: 20.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.2,
            top_code_share_warning_threshold: 0.35,
            top_code_share_critical_threshold: 0.5,
            top_project_share_warning_threshold: 0.3,
            top_project_share_critical_threshold: 0.45,
            top_api_key_share_warning_threshold: 0.25,
            top_api_key_share_critical_threshold: 0.4,
            top_requested_model_share_warning_threshold: 0.3,
            top_requested_model_share_critical_threshold: 0.45,
            top_endpoint_share_warning_threshold: 0.45,
            top_endpoint_share_critical_threshold: 0.65,
        },
        _ => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 15.0,
            total_rate_limited_requests_critical_threshold: 35.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.35,
            top_code_share_warning_threshold: 0.45,
            top_code_share_critical_threshold: 0.6,
            top_project_share_warning_threshold: 0.35,
            top_project_share_critical_threshold: 0.5,
            top_api_key_share_warning_threshold: 0.3,
            top_api_key_share_critical_threshold: 0.45,
            top_requested_model_share_warning_threshold: 0.35,
            top_requested_model_share_critical_threshold: 0.5,
            top_endpoint_share_warning_threshold: 0.5,
            top_endpoint_share_critical_threshold: 0.7,
        },
    };

    GatewayRateLimitHotspotAnomalyThresholdConfig {
        total_rate_limited_requests_warning_threshold: overrides
            .total_rate_limited_requests_warning_threshold
            .unwrap_or(base.total_rate_limited_requests_warning_threshold),
        total_rate_limited_requests_critical_threshold: overrides
            .total_rate_limited_requests_critical_threshold
            .unwrap_or(base.total_rate_limited_requests_critical_threshold),
        total_rate_limited_requests_delta_ratio_threshold: overrides
            .total_rate_limited_requests_delta_ratio_threshold
            .unwrap_or(base.total_rate_limited_requests_delta_ratio_threshold),
        top_code_share_warning_threshold: overrides
            .top_code_share_warning_threshold
            .unwrap_or(base.top_code_share_warning_threshold),
        top_code_share_critical_threshold: overrides
            .top_code_share_critical_threshold
            .unwrap_or(base.top_code_share_critical_threshold),
        top_project_share_warning_threshold: overrides
            .top_project_share_warning_threshold
            .unwrap_or(base.top_project_share_warning_threshold),
        top_project_share_critical_threshold: overrides
            .top_project_share_critical_threshold
            .unwrap_or(base.top_project_share_critical_threshold),
        top_api_key_share_warning_threshold: overrides
            .top_api_key_share_warning_threshold
            .unwrap_or(base.top_api_key_share_warning_threshold),
        top_api_key_share_critical_threshold: overrides
            .top_api_key_share_critical_threshold
            .unwrap_or(base.top_api_key_share_critical_threshold),
        top_requested_model_share_warning_threshold: overrides
            .top_requested_model_share_warning_threshold
            .unwrap_or(base.top_requested_model_share_warning_threshold),
        top_requested_model_share_critical_threshold: overrides
            .top_requested_model_share_critical_threshold
            .unwrap_or(base.top_requested_model_share_critical_threshold),
        top_endpoint_share_warning_threshold: overrides
            .top_endpoint_share_warning_threshold
            .unwrap_or(base.top_endpoint_share_warning_threshold),
        top_endpoint_share_critical_threshold: overrides
            .top_endpoint_share_critical_threshold
            .unwrap_or(base.top_endpoint_share_critical_threshold),
    }
}

pub(crate) fn build_rate_limit_hotspot_anomaly_report(
    generated_at: String,
    trend_report: GatewayRateLimitHotspotTrendReportView,
    profile_key: String,
    thresholds: GatewayRateLimitHotspotAnomalyThresholdConfig,
) -> GatewayRateLimitHotspotAnomalyReportView {
    let mut anomalies = Vec::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    let trend_summary = trend_report.summary.clone();
    let latest_point = trend_report.points.first().cloned();
    let previous_point = trend_report.points.get(1).cloned();

    if let Some(summary) = trend_summary.as_ref() {
        let total_latest = summary
            .total_rate_limited_requests
            .latest_value
            .unwrap_or(0.0);
        let total_delta_ok = summary
            .total_rate_limited_requests
            .delta_ratio
            .map_or(true, |value| {
                value >= thresholds.total_rate_limited_requests_delta_ratio_threshold
            });
        let total_critical = total_latest
            >= thresholds.total_rate_limited_requests_critical_threshold
            && total_delta_ok;
        let total_warning = total_latest
            >= thresholds.total_rate_limited_requests_warning_threshold
            && total_delta_ok;
        if total_warning {
            anomalies.push(GatewayRateLimitHotspotAnomalyView {
                code: "rate_limit_request_spike".to_string(),
                severity: if total_critical {
                    "critical"
                } else {
                    "warning"
                }
                .to_string(),
                message: "当前时间桶内的 rate-limit 请求量明显升高，热点流量正在打穿当前限流策略。"
                    .to_string(),
                entity_key: None,
                latest_bucket_start_at: summary.latest_bucket_start_at.clone(),
                previous_bucket_start_at: summary.previous_bucket_start_at.clone(),
                latest_value: summary.total_rate_limited_requests.latest_value,
                previous_value: summary.total_rate_limited_requests.previous_value,
                delta_value: summary.total_rate_limited_requests.delta_value,
                delta_ratio: summary.total_rate_limited_requests.delta_ratio,
                threshold_value: thresholds.total_rate_limited_requests_warning_threshold,
            });
        }

        let checks = vec![
            (
                "rate_limit_code_concentration",
                summary.top_code_share.clone(),
                thresholds.top_code_share_warning_threshold,
                thresholds.top_code_share_critical_threshold,
                summary.latest_top_code_key.clone(),
                "单一 rate-limit 错误码占比过高，说明当前热点已经集中在同一类限流路径。"
                    .to_string(),
            ),
            (
                "rate_limit_project_hotspot",
                summary.top_project_share.clone(),
                thresholds.top_project_share_warning_threshold,
                thresholds.top_project_share_critical_threshold,
                summary.latest_top_project_key.clone(),
                "单一 project 的 rate-limit 占比过高，当前热点已经明显集中在特定 project。"
                    .to_string(),
            ),
            (
                "rate_limit_api_key_hotspot",
                summary.top_api_key_share.clone(),
                thresholds.top_api_key_share_warning_threshold,
                thresholds.top_api_key_share_critical_threshold,
                summary.latest_top_api_key_key.clone(),
                "单一 API key 的 rate-limit 占比过高，当前热点已经集中到单个 key。".to_string(),
            ),
            (
                "rate_limit_model_hotspot",
                summary.top_requested_model_share.clone(),
                thresholds.top_requested_model_share_warning_threshold,
                thresholds.top_requested_model_share_critical_threshold,
                summary.latest_top_requested_model_key.clone(),
                "单一模型的 rate-limit 占比过高，当前热点已经集中在同一个模型请求面。".to_string(),
            ),
            (
                "rate_limit_endpoint_hotspot",
                summary.top_endpoint_share.clone(),
                thresholds.top_endpoint_share_warning_threshold,
                thresholds.top_endpoint_share_critical_threshold,
                summary.latest_top_endpoint_key.clone(),
                "单一 endpoint 的 rate-limit 占比过高，当前热点已经集中在同一条公开调用口径。"
                    .to_string(),
            ),
        ];

        for (code, metric, warning_threshold, critical_threshold, entity_key, message) in checks {
            if metric.latest_value.unwrap_or(0.0) < warning_threshold {
                continue;
            }
            anomalies.push(GatewayRateLimitHotspotAnomalyView {
                code: code.to_string(),
                severity: if metric.latest_value.unwrap_or(0.0) >= critical_threshold {
                    "critical".to_string()
                } else {
                    "warning".to_string()
                },
                message,
                entity_key,
                latest_bucket_start_at: summary.latest_bucket_start_at.clone(),
                previous_bucket_start_at: summary.previous_bucket_start_at.clone(),
                latest_value: metric.latest_value,
                previous_value: metric.previous_value,
                delta_value: metric.delta_value,
                delta_ratio: metric.delta_ratio,
                threshold_value: warning_threshold,
            });
        }
    }

    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }

    GatewayRateLimitHotspotAnomalyReportView {
        generated_at,
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        trend_summary,
        latest_point,
        previous_point,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

pub async fn persist_rate_limit_hotspot_snapshot(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
) -> Result<GatewayRateLimitHotspotSnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let lookback_hours = normalize_lookback_hours(lookback_hours);
    let created_from = if filters.created_from.is_some() {
        filters.created_from.clone()
    } else if let Some(hours) = lookback_hours {
        Some(format_timestamp(
            timestamp - time::Duration::hours(i64::from(hours)),
        ))
    } else {
        None
    };
    let limit = filters.limit.unwrap_or(1000).clamp(1, 1000);
    let normalized_filters = RequestAuditFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let summary = summarize_rate_limit_hotspots(pool, &normalized_filters).await?;
    let snapshot_id = Uuid::new_v4().to_string();
    let object_key = build_rate_limit_hotspot_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayRateLimitHotspotSnapshotView {
        snapshot_id,
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayRateLimitHotspotSnapshotFilterView {
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            provider_account_id: normalized_filters.provider_account_id,
            session_id: normalized_filters.session_id,
            api_key_id: normalized_filters.api_key_id,
            response_id: normalized_filters.response_id,
            protocol_family: normalized_filters.protocol_family,
            endpoint_kind: normalized_filters.endpoint_kind,
            error_code: normalized_filters.error_code,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
        },
        summary,
    };
    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize rate-limit hotspot snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_rate_limit_hotspot_snapshots(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<Vec<GatewayRateLimitHotspotSnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(started_at), Some(ended_at)) = (created_from.as_ref(), created_to.as_ref()) {
        if started_at > ended_at {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects(rate_limit_hotspot_snapshot_prefix())
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys
        .into_iter()
        .filter(|key| key.ends_with("/snapshot.json"))
    {
        let snapshot =
            read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotSnapshotView>(&object_key)
                .await
                .ok();
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_rate_limit_hotspot_snapshot_filters(
            &snapshot,
            filters,
            created_from.as_ref(),
            created_to.as_ref(),
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn get_rate_limit_hotspot_snapshot(
    snapshot_id: &str,
) -> Result<GatewayRateLimitHotspotSnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(Some(snapshot_id))
        .ok_or_else(|| GatewayError::conflict("snapshotId 不能为空"))?;
    read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotSnapshotView>(
        &build_rate_limit_hotspot_snapshot_object_key(snapshot_id),
    )
    .await
}

pub async fn summarize_rate_limit_hotspot_snapshot_inventory(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<GatewayRateLimitHotspotSnapshotInventorySummaryView, GatewayError> {
    let snapshots = list_rate_limit_hotspot_snapshots(&GatewayRateLimitHotspotSnapshotFilters {
        limit: Some(filters.limit.unwrap_or(500).clamp(1, 500)),
        ..filters.clone()
    })
    .await?;
    Ok(build_rate_limit_hotspot_snapshot_inventory_summary(
        &snapshots,
    ))
}

pub async fn get_rate_limit_hotspot_snapshot_trend_report(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<GatewayRateLimitHotspotSnapshotTrendReportView, GatewayError> {
    let window_size = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = GatewayRateLimitHotspotSnapshotFilters {
        limit: Some(window_size),
        ..filters.clone()
    };
    let snapshots = list_rate_limit_hotspot_snapshots(&normalized_filters).await?;
    let inventory_summary =
        summarize_rate_limit_hotspot_snapshot_inventory(&GatewayRateLimitHotspotSnapshotFilters {
            limit: Some(500),
            ..normalized_filters.clone()
        })
        .await?;
    let points = snapshots
        .into_iter()
        .map(build_rate_limit_hotspot_snapshot_trend_point)
        .collect::<Vec<_>>();
    Ok(build_rate_limit_hotspot_snapshot_trend_report(
        format_timestamp(OffsetDateTime::now_utc()),
        GatewayRateLimitHotspotSnapshotReportFilterView {
            label: normalized_filters.label,
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            api_key_id: normalized_filters.api_key_id,
            endpoint_kind: normalized_filters.endpoint_kind,
            created_from: normalized_filters.created_from,
            created_to: normalized_filters.created_to,
        },
        window_size,
        inventory_summary,
        points,
    ))
}

pub async fn persist_rate_limit_hotspot_anomaly_snapshot(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
    profile_key: Option<&str>,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> Result<GatewayRateLimitHotspotAnomalySnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let lookback_hours = normalize_lookback_hours(lookback_hours);
    let created_from = if filters.created_from.is_some() {
        filters.created_from.clone()
    } else if let Some(hours) = lookback_hours {
        Some(format_timestamp(
            timestamp - time::Duration::hours(i64::from(hours)),
        ))
    } else {
        None
    };
    let limit = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = RequestAuditFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let report = get_rate_limit_hotspot_anomaly_report(
        pool,
        &normalized_filters,
        None,
        None,
        profile_key,
        overrides,
    )
    .await?;
    let snapshot_id = Uuid::new_v4().to_string();
    let object_key = build_rate_limit_hotspot_anomaly_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayRateLimitHotspotAnomalySnapshotView {
        snapshot_id,
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayRateLimitHotspotAnomalySnapshotFilterView {
            label: trimmed_owned(label),
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            api_key_id: normalized_filters.api_key_id,
            endpoint_kind: normalized_filters.endpoint_kind,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
            profile_key: report.profile_key.clone(),
        },
        report,
    };
    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize rate-limit hotspot anomaly snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_rate_limit_hotspot_anomaly_snapshots(
    filters: &GatewayRateLimitHotspotAnomalySnapshotFilters,
) -> Result<Vec<GatewayRateLimitHotspotAnomalySnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(started_at), Some(ended_at)) = (created_from.as_ref(), created_to.as_ref()) {
        if started_at > ended_at {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects(rate_limit_hotspot_anomaly_snapshot_prefix())
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys
        .into_iter()
        .filter(|key| key.ends_with("/snapshot.json"))
    {
        let snapshot =
            read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotAnomalySnapshotView>(
                &object_key,
            )
            .await
            .ok();
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_rate_limit_hotspot_anomaly_snapshot_filters(
            &snapshot,
            filters,
            created_from.as_ref(),
            created_to.as_ref(),
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn get_rate_limit_hotspot_anomaly_snapshot(
    snapshot_id: &str,
) -> Result<GatewayRateLimitHotspotAnomalySnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(Some(snapshot_id))
        .ok_or_else(|| GatewayError::conflict("snapshotId 不能为空"))?;
    read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotAnomalySnapshotView>(
        &build_rate_limit_hotspot_anomaly_snapshot_object_key(snapshot_id),
    )
    .await
}

pub(crate) fn build_rate_limit_hotspot_snapshot_inventory_summary(
    snapshots: &[GatewayRateLimitHotspotSnapshotView],
) -> GatewayRateLimitHotspotSnapshotInventorySummaryView {
    let mut by_code = BTreeMap::new();
    let mut by_project = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_api_key_id = BTreeMap::new();
    let mut by_requested_model = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_label = BTreeMap::new();
    let mut total_rate_limited_requests = 0usize;

    for snapshot in snapshots {
        total_rate_limited_requests += snapshot.summary.total_rate_limited_requests;
        merge_key_buckets(&mut by_code, &snapshot.summary.by_code);
        merge_key_buckets(&mut by_project, &snapshot.summary.by_project);
        merge_key_buckets(
            &mut by_route_policy_id,
            &snapshot.summary.by_route_policy_id,
        );
        merge_key_buckets(&mut by_api_key_id, &snapshot.summary.by_api_key_id);
        merge_key_buckets(
            &mut by_requested_model,
            &snapshot.summary.by_requested_model,
        );
        merge_key_buckets(&mut by_resolved_model, &snapshot.summary.by_resolved_model);
        merge_key_buckets(&mut by_endpoint_kind, &snapshot.summary.by_endpoint_kind);
        push_key_bucket(&mut by_label, snapshot.label.as_deref(), 1);
    }

    GatewayRateLimitHotspotSnapshotInventorySummaryView {
        total_snapshots: snapshots.len(),
        total_rate_limited_requests,
        by_code: into_key_buckets(by_code),
        by_project: into_key_buckets(by_project),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_api_key_id: into_key_buckets(by_api_key_id),
        by_requested_model: into_key_buckets(by_requested_model),
        by_resolved_model: into_key_buckets(by_resolved_model),
        by_endpoint_kind: into_key_buckets(by_endpoint_kind),
        by_label: into_key_buckets(by_label),
    }
}

pub(crate) fn build_rate_limit_hotspot_snapshot_trend_point(
    snapshot: GatewayRateLimitHotspotSnapshotView,
) -> GatewayRateLimitHotspotSnapshotTrendPointView {
    let total = snapshot.summary.total_rate_limited_requests;
    GatewayRateLimitHotspotSnapshotTrendPointView {
        top_code_share: top_bucket_share(&snapshot.summary.by_code, total),
        top_project_share: top_bucket_share(&snapshot.summary.by_project, total),
        top_api_key_share: top_bucket_share(&snapshot.summary.by_api_key_id, total),
        top_requested_model_share: top_bucket_share(&snapshot.summary.by_requested_model, total),
        top_endpoint_share: top_bucket_share(&snapshot.summary.by_endpoint_kind, total),
        total_rate_limited_requests: total,
        snapshot,
    }
}

pub(crate) fn build_rate_limit_hotspot_snapshot_trend_report(
    generated_at: String,
    filters: GatewayRateLimitHotspotSnapshotReportFilterView,
    window_size: usize,
    inventory_summary: GatewayRateLimitHotspotSnapshotInventorySummaryView,
    points: Vec<GatewayRateLimitHotspotSnapshotTrendPointView>,
) -> GatewayRateLimitHotspotSnapshotTrendReportView {
    GatewayRateLimitHotspotSnapshotTrendReportView {
        matched_snapshots_count: points.len(),
        summary: build_rate_limit_hotspot_snapshot_trend_summary(&points),
        generated_at,
        filters,
        window_size,
        inventory_summary,
        points,
    }
}

fn build_rate_limit_hotspot_snapshot_trend_summary(
    points: &[GatewayRateLimitHotspotSnapshotTrendPointView],
) -> Option<GatewayRateLimitHotspotSnapshotTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayRateLimitHotspotSnapshotTrendSummaryView {
        latest_snapshot_id: Some(latest.snapshot.snapshot_id.clone()),
        previous_snapshot_id: previous.map(|value| value.snapshot.snapshot_id.clone()),
        total_rate_limited_requests: build_metric_summary(
            Some(latest.total_rate_limited_requests as f64),
            previous.map(|value| value.total_rate_limited_requests as f64),
        ),
        top_code_share: build_metric_summary(
            latest.top_code_share,
            previous.and_then(|value| value.top_code_share),
        ),
        top_project_share: build_metric_summary(
            latest.top_project_share,
            previous.and_then(|value| value.top_project_share),
        ),
        top_api_key_share: build_metric_summary(
            latest.top_api_key_share,
            previous.and_then(|value| value.top_api_key_share),
        ),
        top_requested_model_share: build_metric_summary(
            latest.top_requested_model_share,
            previous.and_then(|value| value.top_requested_model_share),
        ),
        top_endpoint_share: build_metric_summary(
            latest.top_endpoint_share,
            previous.and_then(|value| value.top_endpoint_share),
        ),
    })
}

fn current_time_ms() -> i128 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
}

fn parse_timestamp_ms(value: &str) -> Option<i128> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp_nanos() / 1_000_000)
}

fn format_timestamp_ms(value: i128) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(value * 1_000_000)
        .map(format_timestamp)
        .unwrap_or_else(|_| value.to_string())
}

fn route_trace_error_code(row: &GatewayRequestAuditView) -> Option<&str> {
    row.route_trace
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|object| object.get("errorCode"))
        .and_then(Value::as_str)
}

fn is_rate_limit_error_code(value: Option<&str>) -> bool {
    let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
    normalized.starts_with("rate_limit_exceeded") || normalized.starts_with("rate-limit")
}

fn accumulate_bucket(target: &mut BTreeMap<String, usize>, key: Option<&str>) {
    let Some(normalized) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    *target.entry(normalized.to_string()).or_insert(0) += 1;
}

fn into_key_buckets(map: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketKeyView> {
    let mut buckets = map
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    buckets
}

fn top_bucket_share(buckets: &[GatewaySummaryBucketKeyView], total: usize) -> Option<f64> {
    let count = buckets.first().map(|bucket| bucket.count)?;
    if total == 0 {
        return None;
    }
    Some(count as f64 / total as f64)
}

fn build_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayRateLimitHotspotMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta)) if previous != 0.0 => Some(delta / previous),
        _ => None,
    };
    GatewayRateLimitHotspotMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}

fn normalize_profile_key(value: Option<&str>) -> String {
    let normalized = value.unwrap_or("balanced").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "balanced" | "aggressive" | "conservative" => normalized,
        _ => "balanced".to_string(),
    }
}

fn rate_limit_hotspot_snapshot_prefix() -> &'static str {
    "ai-gateway/rate-limit-hotspot-snapshots"
}

fn rate_limit_hotspot_anomaly_snapshot_prefix() -> &'static str {
    "ai-gateway/rate-limit-hotspot-anomaly-snapshots"
}

fn build_rate_limit_hotspot_snapshot_object_key(snapshot_id: &str) -> String {
    format!(
        "{}/{}/snapshot.json",
        rate_limit_hotspot_snapshot_prefix(),
        snapshot_id.trim()
    )
}

fn build_rate_limit_hotspot_anomaly_snapshot_object_key(snapshot_id: &str) -> String {
    format!(
        "{}/{}/snapshot.json",
        rate_limit_hotspot_anomaly_snapshot_prefix(),
        snapshot_id.trim()
    )
}

async fn read_rate_limit_hotspot_snapshot<T>(object_key: &str) -> Result<T, GatewayError>
where
    T: DeserializeOwned,
{
    let payload = gateway_object_storage()?.read_json(object_key).await?;
    serde_json::from_value(payload).map_err(|error| {
        GatewayError::server_error(format!("parse hotspot snapshot payload: {error}"))
    })
}

fn matches_rate_limit_hotspot_snapshot_filters(
    snapshot: &GatewayRateLimitHotspotSnapshotView,
    filters: &GatewayRateLimitHotspotSnapshotFilters,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = trimmed_owned_ref(filters.snapshot_id.as_deref()) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = trimmed_owned_ref(filters.label.as_deref()) {
        let haystack = snapshot
            .label
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !haystack.contains(&label.to_ascii_lowercase()) {
            return false;
        }
    }
    if !matches_optional_filter(
        snapshot.filters.project_id.as_deref(),
        filters.project_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.route_policy_id.as_deref(),
        filters.route_policy_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.api_key_id.as_deref(),
        filters.api_key_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.endpoint_kind.as_deref(),
        filters.endpoint_kind.as_deref(),
    ) {
        return false;
    }
    matches_created_at_window(snapshot.created_at.as_str(), created_from, created_to)
}

fn matches_rate_limit_hotspot_anomaly_snapshot_filters(
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    filters: &GatewayRateLimitHotspotAnomalySnapshotFilters,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = trimmed_owned_ref(filters.snapshot_id.as_deref()) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = trimmed_owned_ref(filters.label.as_deref()) {
        let haystack = snapshot
            .label
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !haystack.contains(&label.to_ascii_lowercase()) {
            return false;
        }
    }
    if !matches_optional_filter(
        snapshot.filters.project_id.as_deref(),
        filters.project_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.route_policy_id.as_deref(),
        filters.route_policy_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.api_key_id.as_deref(),
        filters.api_key_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.endpoint_kind.as_deref(),
        filters.endpoint_kind.as_deref(),
    ) || !matches_optional_filter(
        Some(snapshot.filters.profile_key.as_str()),
        filters.profile_key.as_deref(),
    ) {
        return false;
    }
    matches_created_at_window(snapshot.created_at.as_str(), created_from, created_to)
}

fn matches_optional_filter(value: Option<&str>, filter: Option<&str>) -> bool {
    let Some(filter) = trimmed_owned_ref(filter) else {
        return true;
    };
    value.is_some_and(|current| current.trim() == filter)
}

fn matches_created_at_window(
    created_at: &str,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    let Some(created_at) = parse_filter_timestamp(Some(created_at), "createdAt")
        .ok()
        .flatten()
    else {
        return false;
    };
    if let Some(started_at) = created_from {
        if &created_at < started_at {
            return false;
        }
    }
    if let Some(ended_at) = created_to {
        if &created_at > ended_at {
            return false;
        }
    }
    true
}

fn merge_key_buckets(
    target: &mut BTreeMap<String, usize>,
    buckets: &[GatewaySummaryBucketKeyView],
) {
    for bucket in buckets {
        push_key_bucket(target, Some(bucket.key.as_str()), bucket.count);
    }
}

fn push_key_bucket(target: &mut BTreeMap<String, usize>, key: Option<&str>, count: usize) {
    let Some(normalized) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if count == 0 {
        return;
    }
    *target.entry(normalized.to_string()).or_insert(0) += count;
}

fn parse_filter_timestamp(
    value: Option<&str>,
    field: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = trimmed_owned_ref(value) else {
        return Ok(None);
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::conflict(format!("{field} 不是合法的 RFC3339 时间戳")))
}

fn normalize_lookback_hours(value: Option<i32>) -> Option<i32> {
    value.map(|hours| hours.clamp(0, 24 * 365))
}

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    trimmed_owned_ref(value).map(ToString::to_string)
}

fn trimmed_owned_ref(value: Option<&str>) -> Option<&str> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_rate_limit_hotspot_anomaly_report, build_rate_limit_hotspot_anomaly_thresholds,
        build_rate_limit_hotspot_snapshot_inventory_summary,
        build_rate_limit_hotspot_snapshot_trend_point,
        build_rate_limit_hotspot_snapshot_trend_report, build_rate_limit_hotspot_summary,
        build_rate_limit_hotspot_trend_report, GatewayRateLimitHotspotAnomalyOverrides,
        GatewayRateLimitHotspotFilterView, GatewayRateLimitHotspotSnapshotFilterView,
        GatewayRateLimitHotspotSnapshotReportFilterView, GatewayRateLimitHotspotSnapshotView,
    };
    use crate::db::request_audits::{GatewayRequestAuditView, GatewaySummaryBucketKeyView};

    fn base_audit() -> GatewayRequestAuditView {
        GatewayRequestAuditView {
            id: "audit-1".to_string(),
            project_id: "project-a".to_string(),
            api_key_id: Some("key-a".to_string()),
            user_credential_id: None,
            access_key_id: Some("access-key-a".to_string()),
            source_access_key_id: Some("access-key-a".to_string()),
            session_id: None,
            route_policy_id: Some("policy-1".to_string()),
            provider_account_id: None,
            protocol_family: "openai".to_string(),
            endpoint_kind: "chat.completions".to_string(),
            requested_model: Some("gpt-4".to_string()),
            resolved_model: Some("gpt-4".to_string()),
            model_alias: None,
            stream: false,
            status: "failed".to_string(),
            upstream_status: Some(429),
            duration_ms: Some(1000),
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            total_tokens: Some(2),
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            client_has_cache_control: false,
            auto_cache_applied: false,
            error_summary: Some("rate limit".to_string()),
            route_trace: Some(json!({ "errorCode": "rate_limit_exceeded_project" })),
            analysis_profile: None,
            request_artifact_object_key: None,
            response_artifact_object_key: None,
            response_id: "resp-1".to_string(),
            previous_response_id: None,
            client_disconnected_at: None,
            created_at: "2026-04-07T11:10:00Z".to_string(),
            completed_at: Some("2026-04-07T11:10:01Z".to_string()),
            updated_at: "2026-04-07T11:10:01Z".to_string(),
        }
    }

    fn create_audit(overrides: Value) -> GatewayRequestAuditView {
        let mut row = base_audit();
        if let Some(id) = overrides.get("id").and_then(Value::as_str) {
            row.id = id.to_string();
        }
        if let Some(project_id) = overrides.get("projectId").and_then(Value::as_str) {
            row.project_id = project_id.to_string();
        }
        if let Some(route_policy_id) = overrides.get("routePolicyId").and_then(Value::as_str) {
            row.route_policy_id = Some(route_policy_id.to_string());
        }
        if overrides.get("routePolicyId").is_some_and(Value::is_null) {
            row.route_policy_id = None;
        }
        if let Some(api_key_id) = overrides.get("apiKeyId").and_then(Value::as_str) {
            row.api_key_id = Some(api_key_id.to_string());
        }
        if let Some(requested_model) = overrides.get("requestedModel").and_then(Value::as_str) {
            row.requested_model = Some(requested_model.to_string());
        }
        if let Some(resolved_model) = overrides.get("resolvedModel").and_then(Value::as_str) {
            row.resolved_model = Some(resolved_model.to_string());
        }
        if let Some(endpoint_kind) = overrides.get("endpointKind").and_then(Value::as_str) {
            row.endpoint_kind = endpoint_kind.to_string();
        }
        if let Some(created_at) = overrides.get("createdAt").and_then(Value::as_str) {
            row.created_at = created_at.to_string();
        }
        if let Some(route_trace) = overrides.get("routeTrace") {
            row.route_trace = Some(route_trace.clone());
        }
        row
    }

    fn create_snapshot(
        snapshot_id: &str,
        created_at: &str,
        label: Option<&str>,
        total_rate_limited_requests: usize,
        code: &str,
        project: &str,
        api_key: &str,
        endpoint_kind: &str,
    ) -> GatewayRateLimitHotspotSnapshotView {
        GatewayRateLimitHotspotSnapshotView {
            snapshot_id: snapshot_id.to_string(),
            label: label.map(|value| value.to_string()),
            created_at: created_at.to_string(),
            object_key: format!(
                "ai-gateway/rate-limit-hotspot-snapshots/{snapshot_id}/snapshot.json"
            ),
            filters: GatewayRateLimitHotspotSnapshotFilterView {
                project_id: Some(project.to_string()),
                route_policy_id: Some("policy-a".to_string()),
                provider_account_id: None,
                session_id: None,
                api_key_id: Some(api_key.to_string()),
                response_id: None,
                protocol_family: None,
                endpoint_kind: Some(endpoint_kind.to_string()),
                error_code: None,
                created_from: None,
                created_to: None,
                limit: 1000,
                lookback_hours: Some(24),
            },
            summary: super::GatewayRateLimitHotspotSummaryView {
                total_rate_limited_requests,
                by_code: vec![GatewaySummaryBucketKeyView {
                    key: code.to_string(),
                    count: total_rate_limited_requests,
                }],
                by_project: vec![GatewaySummaryBucketKeyView {
                    key: project.to_string(),
                    count: total_rate_limited_requests,
                }],
                by_route_policy_id: vec![GatewaySummaryBucketKeyView {
                    key: "policy-a".to_string(),
                    count: total_rate_limited_requests,
                }],
                by_api_key_id: vec![GatewaySummaryBucketKeyView {
                    key: api_key.to_string(),
                    count: total_rate_limited_requests,
                }],
                by_requested_model: vec![GatewaySummaryBucketKeyView {
                    key: "gpt-4o".to_string(),
                    count: total_rate_limited_requests,
                }],
                by_resolved_model: vec![GatewaySummaryBucketKeyView {
                    key: "gpt-4o".to_string(),
                    count: total_rate_limited_requests,
                }],
                by_endpoint_kind: vec![GatewaySummaryBucketKeyView {
                    key: endpoint_kind.to_string(),
                    count: total_rate_limited_requests,
                }],
            },
        }
    }

    #[test]
    fn hotspot_summary_counts_only_rate_limit_rows() {
        let rows = vec![
            create_audit(
                json!({ "id": "audit-rl-1", "routeTrace": { "errorCode": "rate_limit_exceeded_project" } }),
            ),
            create_audit(
                json!({ "id": "audit-rl-2", "projectId": "project-b", "routePolicyId": "policy-2", "apiKeyId": "key-b", "endpointKind": "responses", "routeTrace": { "errorCode": "rate_limit_exceeded_endpoint" } }),
            ),
            create_audit(
                json!({ "id": "audit-legacy", "projectId": "project-b", "routePolicyId": "policy-2", "apiKeyId": "key-c", "endpointKind": "responses", "routeTrace": { "errorCode": "rate-limit-legacy" } }),
            ),
            create_audit(json!({ "id": "audit-nonrl", "routeTrace": { "errorCode": "timeout" } })),
        ];
        let summary = build_rate_limit_hotspot_summary(&rows);
        assert_eq!(summary.total_rate_limited_requests, 3);
        assert_eq!(
            summary
                .by_route_policy_id
                .first()
                .map(|bucket| bucket.key.as_str()),
            Some("policy-2")
        );
        assert_eq!(
            summary
                .by_endpoint_kind
                .first()
                .map(|bucket| bucket.key.as_str()),
            Some("responses")
        );
    }

    #[test]
    fn hotspot_trend_reports_latest_vs_previous_bucket() {
        let rows = vec![
            create_audit(json!({ "id": "latest-1", "createdAt": "2026-04-07T11:05:00Z" })),
            create_audit(
                json!({ "id": "latest-2", "endpointKind": "responses", "routeTrace": { "errorCode": "rate_limit_exceeded_endpoint" }, "createdAt": "2026-04-07T11:30:00Z" }),
            ),
            create_audit(
                json!({ "id": "previous-1", "projectId": "project-b", "apiKeyId": "key-b", "requestedModel": "gpt-4o", "resolvedModel": "gpt-4o", "endpointKind": "responses", "createdAt": "2026-04-07T10:15:00Z" }),
            ),
        ];
        let report = build_rate_limit_hotspot_trend_report(
            "2026-04-07T12:00:00Z".to_string(),
            GatewayRateLimitHotspotFilterView {
                project_id: None,
                route_policy_id: None,
                provider_account_id: None,
                session_id: None,
                api_key_id: None,
                response_id: None,
                protocol_family: None,
                endpoint_kind: None,
                error_code: None,
                created_from: None,
                created_to: None,
                limit: 1000,
                window_size: 3,
                bucket_size_minutes: 60,
            },
            &rows,
        );
        assert_eq!(report.matched_requests_count, 3);
        assert_eq!(report.points[0].bucket_start_at, "2026-04-07T11:00:00Z");
        assert_eq!(report.points[1].bucket_start_at, "2026-04-07T10:00:00Z");
        assert_eq!(
            report
                .summary
                .as_ref()
                .and_then(|summary| summary.total_rate_limited_requests.latest_value),
            Some(2.0)
        );
        assert_eq!(
            report
                .summary
                .as_ref()
                .and_then(|summary| summary.latest_top_api_key_key.as_deref()),
            Some("key-a")
        );
    }

    #[test]
    fn hotspot_anomaly_report_detects_spike_and_key_hotspot() {
        let rows = vec![
            create_audit(json!({ "id": "latest-1", "createdAt": "2026-04-07T11:01:00Z" })),
            create_audit(json!({ "id": "latest-2", "createdAt": "2026-04-07T11:02:00Z" })),
            create_audit(json!({ "id": "latest-3", "createdAt": "2026-04-07T11:03:00Z" })),
            create_audit(json!({ "id": "latest-4", "createdAt": "2026-04-07T11:04:00Z" })),
            create_audit(json!({ "id": "latest-5", "createdAt": "2026-04-07T11:05:00Z" })),
            create_audit(
                json!({ "id": "previous-1", "apiKeyId": "key-b", "createdAt": "2026-04-07T10:10:00Z" }),
            ),
        ];
        let trend = build_rate_limit_hotspot_trend_report(
            "2026-04-07T12:00:00Z".to_string(),
            GatewayRateLimitHotspotFilterView {
                project_id: None,
                route_policy_id: None,
                provider_account_id: None,
                session_id: None,
                api_key_id: None,
                response_id: None,
                protocol_family: None,
                endpoint_kind: None,
                error_code: None,
                created_from: None,
                created_to: None,
                limit: 1000,
                window_size: 2,
                bucket_size_minutes: 60,
            },
            &rows,
        );
        let report = build_rate_limit_hotspot_anomaly_report(
            "2026-04-07T12:00:00Z".to_string(),
            trend,
            "aggressive".to_string(),
            build_rate_limit_hotspot_anomaly_thresholds(
                "aggressive",
                GatewayRateLimitHotspotAnomalyOverrides {
                    total_rate_limited_requests_warning_threshold: Some(4.0),
                    total_rate_limited_requests_critical_threshold: Some(5.0),
                    total_rate_limited_requests_delta_ratio_threshold: Some(0.1),
                    top_api_key_share_warning_threshold: Some(0.7),
                    top_api_key_share_critical_threshold: Some(0.9),
                    ..Default::default()
                },
            ),
        );
        assert!(report
            .anomalies
            .iter()
            .any(|item| item.code == "rate_limit_request_spike"));
        let api_key_hotspot = report
            .anomalies
            .iter()
            .find(|item| item.code == "rate_limit_api_key_hotspot");
        assert_eq!(
            api_key_hotspot.and_then(|item| item.entity_key.as_deref()),
            Some("key-a")
        );
        assert_eq!(
            api_key_hotspot.map(|item| item.severity.as_str()),
            Some("critical")
        );
    }

    #[test]
    fn hotspot_snapshot_inventory_merges_snapshot_buckets() {
        let summary = build_rate_limit_hotspot_snapshot_inventory_summary(&[
            create_snapshot(
                "snapshot-1",
                "2026-04-07T12:00:00Z",
                Some("daily"),
                5,
                "rate_limit_exceeded_api_key",
                "project-a",
                "key-a",
                "responses",
            ),
            create_snapshot(
                "snapshot-2",
                "2026-04-08T12:00:00Z",
                Some("weekly"),
                2,
                "rate_limit_exceeded_model",
                "project-b",
                "key-b",
                "chat.completions",
            ),
        ]);
        assert_eq!(summary.total_snapshots, 2);
        assert_eq!(summary.total_rate_limited_requests, 7);
        assert_eq!(
            summary.by_label.first().map(|bucket| bucket.key.as_str()),
            Some("daily")
        );
        assert_eq!(
            summary
                .by_project
                .iter()
                .find(|bucket| bucket.key == "project-b")
                .map(|bucket| bucket.count),
            Some(2)
        );
    }

    #[test]
    fn hotspot_snapshot_trend_report_summarizes_latest_vs_previous() {
        let snapshots = vec![
            create_snapshot(
                "snapshot-1",
                "2026-04-08T12:00:00Z",
                Some("daily"),
                10,
                "rate_limit_exceeded_api_key",
                "project-a",
                "key-a",
                "responses",
            ),
            create_snapshot(
                "snapshot-2",
                "2026-04-07T12:00:00Z",
                Some("daily"),
                4,
                "rate_limit_exceeded_model",
                "project-a",
                "key-a",
                "responses",
            ),
        ];
        let report = build_rate_limit_hotspot_snapshot_trend_report(
            "2026-04-08T13:00:00Z".to_string(),
            GatewayRateLimitHotspotSnapshotReportFilterView {
                label: Some("daily".to_string()),
                project_id: Some("project-a".to_string()),
                route_policy_id: Some("policy-a".to_string()),
                api_key_id: Some("key-a".to_string()),
                endpoint_kind: Some("responses".to_string()),
                created_from: None,
                created_to: None,
            },
            10,
            build_rate_limit_hotspot_snapshot_inventory_summary(&snapshots),
            snapshots
                .into_iter()
                .map(build_rate_limit_hotspot_snapshot_trend_point)
                .collect(),
        );
        assert_eq!(report.matched_snapshots_count, 2);
        assert_eq!(
            report
                .summary
                .as_ref()
                .and_then(|item| item.latest_snapshot_id.as_deref()),
            Some("snapshot-1")
        );
        assert_eq!(
            report
                .summary
                .as_ref()
                .and_then(|item| item.total_rate_limited_requests.latest_value),
            Some(10.0)
        );
        assert_eq!(
            report
                .summary
                .as_ref()
                .and_then(|item| item.total_rate_limited_requests.previous_value),
            Some(4.0)
        );
    }
}
