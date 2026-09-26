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

mod aggregation;
mod anomalies;
mod anomaly_snapshots;
mod metrics;
mod reports;
mod snapshot_aggregation;
mod snapshot_storage;
mod snapshots;

pub(crate) use aggregation::{
    build_rate_limit_hotspot_summary, build_rate_limit_hotspot_trend_report,
};
pub(crate) use anomalies::{
    build_rate_limit_hotspot_anomaly_report, build_rate_limit_hotspot_anomaly_thresholds,
};
pub use anomaly_snapshots::{
    get_rate_limit_hotspot_anomaly_snapshot, list_rate_limit_hotspot_anomaly_snapshots,
    persist_rate_limit_hotspot_anomaly_snapshot,
};
pub use reports::{
    get_rate_limit_hotspot_anomaly_report, get_rate_limit_hotspot_trend_report,
    summarize_rate_limit_hotspots,
};
pub(crate) use snapshot_aggregation::{
    build_rate_limit_hotspot_snapshot_inventory_summary,
    build_rate_limit_hotspot_snapshot_trend_point, build_rate_limit_hotspot_snapshot_trend_report,
};
pub use snapshots::{
    get_rate_limit_hotspot_snapshot, get_rate_limit_hotspot_snapshot_trend_report,
    list_rate_limit_hotspot_snapshots, persist_rate_limit_hotspot_snapshot,
    summarize_rate_limit_hotspot_snapshot_inventory,
};

use anomalies::normalize_profile_key;
use metrics::{
    accumulate_bucket, build_metric_summary, into_key_buckets, merge_key_buckets, push_key_bucket,
    top_bucket_share,
};
use snapshot_storage::{
    build_rate_limit_hotspot_anomaly_snapshot_object_key,
    build_rate_limit_hotspot_snapshot_object_key,
    matches_rate_limit_hotspot_anomaly_snapshot_filters,
    matches_rate_limit_hotspot_snapshot_filters, normalize_lookback_hours, parse_filter_timestamp,
    rate_limit_hotspot_anomaly_snapshot_prefix, rate_limit_hotspot_snapshot_prefix,
    read_rate_limit_hotspot_snapshot, trimmed_owned, trimmed_owned_ref,
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

#[cfg(test)]
mod tests;
