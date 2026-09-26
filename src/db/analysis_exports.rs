use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::GatewayError;
use crate::object_storage::{
    build_gateway_analysis_export_dataset_object_key,
    build_gateway_analysis_export_manifest_object_key, build_gateway_analysis_export_prefix,
    gateway_object_storage,
};

use super::remediation::{list_anomaly_policies, GatewayAnalysisAnomalyPolicyFilters};
use super::request_audits::{
    list_analysis_samples, GatewayAnalysisSampleView, GatewaySummaryBucketKeyView,
    RequestAuditFilters,
};
use super::{format_timestamp, map_db_error};

mod anomalies;
mod cleanup;
mod dataset;
mod diff;
mod export;
mod filters;
mod inventory;
mod manifest;
mod messages;
mod metadata;
mod normalization;
mod queries;
mod record_views;
mod reports;
mod text;
mod thresholds;
mod trends;

pub use anomalies::get_analysis_export_anomaly_report;
pub use cleanup::cleanup_expired_analysis_exports;
pub use export::{export_analysis_rows, persist_analysis_export};
pub use metadata::update_persisted_analysis_export_metadata;
pub use queries::{
    get_persisted_analysis_export, list_persisted_analysis_exports,
    summarize_persisted_analysis_exports,
};
pub use reports::{
    get_analysis_export_baseline_report, get_analysis_export_timeline_report,
    get_analysis_export_trend_report, get_persisted_analysis_export_diff,
};

#[cfg(test)]
use filters::matches_persisted_analysis_export_view_filters;
#[cfg(test)]
use inventory::build_analysis_export_inventory_summary;
#[cfg(test)]
use manifest::{build_analysis_export_file_view, build_manifest_artifacts};
#[cfg(test)]
use normalization::normalize_export_tags;
#[cfg(test)]
use text::{apply_text_mode, redact_sensitive_text};

const DEFAULT_TEXT_MODE: &str = "preview_redacted";
const DEFAULT_MAX_TEXT_CHARS: usize = 4_000;
const MAX_TEXT_CHARS_LIMIT: usize = 32_000;
const DEFAULT_EXPORT_LIMIT: usize = 200;
const MAX_EXPORT_LIMIT: usize = 500;
const MAX_TAGS: usize = 32;
const MAX_TAG_LENGTH: usize = 40;
const MAX_LABEL_LENGTH: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportMessageView {
    pub role: String,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub text: String,
    pub tool_call_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportRowView {
    pub request_audit_id: String,
    pub response_id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: String,
    pub stream: bool,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub request_artifact_available: bool,
    pub response_artifact_available: bool,
    pub analysis_profile: Option<Value>,
    pub route_trace: Option<Value>,
    pub request_text: Option<String>,
    pub response_text: Option<String>,
    pub request_text_truncated: bool,
    pub response_text_truncated: bool,
    pub request_messages: Vec<GatewayAnalysisExportMessageView>,
    pub request_tool_names: Vec<String>,
    pub response_tool_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportView {
    pub text_mode: String,
    pub max_text_chars: usize,
    pub sample_count: usize,
    pub request_artifact_count: usize,
    pub response_artifact_count: usize,
    pub rows: Vec<GatewayAnalysisExportRowView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportFileView {
    pub kind: String,
    pub object_key: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub sha256: String,
    pub line_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportFilterView {
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
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub artifact_available: Option<bool>,
    pub limit: usize,
    pub text_mode: String,
    pub max_text_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportManifest {
    pub schema_version: i32,
    pub export_id: String,
    pub label: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub retention_expires_at: Option<String>,
    pub filters: GatewayAnalysisExportFilterView,
    pub sample_count: usize,
    pub request_artifact_count: usize,
    pub response_artifact_count: usize,
    pub files: Vec<GatewayAnalysisExportFileView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPersistedAnalysisExportView {
    pub export_id: String,
    pub label: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub object_prefix: String,
    pub filters: GatewayAnalysisExportFilterView,
    pub sample_count: usize,
    pub request_artifact_count: usize,
    pub response_artifact_count: usize,
    pub retention_expires_at: Option<String>,
    pub cleaned_up_at: Option<String>,
    pub last_cleanup_error: Option<String>,
    pub files: Vec<GatewayAnalysisExportFileView>,
    pub manifest: GatewayAnalysisExportManifest,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPersistedAnalysisExportFilters {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportInventorySummaryView {
    pub total_exports: usize,
    pub active_exports: usize,
    pub deleted_exports: usize,
    pub pinned_exports: usize,
    pub expiring_within_24_hours: usize,
    pub expired_active_exports: usize,
    pub total_sample_count: usize,
    pub total_request_artifact_count: usize,
    pub total_response_artifact_count: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_text_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_tag: Vec<GatewaySummaryBucketKeyView>,
    pub by_project: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportMetricDeltaView {
    pub left_value: Option<f64>,
    pub right_value: Option<f64>,
    pub delta_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportBucketDeltaView {
    pub key: String,
    pub left_count: usize,
    pub right_count: usize,
    pub delta_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportDiffView {
    pub left_export: GatewayPersistedAnalysisExportView,
    pub right_export: GatewayPersistedAnalysisExportView,
    pub overlap_request_count: usize,
    pub left_only_request_count: usize,
    pub right_only_request_count: usize,
    pub sample_count: GatewayAnalysisExportMetricDeltaView,
    pub request_artifact_count: GatewayAnalysisExportMetricDeltaView,
    pub response_artifact_count: GatewayAnalysisExportMetricDeltaView,
    pub prompt_tokens: GatewayAnalysisExportMetricDeltaView,
    pub completion_tokens: GatewayAnalysisExportMetricDeltaView,
    pub total_tokens: GatewayAnalysisExportMetricDeltaView,
    pub by_status: Vec<GatewayAnalysisExportBucketDeltaView>,
    pub by_protocol_family: Vec<GatewayAnalysisExportBucketDeltaView>,
    pub by_endpoint_kind: Vec<GatewayAnalysisExportBucketDeltaView>,
    pub by_resolved_model: Vec<GatewayAnalysisExportBucketDeltaView>,
    pub by_provider_account: Vec<GatewayAnalysisExportBucketDeltaView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportBaselineReportFilterView {
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportBaselineReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisExportBaselineReportFilterView,
    pub matched_exports_count: usize,
    pub latest_export: Option<GatewayPersistedAnalysisExportView>,
    pub previous_export: Option<GatewayPersistedAnalysisExportView>,
    pub inventory_summary: GatewayAnalysisExportInventorySummaryView,
    pub diff: Option<GatewayAnalysisExportDiffView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTimelinePairView {
    pub newer_export: GatewayPersistedAnalysisExportView,
    pub older_export: GatewayPersistedAnalysisExportView,
    pub diff: Option<GatewayAnalysisExportDiffView>,
    pub diff_unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTimelineReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisExportBaselineReportFilterView,
    pub matched_exports_count: usize,
    pub window_size: usize,
    pub exports: Vec<GatewayPersistedAnalysisExportView>,
    pub inventory_summary: GatewayAnalysisExportInventorySummaryView,
    pub pair_comparisons: Vec<GatewayAnalysisExportTimelinePairView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTrendPointView {
    pub export: GatewayPersistedAnalysisExportView,
    pub dataset_available: bool,
    pub dataset_unavailable_reason: Option<String>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub stream_samples: Option<i64>,
    pub completed_samples: Option<i64>,
    pub failed_samples: Option<i64>,
    pub cancelled_samples: Option<i64>,
    pub tool_request_samples: Option<i64>,
    pub tool_response_samples: Option<i64>,
    pub system_prompt_samples: Option<i64>,
    pub reasoning_samples: Option<i64>,
    pub metadata_samples: Option<i64>,
    pub explicit_session_samples: Option<i64>,
    pub previous_response_samples: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTrendMetricSummaryView {
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTrendSummaryView {
    pub latest_export_id: Option<String>,
    pub previous_export_id: Option<String>,
    pub prompt_tokens_per_sample: GatewayAnalysisExportTrendMetricSummaryView,
    pub completion_tokens_per_sample: GatewayAnalysisExportTrendMetricSummaryView,
    pub total_tokens_per_sample: GatewayAnalysisExportTrendMetricSummaryView,
    pub request_artifact_coverage: GatewayAnalysisExportTrendMetricSummaryView,
    pub response_artifact_coverage: GatewayAnalysisExportTrendMetricSummaryView,
    pub stream_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub completion_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub failure_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub cancellation_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub tool_request_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub tool_response_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub reasoning_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub metadata_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub explicit_session_rate: GatewayAnalysisExportTrendMetricSummaryView,
    pub previous_response_rate: GatewayAnalysisExportTrendMetricSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportTrendReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisExportBaselineReportFilterView,
    pub matched_exports_count: usize,
    pub window_size: usize,
    pub inventory_summary: GatewayAnalysisExportInventorySummaryView,
    pub points: Vec<GatewayAnalysisExportTrendPointView>,
    pub summary: Option<GatewayAnalysisExportTrendSummaryView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportAnomalyThresholdConfig {
    pub failure_rate_warning_threshold: f64,
    pub failure_rate_critical_threshold: f64,
    pub failure_rate_delta_ratio_threshold: f64,
    pub completion_rate_warning_threshold: f64,
    pub completion_rate_critical_threshold: f64,
    pub completion_rate_delta_value_threshold: f64,
    pub response_artifact_coverage_warning_threshold: f64,
    pub response_artifact_coverage_critical_threshold: f64,
    pub response_artifact_coverage_delta_value_threshold: f64,
    pub request_artifact_coverage_warning_threshold: f64,
    pub request_artifact_coverage_critical_threshold: f64,
    pub request_artifact_coverage_delta_value_threshold: f64,
    pub tokens_per_sample_warning_delta_ratio_threshold: f64,
    pub tokens_per_sample_critical_delta_ratio_threshold: f64,
    pub tokens_per_sample_critical_absolute_threshold: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportAnomalyOverrides {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub latest_export_id: Option<String>,
    pub previous_export_id: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisExportBaselineReportFilterView,
    pub profile_key: String,
    pub thresholds: GatewayAnalysisExportAnomalyThresholdConfig,
    pub latest_export: Option<GatewayPersistedAnalysisExportView>,
    pub previous_export: Option<GatewayPersistedAnalysisExportView>,
    pub trend_summary: Option<GatewayAnalysisExportTrendSummaryView>,
    pub anomalies: Vec<GatewayAnalysisExportAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default)]
pub struct PersistGatewayAnalysisExportInput {
    pub label: Option<String>,
    pub tags: Option<Vec<String>>,
    pub retention_expires_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportMetadataUpdateInput {
    #[serde(default)]
    pub label: Option<Option<String>>,
    #[serde(default)]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub retention_expires_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportCleanupInput {
    pub limit: Option<usize>,
    pub include_pinned: Option<bool>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportCleanupEntryView {
    pub export_id: String,
    pub status: String,
    pub deleted_object_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisExportCleanupResult {
    pub scanned_count: usize,
    pub deleted_count: usize,
    pub failed_count: usize,
    pub results: Vec<GatewayAnalysisExportCleanupEntryView>,
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
struct GatewayAnalysisExportRow {
    id: String,
    project_id: Option<String>,
    label: Option<String>,
    tags: Json<Vec<String>>,
    status: String,
    text_mode: String,
    max_text_chars: i32,
    filters: Json<Value>,
    object_prefix: String,
    manifest_object_key: String,
    dataset_object_key: String,
    sample_count: i32,
    request_artifact_count: i32,
    response_artifact_count: i32,
    retention_expires_at: Option<OffsetDateTime>,
    cleaned_up_at: Option<OffsetDateTime>,
    last_cleanup_error: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

struct ManifestArtifacts {
    manifest: GatewayAnalysisExportManifest,
    manifest_body: Vec<u8>,
}

struct AppliedTextMode {
    text: Option<String>,
    truncated: bool,
}

#[cfg(test)]
mod tests;
