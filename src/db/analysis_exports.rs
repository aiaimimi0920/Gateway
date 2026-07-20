use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use hex::encode as hex_encode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

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

pub async fn list_persisted_analysis_exports(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<Vec<GatewayPersistedAnalysisExportView>, GatewayError> {
    let created_from = parse_optional_rfc3339(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_rfc3339(filters.created_to.as_deref(), "createdTo")?;
    validate_created_range(created_from, created_to)?;
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let rows = query_persisted_analysis_export_rows(
        pool,
        filters,
        created_from,
        created_to,
        Some(limit * 2),
    )
    .await?;
    let matched_rows = rows
        .into_iter()
        .filter(|row| {
            matches_persisted_analysis_export_filters(row, filters, created_from, created_to)
        })
        .collect::<Vec<_>>();

    if !matched_rows.is_empty() {
        let mut exports = Vec::with_capacity(matched_rows.len().min(limit));
        for row in matched_rows.into_iter().take(limit) {
            let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
            exports.push(to_persisted_analysis_export_view(row, manifest)?);
        }
        return Ok(exports);
    }

    if is_non_active_status_filter(filters.status.as_deref())
        || trimmed_owned_ref_opt(filters.tag.as_deref()).is_some()
    {
        return Ok(Vec::new());
    }

    let mut fallback =
        list_fallback_analysis_export_manifests(filters, created_from, created_to).await?;
    fallback.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    fallback.truncate(limit);
    Ok(fallback)
}

pub async fn get_persisted_analysis_export(
    pool: &PgPool,
    export_id: &str,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let Some(export_id) = trimmed_owned_ref(export_id) else {
        return Err(GatewayError::conflict("exportId 不能为空。"));
    };
    let row = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where id = $1
        limit 1
        "#,
    )
    .bind(export_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    if let Some(row) = row {
        let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
        return to_persisted_analysis_export_view(row, manifest);
    }

    let manifest_object_key = build_gateway_analysis_export_manifest_object_key(export_id);
    let manifest = try_read_analysis_export_manifest(&manifest_object_key)
        .await
        .ok_or_else(|| GatewayError::not_found("Gateway analysis export 不存在。"))?;
    Ok(to_manifest_backed_persisted_analysis_export_view(manifest))
}

pub async fn summarize_persisted_analysis_exports(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportInventorySummaryView, GatewayError> {
    let created_from = parse_optional_rfc3339(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_rfc3339(filters.created_to.as_deref(), "createdTo")?;
    validate_created_range(created_from, created_to)?;
    let rows =
        query_persisted_analysis_export_rows(pool, filters, created_from, created_to, None).await?;
    let row_count = rows.len();

    let persisted = rows
        .into_iter()
        .filter(|row| {
            matches_persisted_analysis_export_filters(row, filters, created_from, created_to)
        })
        .map(|row| to_persisted_analysis_export_view(row, None))
        .collect::<Result<Vec<_>, _>>()?;

    if !persisted.is_empty()
        || row_count > 0
        || trimmed_owned_ref_opt(filters.status.as_deref()).is_some()
        || trimmed_owned_ref_opt(filters.tag.as_deref()).is_some()
    {
        return Ok(build_analysis_export_inventory_summary(
            &persisted,
            OffsetDateTime::now_utc(),
        ));
    }

    let fallback =
        list_fallback_analysis_export_manifests(filters, created_from, created_to).await?;
    Ok(build_analysis_export_inventory_summary(
        &fallback,
        OffsetDateTime::now_utc(),
    ))
}

pub async fn update_persisted_analysis_export_metadata(
    pool: &PgPool,
    export_id: &str,
    input: GatewayAnalysisExportMetadataUpdateInput,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let Some(export_id) = trimmed_owned_ref(export_id) else {
        return Err(GatewayError::conflict("exportId 不能为空。"));
    };
    let row = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where id = $1
        limit 1
        "#,
    )
    .bind(export_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Gateway analysis export 不存在。"))?;

    if row.status == "deleted" {
        return Err(GatewayError::conflict(
            "已删除的 export 不允许继续修改 metadata。",
        ));
    }

    let next_label = match input.label {
        Some(value) => normalize_export_label(value.as_deref())?,
        None => row.label.clone(),
    };
    let next_tags = match input.tags {
        Some(value) => normalize_export_tags(value.as_ref())?,
        None => normalize_tag_values(&row.tags.0),
    };
    let next_retention_expires_at = match input.retention_expires_at {
        Some(value) => parse_optional_rfc3339(value.as_deref(), "retentionExpiresAt")?,
        None => row.retention_expires_at,
    };

    let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
    let dataset_file = manifest
        .as_ref()
        .and_then(|item| {
            item.files
                .iter()
                .find(|file| file.kind == "dataset_jsonl")
                .cloned()
        })
        .unwrap_or_else(|| GatewayAnalysisExportFileView {
            kind: "dataset_jsonl".to_string(),
            object_key: row.dataset_object_key.clone(),
            content_type: "application/x-ndjson".to_string(),
            size_bytes: 0,
            sha256: String::new(),
            line_count: Some(usize::try_from(row.sample_count).unwrap_or_default()),
        });
    let filters = serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
        .map_err(|error| {
            GatewayError::server_error(format!("parse analysis export filters: {error}"))
        })?;
    let manifest_artifacts = build_manifest_artifacts(
        &row.id,
        next_label.clone(),
        next_tags.clone(),
        format_timestamp(row.created_at),
        next_retention_expires_at.map(format_timestamp),
        filters,
        usize::try_from(row.sample_count).unwrap_or_default(),
        usize::try_from(row.request_artifact_count).unwrap_or_default(),
        usize::try_from(row.response_artifact_count).unwrap_or_default(),
        dataset_file,
        row.manifest_object_key.clone(),
    )?;
    gateway_object_storage()?
        .put_bytes(
            &row.manifest_object_key,
            manifest_artifacts.manifest_body.clone(),
            "application/json",
        )
        .await?;

    let updated_at = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_exports
        set label = $2,
            tags = $3,
            retention_expires_at = $4,
            updated_at = $5
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_label.as_deref())
    .bind(Json(next_tags.clone()))
    .bind(next_retention_expires_at)
    .bind(updated_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated_row = GatewayAnalysisExportRow {
        label: next_label,
        tags: Json(next_tags),
        retention_expires_at: next_retention_expires_at,
        updated_at,
        ..row
    };
    to_persisted_analysis_export_view(updated_row, Some(manifest_artifacts.manifest))
}

pub async fn cleanup_expired_analysis_exports(
    pool: &PgPool,
    input: GatewayAnalysisExportCleanupInput,
) -> Result<GatewayAnalysisExportCleanupResult, GatewayError> {
    let limit = input.limit.unwrap_or(50).clamp(1, 500);
    let scan_time = OffsetDateTime::now_utc();
    let rows = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where status = 'active'
          and retention_expires_at is not null
          and retention_expires_at <= $1
        order by retention_expires_at asc, created_at asc
        limit $2
        "#,
    )
    .bind(scan_time)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    let include_pinned = input.include_pinned.unwrap_or(false);
    let dry_run = input.dry_run.unwrap_or(false);
    let rows = rows
        .into_iter()
        .filter(|row| include_pinned || !has_pinned_tag(&row.tags.0))
        .take(limit)
        .collect::<Vec<_>>();
    let storage = gateway_object_storage()?;
    let mut results = Vec::with_capacity(rows.len());

    for row in &rows {
        let listed_keys = storage
            .list_objects(&row.object_prefix)
            .await
            .unwrap_or_default();
        let mut keys_to_delete = vec![
            row.manifest_object_key.clone(),
            row.dataset_object_key.clone(),
        ];
        keys_to_delete.extend(listed_keys);
        keys_to_delete.sort();
        keys_to_delete.dedup();
        keys_to_delete.retain(|value| trimmed_owned_ref(value).is_some());

        if dry_run {
            results.push(GatewayAnalysisExportCleanupEntryView {
                export_id: row.id.clone(),
                status: "deleted".to_string(),
                deleted_object_count: keys_to_delete.len(),
                error_message: None,
            });
            continue;
        }

        let mut error_message = None;
        for object_key in &keys_to_delete {
            if let Err(error) = storage.delete_object(object_key).await {
                error_message = Some(truncate_error_message(&error.message, 500));
                break;
            }
        }

        if let Some(error_message) = error_message {
            sqlx::query(
                r#"
                update gateway_analysis_exports
                set last_cleanup_error = $2,
                    updated_at = $3
                where id = $1
                "#,
            )
            .bind(&row.id)
            .bind(&error_message)
            .bind(scan_time)
            .execute(pool)
            .await
            .map_err(map_db_error)?;
            results.push(GatewayAnalysisExportCleanupEntryView {
                export_id: row.id.clone(),
                status: "failed".to_string(),
                deleted_object_count: 0,
                error_message: Some(error_message),
            });
            continue;
        }

        sqlx::query(
            r#"
            update gateway_analysis_exports
            set status = 'deleted',
                cleaned_up_at = $2,
                last_cleanup_error = null,
                updated_at = $2
            where id = $1
            "#,
        )
        .bind(&row.id)
        .bind(scan_time)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
        results.push(GatewayAnalysisExportCleanupEntryView {
            export_id: row.id.clone(),
            status: "deleted".to_string(),
            deleted_object_count: keys_to_delete.len(),
            error_message: None,
        });
    }

    Ok(GatewayAnalysisExportCleanupResult {
        scanned_count: rows.len(),
        deleted_count: results
            .iter()
            .filter(|entry| entry.status == "deleted")
            .count(),
        failed_count: results
            .iter()
            .filter(|entry| entry.status == "failed")
            .count(),
        results,
    })
}

pub async fn get_persisted_analysis_export_diff(
    pool: &PgPool,
    left_export_id: &str,
    right_export_id: &str,
) -> Result<GatewayAnalysisExportDiffView, GatewayError> {
    let Some(left_export_id) = trimmed_owned_ref(left_export_id) else {
        return Err(GatewayError::conflict(
            "leftExportId 与 rightExportId 都不能为空。",
        ));
    };
    let Some(right_export_id) = trimmed_owned_ref(right_export_id) else {
        return Err(GatewayError::conflict(
            "leftExportId 与 rightExportId 都不能为空。",
        ));
    };
    let left_export = get_persisted_analysis_export(pool, left_export_id).await?;
    let right_export = get_persisted_analysis_export(pool, right_export_id).await?;
    build_analysis_export_diff_for_views(&left_export, &right_export).await
}

pub async fn get_analysis_export_baseline_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportBaselineReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 2, 50, 10);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let diff = if exports.len() >= 2 {
        build_analysis_export_diff_for_views(&exports[1], &exports[0])
            .await
            .ok()
    } else {
        None
    };
    Ok(GatewayAnalysisExportBaselineReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count: exports.len(),
        latest_export: exports.first().cloned(),
        previous_export: exports.get(1).cloned(),
        inventory_summary,
        diff,
    })
}

pub async fn get_analysis_export_timeline_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportTimelineReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 2, 20, 5);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let mut pair_comparisons = Vec::new();
    for index in 0..exports.len().saturating_sub(1) {
        let Some(newer_export) = exports.get(index).cloned() else {
            continue;
        };
        let Some(older_export) = exports.get(index + 1).cloned() else {
            continue;
        };
        match build_analysis_export_diff_for_views(&older_export, &newer_export).await {
            Ok(diff) => pair_comparisons.push(GatewayAnalysisExportTimelinePairView {
                newer_export,
                older_export,
                diff: Some(diff),
                diff_unavailable_reason: None,
            }),
            Err(error) => pair_comparisons.push(GatewayAnalysisExportTimelinePairView {
                newer_export,
                older_export,
                diff: None,
                diff_unavailable_reason: Some(truncate_error_message(&error.message, 240)),
            }),
        }
    }
    Ok(GatewayAnalysisExportTimelineReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count: exports.len(),
        window_size: normalized_filters.limit.unwrap_or(5),
        exports,
        inventory_summary,
        pair_comparisons,
    })
}

pub async fn get_analysis_export_trend_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportTrendReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 1, 50, 10);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let mut points = Vec::with_capacity(exports.len());
    for export in &exports {
        points.push(build_analysis_export_trend_point(export).await);
    }
    let matched_exports_count = points.len();
    let window_size = normalized_filters.limit.unwrap_or(10);
    Ok(GatewayAnalysisExportTrendReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count,
        window_size,
        inventory_summary,
        summary: build_analysis_export_trend_summary(&points),
        points,
    })
}

pub async fn get_analysis_export_anomaly_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    policy_id: Option<&str>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisExportAnomalyOverrides,
) -> Result<GatewayAnalysisExportAnomalyReportView, GatewayError> {
    let context =
        resolve_analysis_export_anomaly_context(pool, filters, policy_id, profile_key, overrides)
            .await?;
    let trend_report = get_analysis_export_trend_report(pool, &context.filters).await?;
    Ok(build_analysis_export_anomaly_report(
        trend_report,
        context.profile_key,
        context.thresholds,
    ))
}

struct AnalysisExportAnomalyContext {
    filters: GatewayPersistedAnalysisExportFilters,
    profile_key: String,
    thresholds: GatewayAnalysisExportAnomalyThresholdConfig,
}

async fn resolve_analysis_export_anomaly_context(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    policy_id: Option<&str>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisExportAnomalyOverrides,
) -> Result<AnalysisExportAnomalyContext, GatewayError> {
    let policy = if let Some(policy_id) = trimmed_owned_ref_opt(policy_id) {
        let policies = list_anomaly_policies(
            pool,
            &GatewayAnalysisAnomalyPolicyFilters {
                policy_id: Some(policy_id.to_string()),
                limit: Some(1),
                ..GatewayAnalysisAnomalyPolicyFilters::default()
            },
        )
        .await?;
        let Some(policy) = policies.into_iter().next() else {
            return Err(GatewayError::not_found(
                "Gateway analysis anomaly policy 不存在。",
            ));
        };
        Some(policy)
    } else {
        None
    };

    if let (Some(request_project_id), Some(policy)) = (
        trimmed_owned_ref_opt(filters.project_id.as_deref()),
        policy.as_ref(),
    ) {
        if let Some(policy_project_id) = trimmed_owned_ref_opt(policy.project_id.as_deref()) {
            if request_project_id != policy_project_id {
                return Err(GatewayError::conflict(
                    "filters.projectId 与 anomaly policy 绑定的 project 不一致。",
                ));
            }
        }
    }

    let policy_profile_key = policy.as_ref().map(|item| item.profile_key.as_str());
    let resolved_profile_key =
        normalize_analysis_export_profile_key(profile_key.or(policy_profile_key)).to_string();
    let policy_thresholds = policy.as_ref().and_then(|item| {
        serde_json::from_value::<GatewayAnalysisExportAnomalyThresholdConfig>(
            item.thresholds.clone(),
        )
        .ok()
    });
    let thresholds = build_analysis_export_anomaly_threshold_config(
        &resolved_profile_key,
        policy_thresholds,
        &overrides,
    )?;

    Ok(AnalysisExportAnomalyContext {
        filters: GatewayPersistedAnalysisExportFilters {
            export_id: None,
            label: filters.label.clone(),
            tag: filters
                .tag
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.tag.clone())),
            project_id: filters
                .project_id
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.project_id.clone())),
            status: Some(
                trimmed_owned_ref_opt(filters.status.as_deref())
                    .unwrap_or("active")
                    .to_string(),
            ),
            text_mode: filters
                .text_mode
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.text_mode.clone())),
            created_from: filters.created_from.clone(),
            created_to: filters.created_to.clone(),
            limit: filters.limit,
        },
        profile_key: resolved_profile_key,
        thresholds,
    })
}

async fn build_analysis_export_diff_for_views(
    left_export: &GatewayPersistedAnalysisExportView,
    right_export: &GatewayPersistedAnalysisExportView,
) -> Result<GatewayAnalysisExportDiffView, GatewayError> {
    let left_rows = read_analysis_export_dataset_for_export(left_export)
        .await
        .map_err(|_| {
            GatewayError::conflict(format!(
                "leftExportId={} 的 dataset.jsonl 不可用，无法执行 diff。",
                left_export.export_id
            ))
        })?;
    let right_rows = read_analysis_export_dataset_for_export(right_export)
        .await
        .map_err(|_| {
            GatewayError::conflict(format!(
                "rightExportId={} 的 dataset.jsonl 不可用，无法执行 diff。",
                right_export.export_id
            ))
        })?;
    Ok(build_analysis_export_diff(
        left_export.clone(),
        right_export.clone(),
        &left_rows,
        &right_rows,
    ))
}

pub async fn export_analysis_rows(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    text_mode: Option<&str>,
    max_text_chars: Option<usize>,
) -> Result<GatewayAnalysisExportView, GatewayError> {
    let normalized_text_mode = normalize_text_mode(text_mode);
    let normalized_max_text_chars = normalize_max_text_chars(max_text_chars);
    let export_filters = normalized_export_request_filters(filters);
    let samples = list_analysis_samples(pool, &export_filters).await?;
    let storage = gateway_object_storage()?;
    let mut rows = Vec::with_capacity(samples.len());

    for sample in samples {
        let request_artifact = match sample.request_artifact_object_key.as_deref() {
            Some(object_key) => storage.read_json(object_key).await.ok(),
            None => None,
        };
        let response_artifact = match sample.response_artifact_object_key.as_deref() {
            Some(object_key) => storage.read_json(object_key).await.ok(),
            None => None,
        };
        rows.push(build_export_row(
            &sample,
            request_artifact.as_ref(),
            response_artifact.as_ref(),
            normalized_text_mode,
            normalized_max_text_chars,
        ));
    }

    Ok(GatewayAnalysisExportView {
        text_mode: normalized_text_mode.to_string(),
        max_text_chars: normalized_max_text_chars,
        sample_count: rows.len(),
        request_artifact_count: rows
            .iter()
            .filter(|row| row.request_artifact_available)
            .count(),
        response_artifact_count: rows
            .iter()
            .filter(|row| row.response_artifact_available)
            .count(),
        rows,
    })
}

pub async fn persist_analysis_export(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    text_mode: Option<&str>,
    max_text_chars: Option<usize>,
    input: PersistGatewayAnalysisExportInput,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let normalized_text_mode = normalize_text_mode(text_mode);
    let normalized_max_text_chars = normalize_max_text_chars(max_text_chars);
    let normalized_filters = normalized_export_request_filters(filters);
    let export_view = export_analysis_rows(
        pool,
        &normalized_filters,
        Some(normalized_text_mode),
        Some(normalized_max_text_chars),
    )
    .await?;
    let export_id = Uuid::new_v4().to_string();
    let created_at = OffsetDateTime::now_utc();
    let label = normalize_export_label(input.label.as_deref())?;
    let tags = normalize_export_tags(input.tags.as_ref())?;
    let retention_expires_at =
        parse_optional_rfc3339(input.retention_expires_at.as_deref(), "retentionExpiresAt")?;
    let filter_view = build_export_filter_view(
        &normalized_filters,
        normalized_text_mode,
        normalized_max_text_chars,
    );
    let object_prefix = build_gateway_analysis_export_prefix(&export_id);
    let dataset_object_key = build_gateway_analysis_export_dataset_object_key(&export_id);
    let dataset_body = build_analysis_dataset_jsonl(&export_view.rows)?;
    let dataset_file = build_analysis_export_file_view(
        "dataset_jsonl",
        dataset_object_key.clone(),
        "application/x-ndjson",
        &dataset_body,
        Some(export_view.rows.len()),
    );
    let manifest_object_key = build_gateway_analysis_export_manifest_object_key(&export_id);
    let manifest_artifacts = build_manifest_artifacts(
        &export_id,
        label.clone(),
        tags.clone(),
        format_timestamp(created_at),
        retention_expires_at.map(format_timestamp),
        filter_view.clone(),
        export_view.sample_count,
        export_view.request_artifact_count,
        export_view.response_artifact_count,
        dataset_file,
        manifest_object_key.clone(),
    )?;
    let storage = gateway_object_storage()?;
    storage
        .put_bytes(&dataset_object_key, dataset_body, "application/x-ndjson")
        .await?;
    storage
        .put_bytes(
            &manifest_object_key,
            manifest_artifacts.manifest_body.clone(),
            "application/json",
        )
        .await?;

    sqlx::query(
        r#"
        insert into gateway_analysis_exports (
          id,
          project_id,
          label,
          tags,
          status,
          text_mode,
          max_text_chars,
          filters,
          object_prefix,
          manifest_object_key,
          dataset_object_key,
          sample_count,
          request_artifact_count,
          response_artifact_count,
          retention_expires_at,
          cleaned_up_at,
          last_cleanup_error,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, 'active', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, null, null, $15, $15
        )
        "#,
    )
    .bind(&export_id)
    .bind(filter_view.project_id.as_deref())
    .bind(label.as_deref())
    .bind(Json(tags))
    .bind(normalized_text_mode)
    .bind(i32::try_from(normalized_max_text_chars).unwrap_or(i32::MAX))
    .bind(Json(serde_json::to_value(&filter_view).map_err(|error| {
        GatewayError::server_error(format!("serialize analysis export filters: {error}"))
    })?))
    .bind(&object_prefix)
    .bind(&manifest_object_key)
    .bind(&dataset_object_key)
    .bind(i32::try_from(export_view.sample_count).unwrap_or(i32::MAX))
    .bind(i32::try_from(export_view.request_artifact_count).unwrap_or(i32::MAX))
    .bind(i32::try_from(export_view.response_artifact_count).unwrap_or(i32::MAX))
    .bind(retention_expires_at)
    .bind(created_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let row = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where id = $1
        limit 1
        "#,
    )
    .bind(&export_id)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    to_persisted_analysis_export_view(row, Some(manifest_artifacts.manifest))
}

async fn query_persisted_analysis_export_rows(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
    limit: Option<usize>,
) -> Result<Vec<GatewayAnalysisExportRow>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where 1 = 1
        "#,
    );
    if let Some(export_id) = trimmed_owned_ref_opt(filters.export_id.as_deref()) {
        builder.push(" and id = ").push_bind(export_id);
    }
    if let Some(project_id) = trimmed_owned_ref_opt(filters.project_id.as_deref()) {
        builder.push(" and project_id = ").push_bind(project_id);
    }
    if let Some(status) = trimmed_owned_ref_opt(filters.status.as_deref()) {
        builder.push(" and status = ").push_bind(status);
    }
    if let Some(text_mode) = trimmed_owned_ref_opt(filters.text_mode.as_deref()) {
        builder.push(" and text_mode = ").push_bind(text_mode);
    }
    if let Some(created_from) = created_from {
        builder.push(" and created_at >= ").push_bind(created_from);
    }
    if let Some(created_to) = created_to {
        builder.push(" and created_at <= ").push_bind(created_to);
    }
    builder.push(" order by created_at desc");
    if let Some(limit) = limit {
        builder
            .push(" limit ")
            .push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
    }
    builder
        .build_query_as::<GatewayAnalysisExportRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

fn normalized_export_request_filters(filters: &RequestAuditFilters) -> RequestAuditFilters {
    RequestAuditFilters {
        limit: Some(
            filters
                .limit
                .unwrap_or(DEFAULT_EXPORT_LIMIT)
                .clamp(1, MAX_EXPORT_LIMIT),
        ),
        ..filters.clone()
    }
}

fn normalize_text_mode(value: Option<&str>) -> &'static str {
    match value.and_then(trimmed_owned_ref) {
        Some("none") => "none",
        Some("full") => "full",
        Some("preview_redacted") => "preview_redacted",
        _ => DEFAULT_TEXT_MODE,
    }
}

fn normalize_max_text_chars(value: Option<usize>) -> usize {
    value
        .unwrap_or(DEFAULT_MAX_TEXT_CHARS)
        .clamp(0, MAX_TEXT_CHARS_LIMIT)
}

fn normalize_export_label(value: Option<&str>) -> Result<Option<String>, GatewayError> {
    let Some(value) = trimmed_owned_ref_opt(value) else {
        return Ok(None);
    };
    Ok(Some(
        value.chars().take(MAX_LABEL_LENGTH).collect::<String>(),
    ))
}

fn normalize_export_tags(values: Option<&Vec<String>>) -> Result<Vec<String>, GatewayError> {
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    if values.len() > MAX_TAGS {
        return Err(GatewayError::conflict("tags 数量不能超过 32 个。"));
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for value in values {
        let Some(normalized) = trimmed_owned(Some(value.as_str())).map(|item| item.to_lowercase())
        else {
            continue;
        };
        if normalized.chars().count() > MAX_TAG_LENGTH {
            return Err(GatewayError::conflict("单个 tag 长度不能超过 40 个字符。"));
        }
        if seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    Ok(result)
}

fn parse_optional_rfc3339(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = trimmed_owned_ref_opt(value) else {
        return Ok(None);
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::bad_request(format!("{field_name} 必须是合法的 RFC3339 时间戳")))
}

fn build_export_filter_view(
    filters: &RequestAuditFilters,
    text_mode: &str,
    max_text_chars: usize,
) -> GatewayAnalysisExportFilterView {
    GatewayAnalysisExportFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        status: filters.status.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        stream: filters.stream,
        error_code: filters.error_code.clone(),
        fallback_eligible: filters.fallback_eligible,
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        artifact_available: filters.artifact_available,
        limit: filters.limit.unwrap_or(DEFAULT_EXPORT_LIMIT).clamp(1, 1000),
        text_mode: text_mode.to_string(),
        max_text_chars,
    }
}

async fn try_read_analysis_export_manifest(
    object_key: &str,
) -> Option<GatewayAnalysisExportManifest> {
    let storage = gateway_object_storage().ok()?;
    let value = storage.read_json(object_key).await.ok()?;
    serde_json::from_value(value).ok()
}

async fn list_fallback_analysis_export_manifests(
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> Result<Vec<GatewayPersistedAnalysisExportView>, GatewayError> {
    let storage = gateway_object_storage()?;
    let manifest_keys = storage
        .list_objects("ai-gateway/analysis-exports")
        .await?
        .into_iter()
        .filter(|key| key.ends_with("/manifest.json"))
        .collect::<Vec<_>>();

    let mut results = Vec::new();
    for manifest_key in manifest_keys {
        let Some(manifest) = try_read_analysis_export_manifest(&manifest_key).await else {
            continue;
        };
        let view = to_manifest_backed_persisted_analysis_export_view(manifest);
        if let Some(export_id) = trimmed_owned_ref_opt(filters.export_id.as_deref()) {
            if view.export_id != export_id {
                continue;
            }
        }
        if !matches_persisted_analysis_export_view_filters(&view, filters, created_from, created_to)
        {
            continue;
        }
        results.push(view);
    }
    Ok(results)
}

fn to_manifest_backed_persisted_analysis_export_view(
    manifest: GatewayAnalysisExportManifest,
) -> GatewayPersistedAnalysisExportView {
    GatewayPersistedAnalysisExportView {
        export_id: manifest.export_id.clone(),
        label: manifest.label.clone(),
        tags: Vec::new(),
        status: "active".to_string(),
        created_at: manifest.created_at.clone(),
        updated_at: manifest.created_at.clone(),
        object_prefix: build_gateway_analysis_export_prefix(&manifest.export_id),
        filters: manifest.filters.clone(),
        sample_count: manifest.sample_count,
        request_artifact_count: manifest.request_artifact_count,
        response_artifact_count: manifest.response_artifact_count,
        retention_expires_at: None,
        cleaned_up_at: None,
        last_cleanup_error: None,
        files: manifest.files.clone(),
        manifest,
    }
}

fn build_synthetic_analysis_export_manifest(
    row: &GatewayAnalysisExportRow,
    filters: GatewayAnalysisExportFilterView,
) -> GatewayAnalysisExportManifest {
    GatewayAnalysisExportManifest {
        schema_version: 1,
        export_id: row.id.clone(),
        label: row.label.clone(),
        tags: row.tags.0.clone(),
        created_at: format_timestamp(row.created_at),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        filters,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        files: Vec::new(),
    }
}

fn build_export_row(
    sample: &GatewayAnalysisSampleView,
    request_artifact: Option<&Value>,
    response_artifact: Option<&Value>,
    text_mode: &str,
    max_text_chars: usize,
) -> GatewayAnalysisExportRowView {
    let request_messages = request_artifact
        .and_then(|artifact| {
            artifact
                .get("canonicalRequest")
                .and_then(|value| value.get("messages"))
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|message| coerce_message_export(&message))
        .collect::<Vec<_>>();
    let request_text_source = request_messages
        .iter()
        .map(|message| message.text.as_str())
        .filter(|item| !item.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let response_text_source = response_artifact
        .and_then(|artifact| artifact.get("result"))
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let request_text = apply_text_mode(&request_text_source, text_mode, max_text_chars);
    let response_text = apply_text_mode(&response_text_source, text_mode, max_text_chars);

    GatewayAnalysisExportRowView {
        request_audit_id: sample.request_audit_id.clone(),
        response_id: sample.response_id.clone(),
        project_id: sample.project_id.clone(),
        session_id: sample.session_id.clone(),
        provider_account_id: sample.provider_account_id.clone(),
        protocol_family: sample.protocol_family.clone(),
        endpoint_kind: sample.endpoint_kind.clone(),
        requested_model: sample.requested_model.clone(),
        resolved_model: sample.resolved_model.clone(),
        status: sample.status.clone(),
        stream: sample.stream,
        created_at: sample.created_at.clone(),
        completed_at: sample.completed_at.clone(),
        prompt_tokens: sample.prompt_tokens,
        completion_tokens: sample.completion_tokens,
        total_tokens: sample.total_tokens,
        request_artifact_available: sample.request_artifact_object_key.is_some(),
        response_artifact_available: sample.response_artifact_object_key.is_some(),
        analysis_profile: sample.analysis_profile.clone(),
        route_trace: sample.route_trace.clone(),
        request_text: request_text.text,
        response_text: response_text.text,
        request_text_truncated: request_text.truncated,
        response_text_truncated: response_text.truncated,
        request_messages,
        request_tool_names: coerce_request_tool_names(request_artifact),
        response_tool_names: coerce_response_tool_names(response_artifact),
    }
}

fn build_analysis_dataset_jsonl(
    rows: &[GatewayAnalysisExportRowView],
) -> Result<Vec<u8>, GatewayError> {
    let mut body = String::new();
    for row in rows {
        let line = serde_json::to_string(row).map_err(|error| {
            GatewayError::server_error(format!("serialize analysis export row: {error}"))
        })?;
        body.push_str(&line);
        body.push('\n');
    }
    Ok(body.into_bytes())
}

fn build_analysis_export_manifest(
    export_id: &str,
    label: Option<String>,
    tags: Vec<String>,
    created_at: String,
    retention_expires_at: Option<String>,
    filters: GatewayAnalysisExportFilterView,
    sample_count: usize,
    request_artifact_count: usize,
    response_artifact_count: usize,
    files: Vec<GatewayAnalysisExportFileView>,
) -> GatewayAnalysisExportManifest {
    GatewayAnalysisExportManifest {
        schema_version: 1,
        export_id: export_id.to_string(),
        label,
        tags,
        created_at,
        retention_expires_at,
        filters,
        sample_count,
        request_artifact_count,
        response_artifact_count,
        files,
    }
}

struct ManifestArtifacts {
    manifest: GatewayAnalysisExportManifest,
    manifest_body: Vec<u8>,
}

fn build_manifest_artifacts(
    export_id: &str,
    label: Option<String>,
    tags: Vec<String>,
    created_at: String,
    retention_expires_at: Option<String>,
    filters: GatewayAnalysisExportFilterView,
    sample_count: usize,
    request_artifact_count: usize,
    response_artifact_count: usize,
    dataset_file: GatewayAnalysisExportFileView,
    manifest_object_key: String,
) -> Result<ManifestArtifacts, GatewayError> {
    let placeholder_manifest = build_analysis_export_manifest(
        export_id,
        label,
        tags,
        created_at,
        retention_expires_at,
        filters,
        sample_count,
        request_artifact_count,
        response_artifact_count,
        vec![
            dataset_file.clone(),
            GatewayAnalysisExportFileView {
                kind: "manifest".to_string(),
                object_key: manifest_object_key.clone(),
                content_type: "application/json".to_string(),
                size_bytes: 0,
                sha256: String::new(),
                line_count: None,
            },
        ],
    );
    let first_body = serde_json::to_vec_pretty(&placeholder_manifest).map_err(|error| {
        GatewayError::server_error(format!("serialize analysis export manifest: {error}"))
    })?;
    let manifest_file = build_analysis_export_file_view(
        "manifest",
        manifest_object_key,
        "application/json",
        &first_body,
        None,
    );
    let manifest = build_analysis_export_manifest(
        &placeholder_manifest.export_id,
        placeholder_manifest.label,
        placeholder_manifest.tags,
        placeholder_manifest.created_at,
        placeholder_manifest.retention_expires_at,
        placeholder_manifest.filters,
        placeholder_manifest.sample_count,
        placeholder_manifest.request_artifact_count,
        placeholder_manifest.response_artifact_count,
        vec![manifest_file, dataset_file],
    );
    let manifest_body = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize finalized analysis export manifest: {error}"
        ))
    })?;
    Ok(ManifestArtifacts {
        manifest,
        manifest_body,
    })
}

fn build_analysis_export_file_view(
    kind: &str,
    object_key: String,
    content_type: &str,
    body: &[u8],
    line_count: Option<usize>,
) -> GatewayAnalysisExportFileView {
    GatewayAnalysisExportFileView {
        kind: kind.to_string(),
        object_key,
        content_type: content_type.to_string(),
        size_bytes: body.len(),
        sha256: sha256_bytes(body),
        line_count,
    }
}

fn to_persisted_analysis_export_view(
    row: GatewayAnalysisExportRow,
    manifest: Option<GatewayAnalysisExportManifest>,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let filters = serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
        .map_err(|error| {
            GatewayError::server_error(format!("parse analysis export filters: {error}"))
        })?;
    let manifest = match manifest {
        Some(mut manifest) => {
            manifest.label = row.label.clone();
            manifest.tags = row.tags.0.clone();
            manifest.filters = filters.clone();
            manifest.sample_count = usize::try_from(row.sample_count).unwrap_or_default();
            manifest.request_artifact_count =
                usize::try_from(row.request_artifact_count).unwrap_or_default();
            manifest.response_artifact_count =
                usize::try_from(row.response_artifact_count).unwrap_or_default();
            manifest.retention_expires_at = row.retention_expires_at.map(format_timestamp);
            manifest
        }
        None => build_synthetic_analysis_export_manifest(&row, filters.clone()),
    };
    Ok(GatewayPersistedAnalysisExportView {
        export_id: row.id,
        label: row.label,
        tags: row.tags.0,
        status: row.status,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        object_prefix: row.object_prefix,
        filters,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        cleaned_up_at: row.cleaned_up_at.map(format_timestamp),
        last_cleanup_error: row.last_cleanup_error,
        files: manifest.files.clone(),
        manifest,
    })
}

fn matches_persisted_analysis_export_filters(
    row: &GatewayAnalysisExportRow,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    let filter_view =
        match serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone()) {
            Ok(filters) => filters,
            Err(_) => return false,
        };
    let item = GatewayPersistedAnalysisExportView {
        export_id: row.id.clone(),
        label: row.label.clone(),
        tags: row.tags.0.clone(),
        status: row.status.clone(),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        object_prefix: row.object_prefix.clone(),
        filters: filter_view,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        cleaned_up_at: row.cleaned_up_at.map(format_timestamp),
        last_cleanup_error: row.last_cleanup_error.clone(),
        files: Vec::new(),
        manifest: build_synthetic_analysis_export_manifest(
            row,
            serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
                .unwrap_or(GatewayAnalysisExportFilterView {
                    project_id: None,
                    route_policy_id: None,
                    provider_account_id: None,
                    session_id: None,
                    api_key_id: None,
                    response_id: None,
                    protocol_family: None,
                    status: None,
                    endpoint_kind: None,
                    stream: None,
                    error_code: None,
                    fallback_eligible: None,
                    created_from: None,
                    created_to: None,
                    artifact_available: None,
                    limit: DEFAULT_EXPORT_LIMIT,
                    text_mode: DEFAULT_TEXT_MODE.to_string(),
                    max_text_chars: DEFAULT_MAX_TEXT_CHARS,
                }),
        ),
    };
    matches_persisted_analysis_export_view_filters(&item, filters, created_from, created_to)
}

fn matches_persisted_analysis_export_view_filters(
    item: &GatewayPersistedAnalysisExportView,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    if let Some(label) = trimmed_owned_ref_opt(filters.label.as_deref()) {
        let haystack = item
            .label
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if !haystack.contains(&label.to_lowercase()) {
            return false;
        }
    }
    if let Some(tag) = trimmed_owned_ref_opt(filters.tag.as_deref()) {
        let tag = tag.to_lowercase();
        if !normalize_tag_values(&item.tags)
            .iter()
            .any(|value| value == &tag)
        {
            return false;
        }
    }
    if let Some(project_id) = trimmed_owned_ref_opt(filters.project_id.as_deref()) {
        if item.filters.project_id.as_deref() != Some(project_id) {
            return false;
        }
    }
    if let Some(status) = trimmed_owned_ref_opt(filters.status.as_deref()) {
        if item.status.trim() != status {
            return false;
        }
    }
    if let Some(text_mode) = trimmed_owned_ref_opt(filters.text_mode.as_deref()) {
        if item.filters.text_mode.trim() != text_mode {
            return false;
        }
    }
    let created_at = match OffsetDateTime::parse(&item.created_at, &Rfc3339) {
        Ok(timestamp) => timestamp,
        Err(_) => return false,
    };
    if let Some(created_from) = created_from {
        if created_at < created_from {
            return false;
        }
    }
    if let Some(created_to) = created_to {
        if created_at > created_to {
            return false;
        }
    }
    true
}

fn build_analysis_export_inventory_summary(
    exports: &[GatewayPersistedAnalysisExportView],
    now: OffsetDateTime,
) -> GatewayAnalysisExportInventorySummaryView {
    let expiring_threshold = now + time::Duration::hours(24);
    let mut by_status = HashMap::new();
    let mut by_text_mode = HashMap::new();
    let mut by_tag = HashMap::new();
    let mut by_project = HashMap::new();
    let mut active_exports = 0usize;
    let mut deleted_exports = 0usize;
    let mut pinned_exports = 0usize;
    let mut expiring_within_24_hours = 0usize;
    let mut expired_active_exports = 0usize;
    let mut total_sample_count = 0usize;
    let mut total_request_artifact_count = 0usize;
    let mut total_response_artifact_count = 0usize;

    for item in exports {
        total_sample_count += item.sample_count;
        total_request_artifact_count += item.request_artifact_count;
        total_response_artifact_count += item.response_artifact_count;
        accumulate_bucket(&mut by_status, &item.status);
        accumulate_bucket(&mut by_text_mode, &item.filters.text_mode);
        if let Some(project_id) = item.filters.project_id.as_deref() {
            accumulate_bucket(&mut by_project, project_id);
        }
        for tag in &item.tags {
            accumulate_bucket(&mut by_tag, tag);
        }

        match item.status.as_str() {
            "active" => active_exports += 1,
            "deleted" => deleted_exports += 1,
            _ => {}
        }
        if has_pinned_tag(&item.tags) {
            pinned_exports += 1;
        }
        if item.status == "active" {
            if let Some(retention_expires_at) = item.retention_expires_at.as_deref() {
                if let Ok(expires_at) = OffsetDateTime::parse(retention_expires_at, &Rfc3339) {
                    if expires_at <= now {
                        expired_active_exports += 1;
                    } else if expires_at <= expiring_threshold {
                        expiring_within_24_hours += 1;
                    }
                }
            }
        }
    }

    GatewayAnalysisExportInventorySummaryView {
        total_exports: exports.len(),
        active_exports,
        deleted_exports,
        pinned_exports,
        expiring_within_24_hours,
        expired_active_exports,
        total_sample_count,
        total_request_artifact_count,
        total_response_artifact_count,
        by_status: into_key_buckets(by_status),
        by_text_mode: into_key_buckets(by_text_mode),
        by_tag: into_key_buckets(by_tag),
        by_project: into_key_buckets(by_project),
    }
}

async fn read_analysis_export_dataset_for_export(
    export: &GatewayPersistedAnalysisExportView,
) -> Result<Vec<GatewayAnalysisExportRowView>, GatewayError> {
    let dataset_object_key = export
        .files
        .iter()
        .find(|file| file.kind == "dataset_jsonl")
        .map(|file| file.object_key.clone())
        .unwrap_or_else(|| build_gateway_analysis_export_dataset_object_key(&export.export_id));
    read_analysis_export_dataset(&dataset_object_key).await
}

async fn read_analysis_export_dataset(
    object_key: &str,
) -> Result<Vec<GatewayAnalysisExportRowView>, GatewayError> {
    let bytes = gateway_object_storage()?.read_bytes(object_key).await?;
    let body = String::from_utf8(bytes).map_err(|error| {
        GatewayError::server_error(format!("parse analysis export dataset utf8: {error}"))
    })?;
    let mut rows = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let row =
            serde_json::from_str::<GatewayAnalysisExportRowView>(trimmed).map_err(|error| {
                GatewayError::server_error(format!("parse analysis export dataset row: {error}"))
            })?;
        rows.push(row);
    }
    Ok(rows)
}

fn build_metric_delta(
    left_value: Option<f64>,
    right_value: Option<f64>,
) -> GatewayAnalysisExportMetricDeltaView {
    GatewayAnalysisExportMetricDeltaView {
        left_value,
        right_value,
        delta_value: match (left_value, right_value) {
            (Some(left), Some(right)) => Some(right - left),
            _ => None,
        },
    }
}

fn build_bucket_delta<F>(
    left_rows: &[GatewayAnalysisExportRowView],
    right_rows: &[GatewayAnalysisExportRowView],
    selector: F,
) -> Vec<GatewayAnalysisExportBucketDeltaView>
where
    F: Fn(&GatewayAnalysisExportRowView) -> Option<&str>,
{
    let mut left_counts = HashMap::new();
    let mut right_counts = HashMap::new();

    for row in left_rows {
        if let Some(key) = selector(row).and_then(trimmed_owned_ref) {
            *left_counts.entry(key.to_string()).or_insert(0usize) += 1;
        }
    }
    for row in right_rows {
        if let Some(key) = selector(row).and_then(trimmed_owned_ref) {
            *right_counts.entry(key.to_string()).or_insert(0usize) += 1;
        }
    }

    let mut items = left_counts
        .keys()
        .chain(right_counts.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|key| {
            let left_count = left_counts.get(&key).copied().unwrap_or_default();
            let right_count = right_counts.get(&key).copied().unwrap_or_default();
            GatewayAnalysisExportBucketDeltaView {
                key,
                left_count,
                right_count,
                delta_count: right_count as i64 - left_count as i64,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .delta_count
            .abs()
            .cmp(&left.delta_count.abs())
            .then_with(|| left.key.cmp(&right.key))
    });
    items
}

fn sum_metric<F>(rows: &[GatewayAnalysisExportRowView], selector: F) -> Option<f64>
where
    F: Fn(&GatewayAnalysisExportRowView) -> Option<i32>,
{
    Some(
        rows.iter()
            .map(|row| i64::from(selector(row).unwrap_or_default()))
            .sum::<i64>() as f64,
    )
}

fn normalize_analysis_export_report_filters(
    filters: &GatewayPersistedAnalysisExportFilters,
    min_limit: usize,
    max_limit: usize,
    default_limit: usize,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        export_id: None,
        label: filters.label.clone(),
        tag: filters.tag.clone(),
        project_id: filters.project_id.clone(),
        status: Some(
            trimmed_owned_ref_opt(filters.status.as_deref())
                .unwrap_or("active")
                .to_string(),
        ),
        text_mode: filters.text_mode.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit: Some(
            filters
                .limit
                .unwrap_or(default_limit)
                .clamp(min_limit, max_limit),
        ),
    }
}

fn to_baseline_report_filter_view(
    filters: &GatewayPersistedAnalysisExportFilters,
) -> GatewayAnalysisExportBaselineReportFilterView {
    GatewayAnalysisExportBaselineReportFilterView {
        label: filters.label.clone(),
        tag: filters.tag.clone(),
        project_id: filters.project_id.clone(),
        status: filters.status.clone(),
        text_mode: filters.text_mode.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
    }
}

fn build_analysis_export_diff(
    left_export: GatewayPersistedAnalysisExportView,
    right_export: GatewayPersistedAnalysisExportView,
    left_rows: &[GatewayAnalysisExportRowView],
    right_rows: &[GatewayAnalysisExportRowView],
) -> GatewayAnalysisExportDiffView {
    let left_request_ids = left_rows
        .iter()
        .map(|row| row.request_audit_id.clone())
        .collect::<HashSet<_>>();
    let right_request_ids = right_rows
        .iter()
        .map(|row| row.request_audit_id.clone())
        .collect::<HashSet<_>>();
    let overlap_request_count = left_request_ids
        .iter()
        .filter(|request_id| right_request_ids.contains(*request_id))
        .count();
    GatewayAnalysisExportDiffView {
        left_export,
        right_export,
        overlap_request_count,
        left_only_request_count: left_request_ids.len().saturating_sub(overlap_request_count),
        right_only_request_count: right_request_ids
            .len()
            .saturating_sub(overlap_request_count),
        sample_count: build_metric_delta(
            Some(left_rows.len() as f64),
            Some(right_rows.len() as f64),
        ),
        request_artifact_count: build_metric_delta(
            Some(
                left_rows
                    .iter()
                    .filter(|row| row.request_artifact_available)
                    .count() as f64,
            ),
            Some(
                right_rows
                    .iter()
                    .filter(|row| row.request_artifact_available)
                    .count() as f64,
            ),
        ),
        response_artifact_count: build_metric_delta(
            Some(
                left_rows
                    .iter()
                    .filter(|row| row.response_artifact_available)
                    .count() as f64,
            ),
            Some(
                right_rows
                    .iter()
                    .filter(|row| row.response_artifact_available)
                    .count() as f64,
            ),
        ),
        prompt_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.prompt_tokens),
            sum_metric(right_rows, |row| row.prompt_tokens),
        ),
        completion_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.completion_tokens),
            sum_metric(right_rows, |row| row.completion_tokens),
        ),
        total_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.total_tokens),
            sum_metric(right_rows, |row| row.total_tokens),
        ),
        by_status: build_bucket_delta(left_rows, right_rows, |row| Some(row.status.as_str())),
        by_protocol_family: build_bucket_delta(left_rows, right_rows, |row| {
            Some(row.protocol_family.as_str())
        }),
        by_endpoint_kind: build_bucket_delta(left_rows, right_rows, |row| {
            Some(row.endpoint_kind.as_str())
        }),
        by_resolved_model: build_bucket_delta(left_rows, right_rows, |row| {
            row.resolved_model.as_deref()
        }),
        by_provider_account: build_bucket_delta(left_rows, right_rows, |row| {
            row.provider_account_id.as_deref()
        }),
    }
}

async fn build_analysis_export_trend_point(
    export: &GatewayPersistedAnalysisExportView,
) -> GatewayAnalysisExportTrendPointView {
    let rows = match read_analysis_export_dataset_for_export(export).await {
        Ok(rows) => rows,
        Err(_) => {
            return GatewayAnalysisExportTrendPointView {
                export: export.clone(),
                dataset_available: false,
                dataset_unavailable_reason: Some("dataset_missing".to_string()),
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                stream_samples: None,
                completed_samples: None,
                failed_samples: None,
                cancelled_samples: None,
                tool_request_samples: None,
                tool_response_samples: None,
                system_prompt_samples: None,
                reasoning_samples: None,
                metadata_samples: None,
                explicit_session_samples: None,
                previous_response_samples: None,
            };
        }
    };

    let mut prompt_tokens = 0i64;
    let mut completion_tokens = 0i64;
    let mut total_tokens = 0i64;
    let mut stream_samples = 0i64;
    let mut completed_samples = 0i64;
    let mut failed_samples = 0i64;
    let mut cancelled_samples = 0i64;
    let mut tool_request_samples = 0i64;
    let mut tool_response_samples = 0i64;
    let mut system_prompt_samples = 0i64;
    let mut reasoning_samples = 0i64;
    let mut metadata_samples = 0i64;
    let mut explicit_session_samples = 0i64;
    let mut previous_response_samples = 0i64;

    for row in &rows {
        prompt_tokens += i64::from(row.prompt_tokens.unwrap_or_default());
        completion_tokens += i64::from(row.completion_tokens.unwrap_or_default());
        total_tokens += i64::from(row.total_tokens.unwrap_or_default());
        if row.stream {
            stream_samples += 1;
        }
        match row.status.as_str() {
            "completed" => completed_samples += 1,
            "failed" => failed_samples += 1,
            "cancelled" => cancelled_samples += 1,
            _ => {}
        }
        let request_tool_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("requestToolCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let request_historical_tool_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("requestHistoricalToolCallCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if request_tool_count > 0 || request_historical_tool_count > 0 {
            tool_request_samples += 1;
        }
        let response_tool_call_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("responseToolCallCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if response_tool_call_count > 0 {
            tool_response_samples += 1;
        }
        if analysis_profile_flag(row, "hasSystemPrompt") {
            system_prompt_samples += 1;
        }
        if analysis_profile_flag(row, "hasReasoning") {
            reasoning_samples += 1;
        }
        if analysis_profile_flag(row, "hasMetadata") {
            metadata_samples += 1;
        }
        if analysis_profile_flag(row, "hasExplicitSessionKey") {
            explicit_session_samples += 1;
        }
        if analysis_profile_flag(row, "hasPreviousResponse") {
            previous_response_samples += 1;
        }
    }

    GatewayAnalysisExportTrendPointView {
        export: export.clone(),
        dataset_available: true,
        dataset_unavailable_reason: None,
        prompt_tokens: Some(prompt_tokens),
        completion_tokens: Some(completion_tokens),
        total_tokens: Some(total_tokens),
        stream_samples: Some(stream_samples),
        completed_samples: Some(completed_samples),
        failed_samples: Some(failed_samples),
        cancelled_samples: Some(cancelled_samples),
        tool_request_samples: Some(tool_request_samples),
        tool_response_samples: Some(tool_response_samples),
        system_prompt_samples: Some(system_prompt_samples),
        reasoning_samples: Some(reasoning_samples),
        metadata_samples: Some(metadata_samples),
        explicit_session_samples: Some(explicit_session_samples),
        previous_response_samples: Some(previous_response_samples),
    }
}

fn analysis_profile_flag(row: &GatewayAnalysisExportRowView, field: &str) -> bool {
    row.analysis_profile
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn build_analysis_export_trend_summary(
    points: &[GatewayAnalysisExportTrendPointView],
) -> Option<GatewayAnalysisExportTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayAnalysisExportTrendSummaryView {
        latest_export_id: Some(latest.export.export_id.clone()),
        previous_export_id: previous.map(|item| item.export.export_id.clone()),
        prompt_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.prompt_tokens, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.prompt_tokens, item.export.sample_count)),
        ),
        completion_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.completion_tokens, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.completion_tokens, item.export.sample_count)),
        ),
        total_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.total_tokens, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.total_tokens, item.export.sample_count)),
        ),
        request_artifact_coverage: build_trend_metric_summary(
            safe_ratio_usize(
                Some(latest.export.request_artifact_count),
                latest.export.sample_count,
            ),
            previous.and_then(|item| {
                safe_ratio_usize(
                    Some(item.export.request_artifact_count),
                    item.export.sample_count,
                )
            }),
        ),
        response_artifact_coverage: build_trend_metric_summary(
            safe_ratio_usize(
                Some(latest.export.response_artifact_count),
                latest.export.sample_count,
            ),
            previous.and_then(|item| {
                safe_ratio_usize(
                    Some(item.export.response_artifact_count),
                    item.export.sample_count,
                )
            }),
        ),
        stream_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.stream_samples, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.stream_samples, item.export.sample_count)),
        ),
        completion_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.completed_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.completed_samples, item.export.sample_count)),
        ),
        failure_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.failed_samples, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.failed_samples, item.export.sample_count)),
        ),
        cancellation_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.cancelled_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.cancelled_samples, item.export.sample_count)),
        ),
        tool_request_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.tool_request_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.tool_request_samples, item.export.sample_count)
            }),
        ),
        tool_response_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.tool_response_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.tool_response_samples, item.export.sample_count)
            }),
        ),
        reasoning_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.reasoning_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.reasoning_samples, item.export.sample_count)),
        ),
        metadata_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.metadata_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.metadata_samples, item.export.sample_count)),
        ),
        explicit_session_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.explicit_session_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.explicit_session_samples, item.export.sample_count)
            }),
        ),
        previous_response_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.previous_response_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.previous_response_samples, item.export.sample_count)
            }),
        ),
    })
}

fn build_trend_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayAnalysisExportTrendMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta_value)) if previous != 0.0 => {
            Some(delta_value / previous)
        }
        _ => None,
    };
    GatewayAnalysisExportTrendMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}

fn safe_ratio_i64(numerator: Option<i64>, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    numerator.map(|value| value as f64 / denominator as f64)
}

fn safe_ratio_usize(numerator: Option<usize>, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    numerator.map(|value| value as f64 / denominator as f64)
}

fn normalize_analysis_export_profile_key(value: Option<&str>) -> &'static str {
    match value
        .and_then(trimmed_owned_ref)
        .map(|value| value.to_lowercase())
    {
        Some(value) if value == "conservative" => "conservative",
        Some(value) if value == "aggressive" => "aggressive",
        _ => "balanced",
    }
}

fn build_analysis_export_anomaly_threshold_config(
    profile_key: &str,
    policy_thresholds: Option<GatewayAnalysisExportAnomalyThresholdConfig>,
    overrides: &GatewayAnalysisExportAnomalyOverrides,
) -> Result<GatewayAnalysisExportAnomalyThresholdConfig, GatewayError> {
    let mut thresholds = policy_thresholds.unwrap_or_else(|| match profile_key {
        "conservative" => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.2,
            failure_rate_critical_threshold: 0.3,
            failure_rate_delta_ratio_threshold: 0.8,
            completion_rate_warning_threshold: 0.7,
            completion_rate_critical_threshold: 0.55,
            completion_rate_delta_value_threshold: -0.15,
            response_artifact_coverage_warning_threshold: 0.75,
            response_artifact_coverage_critical_threshold: 0.55,
            response_artifact_coverage_delta_value_threshold: -0.15,
            request_artifact_coverage_warning_threshold: 0.8,
            request_artifact_coverage_critical_threshold: 0.6,
            request_artifact_coverage_delta_value_threshold: -0.15,
            tokens_per_sample_warning_delta_ratio_threshold: 0.5,
            tokens_per_sample_critical_delta_ratio_threshold: 1.0,
            tokens_per_sample_critical_absolute_threshold: 2500.0,
        },
        "aggressive" => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.12,
            failure_rate_critical_threshold: 0.2,
            failure_rate_delta_ratio_threshold: 0.35,
            completion_rate_warning_threshold: 0.8,
            completion_rate_critical_threshold: 0.7,
            completion_rate_delta_value_threshold: -0.08,
            response_artifact_coverage_warning_threshold: 0.85,
            response_artifact_coverage_critical_threshold: 0.7,
            response_artifact_coverage_delta_value_threshold: -0.08,
            request_artifact_coverage_warning_threshold: 0.9,
            request_artifact_coverage_critical_threshold: 0.75,
            request_artifact_coverage_delta_value_threshold: -0.08,
            tokens_per_sample_warning_delta_ratio_threshold: 0.25,
            tokens_per_sample_critical_delta_ratio_threshold: 0.6,
            tokens_per_sample_critical_absolute_threshold: 1800.0,
        },
        _ => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.15,
            failure_rate_critical_threshold: 0.25,
            failure_rate_delta_ratio_threshold: 0.5,
            completion_rate_warning_threshold: 0.75,
            completion_rate_critical_threshold: 0.6,
            completion_rate_delta_value_threshold: -0.1,
            response_artifact_coverage_warning_threshold: 0.8,
            response_artifact_coverage_critical_threshold: 0.6,
            response_artifact_coverage_delta_value_threshold: -0.1,
            request_artifact_coverage_warning_threshold: 0.85,
            request_artifact_coverage_critical_threshold: 0.65,
            request_artifact_coverage_delta_value_threshold: -0.1,
            tokens_per_sample_warning_delta_ratio_threshold: 0.35,
            tokens_per_sample_critical_delta_ratio_threshold: 0.8,
            tokens_per_sample_critical_absolute_threshold: 2000.0,
        },
    });
    apply_analysis_export_anomaly_overrides(&mut thresholds, overrides)?;
    Ok(thresholds)
}

fn apply_analysis_export_anomaly_overrides(
    thresholds: &mut GatewayAnalysisExportAnomalyThresholdConfig,
    overrides: &GatewayAnalysisExportAnomalyOverrides,
) -> Result<(), GatewayError> {
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_warning_threshold,
        "failureRateWarningThreshold",
    )? {
        thresholds.failure_rate_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_critical_threshold,
        "failureRateCriticalThreshold",
    )? {
        thresholds.failure_rate_critical_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_delta_ratio_threshold,
        "failureRateDeltaRatioThreshold",
    )? {
        thresholds.failure_rate_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.completion_rate_warning_threshold,
        "completionRateWarningThreshold",
    )? {
        thresholds.completion_rate_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.completion_rate_critical_threshold,
        "completionRateCriticalThreshold",
    )? {
        thresholds.completion_rate_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.completion_rate_delta_value_threshold,
        "completionRateDeltaValueThreshold",
    )? {
        thresholds.completion_rate_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.response_artifact_coverage_warning_threshold,
        "responseArtifactCoverageWarningThreshold",
    )? {
        thresholds.response_artifact_coverage_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.response_artifact_coverage_critical_threshold,
        "responseArtifactCoverageCriticalThreshold",
    )? {
        thresholds.response_artifact_coverage_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.response_artifact_coverage_delta_value_threshold,
        "responseArtifactCoverageDeltaValueThreshold",
    )? {
        thresholds.response_artifact_coverage_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.request_artifact_coverage_warning_threshold,
        "requestArtifactCoverageWarningThreshold",
    )? {
        thresholds.request_artifact_coverage_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.request_artifact_coverage_critical_threshold,
        "requestArtifactCoverageCriticalThreshold",
    )? {
        thresholds.request_artifact_coverage_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.request_artifact_coverage_delta_value_threshold,
        "requestArtifactCoverageDeltaValueThreshold",
    )? {
        thresholds.request_artifact_coverage_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_warning_delta_ratio_threshold,
        "tokensPerSampleWarningDeltaRatioThreshold",
    )? {
        thresholds.tokens_per_sample_warning_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_critical_delta_ratio_threshold,
        "tokensPerSampleCriticalDeltaRatioThreshold",
    )? {
        thresholds.tokens_per_sample_critical_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_critical_absolute_threshold,
        "tokensPerSampleCriticalAbsoluteThreshold",
    )? {
        thresholds.tokens_per_sample_critical_absolute_threshold = value;
    }
    Ok(())
}

fn normalize_non_negative_override(
    value: Option<f64>,
    field_name: &str,
) -> Result<Option<f64>, GatewayError> {
    match value {
        Some(value) if !value.is_finite() => Err(GatewayError::conflict(format!(
            "{field_name} 必须是合法数字。"
        ))),
        Some(value) if value < 0.0 => {
            Err(GatewayError::conflict(format!("{field_name} 不能小于 0。")))
        }
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn normalize_signed_override(
    value: Option<f64>,
    field_name: &str,
) -> Result<Option<f64>, GatewayError> {
    match value {
        Some(value) if !value.is_finite() => Err(GatewayError::conflict(format!(
            "{field_name} 必须是合法数字。"
        ))),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn build_analysis_export_anomaly_report(
    trend_report: GatewayAnalysisExportTrendReportView,
    profile_key: String,
    thresholds: GatewayAnalysisExportAnomalyThresholdConfig,
) -> GatewayAnalysisExportAnomalyReportView {
    let mut anomalies = Vec::new();
    let latest = trend_report.points.first();
    let latest_export_id = trend_report
        .summary
        .as_ref()
        .and_then(|summary| summary.latest_export_id.clone())
        .or_else(|| latest.map(|point| point.export.export_id.clone()));
    let previous_export_id = trend_report
        .summary
        .as_ref()
        .and_then(|summary| summary.previous_export_id.clone())
        .or_else(|| {
            trend_report
                .points
                .get(1)
                .map(|point| point.export.export_id.clone())
        });

    if latest.is_some_and(|point| !point.dataset_available) {
        anomalies.push(GatewayAnalysisExportAnomalyView {
            code: "latest_dataset_missing".to_string(),
            severity: "critical".to_string(),
            message: "最新一批 export 的 dataset.jsonl 不可用，趋势与基线分析已经失真。"
                .to_string(),
            latest_export_id: latest_export_id.clone(),
            previous_export_id: previous_export_id.clone(),
            latest_value: None,
            previous_value: None,
            delta_value: None,
            delta_ratio: None,
            threshold_value: None,
        });
    }

    if let Some(summary) = trend_report.summary.as_ref() {
        if summary.failure_rate.latest_value.unwrap_or(0.0)
            >= thresholds.failure_rate_warning_threshold
            || summary.failure_rate.delta_ratio.unwrap_or(0.0)
                >= thresholds.failure_rate_delta_ratio_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "failure_rate_spike",
                "失败率相对基线显著升高。",
                if summary.failure_rate.latest_value.unwrap_or(0.0)
                    >= thresholds.failure_rate_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.failure_rate,
                Some(thresholds.failure_rate_warning_threshold),
            );
        }
        if summary.completion_rate.delta_value.unwrap_or(0.0)
            <= thresholds.completion_rate_delta_value_threshold
            || summary.completion_rate.latest_value.unwrap_or(1.0)
                <= thresholds.completion_rate_warning_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "completion_rate_drop",
                "完成率相对基线明显下降。",
                if summary.completion_rate.latest_value.unwrap_or(1.0)
                    <= thresholds.completion_rate_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.completion_rate,
                Some(thresholds.completion_rate_warning_threshold),
            );
        }
        if summary
            .response_artifact_coverage
            .latest_value
            .unwrap_or(1.0)
            <= thresholds.response_artifact_coverage_warning_threshold
            || summary
                .response_artifact_coverage
                .delta_value
                .unwrap_or(0.0)
                <= thresholds.response_artifact_coverage_delta_value_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "response_artifact_coverage_drop",
                "response artifact 覆盖率低于安全阈值或相对基线明显下降。",
                if summary
                    .response_artifact_coverage
                    .latest_value
                    .unwrap_or(1.0)
                    <= thresholds.response_artifact_coverage_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.response_artifact_coverage,
                Some(thresholds.response_artifact_coverage_warning_threshold),
            );
        }
        if summary
            .request_artifact_coverage
            .latest_value
            .unwrap_or(1.0)
            <= thresholds.request_artifact_coverage_warning_threshold
            || summary.request_artifact_coverage.delta_value.unwrap_or(0.0)
                <= thresholds.request_artifact_coverage_delta_value_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "request_artifact_coverage_drop",
                "request artifact 覆盖率低于安全阈值或相对基线明显下降。",
                if summary
                    .request_artifact_coverage
                    .latest_value
                    .unwrap_or(1.0)
                    <= thresholds.request_artifact_coverage_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.request_artifact_coverage,
                Some(thresholds.request_artifact_coverage_warning_threshold),
            );
        }
        if summary.total_tokens_per_sample.delta_ratio.unwrap_or(0.0)
            >= thresholds.tokens_per_sample_warning_delta_ratio_threshold
            || summary.total_tokens_per_sample.latest_value.unwrap_or(0.0)
                >= thresholds.tokens_per_sample_critical_absolute_threshold
        {
            let severity = if summary.total_tokens_per_sample.delta_ratio.unwrap_or(0.0)
                >= thresholds.tokens_per_sample_critical_delta_ratio_threshold
                || summary.total_tokens_per_sample.latest_value.unwrap_or(0.0)
                    >= thresholds.tokens_per_sample_critical_absolute_threshold
            {
                "critical"
            } else {
                "warning"
            };
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "tokens_per_sample_spike",
                "单样本 token 成本相对基线明显上升。",
                severity,
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.total_tokens_per_sample,
                Some(thresholds.tokens_per_sample_warning_delta_ratio_threshold),
            );
        }
    }

    let mut by_severity = HashMap::new();
    let mut by_code = HashMap::new();
    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, &anomaly.severity);
        accumulate_bucket(&mut by_code, &anomaly.code);
    }
    GatewayAnalysisExportAnomalyReportView {
        generated_at: trend_report.generated_at.clone(),
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        latest_export: latest.map(|point| point.export.clone()),
        previous_export: trend_report.points.get(1).map(|point| point.export.clone()),
        trend_summary: trend_report.summary,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

fn push_analysis_export_metric_anomaly(
    anomalies: &mut Vec<GatewayAnalysisExportAnomalyView>,
    code: &str,
    message: &str,
    severity: &str,
    latest_export_id: Option<String>,
    previous_export_id: Option<String>,
    metric: &GatewayAnalysisExportTrendMetricSummaryView,
    threshold_value: Option<f64>,
) {
    anomalies.push(GatewayAnalysisExportAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_export_id,
        previous_export_id,
        latest_value: metric.latest_value,
        previous_value: metric.previous_value,
        delta_value: metric.delta_value,
        delta_ratio: metric.delta_ratio,
        threshold_value,
    });
}

fn accumulate_bucket(buckets: &mut std::collections::HashMap<String, usize>, key: &str) {
    let Some(key) = trimmed_owned_ref(key) else {
        return;
    };
    *buckets.entry(key.to_string()).or_default() += 1;
}

fn into_key_buckets(
    buckets: std::collections::HashMap<String, usize>,
) -> Vec<GatewaySummaryBucketKeyView> {
    let mut items = buckets
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    items
}

fn has_pinned_tag(tags: &[String]) -> bool {
    normalize_tag_values(tags).iter().any(|tag| tag == "pinned")
}

fn normalize_tag_values(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for tag in tags {
        let Some(tag) = trimmed_owned_ref(tag) else {
            continue;
        };
        let tag = tag.to_lowercase();
        if seen.insert(tag.clone()) {
            normalized.push(tag);
        }
    }
    normalized
}

fn is_non_active_status_filter(status: Option<&str>) -> bool {
    matches!(
        trimmed_owned_ref_opt(status),
        Some(value) if value != "active"
    )
}

fn validate_created_range(
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> Result<(), GatewayError> {
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo。"));
        }
    }
    Ok(())
}

struct AppliedTextMode {
    text: Option<String>,
    truncated: bool,
}

fn apply_text_mode(text: &str, text_mode: &str, max_text_chars: usize) -> AppliedTextMode {
    let normalized = text.trim();
    if normalized.is_empty() || text_mode == "none" {
        return AppliedTextMode {
            text: None,
            truncated: false,
        };
    }
    let candidate = if text_mode == "preview_redacted" {
        redact_sensitive_text(normalized)
    } else {
        normalized.to_string()
    };
    let truncated = truncate_text(&candidate, max_text_chars);
    AppliedTextMode {
        text: Some(truncated.0),
        truncated: truncated.1,
    }
}

fn truncate_text(text: &str, max_text_chars: usize) -> (String, bool) {
    let char_count = text.chars().count();
    if char_count <= max_text_chars {
        return (text.to_string(), false);
    }
    (text.chars().take(max_text_chars).collect(), true)
}

fn truncate_error_message(text: &str, max_chars: usize) -> String {
    truncate_text(text, max_chars).0
}

fn redact_sensitive_text(text: &str) -> String {
    let mut result = text.to_string();
    result = email_regex()
        .replace_all(&result, "[REDACTED_EMAIL]")
        .into_owned();
    result = platform_api_key_regex()
        .replace_all(&result, "[REDACTED_API_KEY]")
        .into_owned();
    result = secret_key_regex()
        .replace_all(&result, "[REDACTED_SECRET]")
        .into_owned();
    result = bearer_regex()
        .replace_all(&result, "$1 [REDACTED_TOKEN]")
        .into_owned();
    result = query_secret_regex()
        .replace_all(&result, "$1[REDACTED]")
        .into_owned();
    result = field_secret_regex()
        .replace_all(&result, "$1 [REDACTED]")
        .into_owned();
    result
}

fn email_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}\b").expect("email regex")
    })
}

fn platform_api_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b(?:new_api|neuro|nl_bundle_[A-Za-z0-9\-]+|nl_(?:tk|tm|rq)_[A-Za-z0-9\-]+)_[A-Za-z0-9._\-]+\b")
            .expect("platform api regex")
    })
}

fn secret_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\bsk-[A-Za-z0-9_\-]{12,}\b").expect("secret regex"))
}

fn bearer_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(Bearer)\s+[A-Za-z0-9._=\-]{12,}\b").expect("bearer regex")
    })
}

fn query_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)([?&](?:token|key|auth|signature|sig|password)=)[^&\s]+")
            .expect("query secret regex")
    })
}

fn field_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b((?:token|secret|api[_-]?key|authorization)\s*[:=])\s*[^\s,;]+")
            .expect("field secret regex")
    })
}

fn coerce_message_export(message: &Value) -> Option<GatewayAnalysisExportMessageView> {
    let record = message.as_object()?;
    let role = record
        .get("role")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if !matches!(role.as_str(), "system" | "user" | "assistant" | "tool") {
        return None;
    }
    let tool_calls = record
        .get("toolCalls")
        .and_then(Value::as_array)
        .or_else(|| record.get("tool_calls").and_then(Value::as_array))
        .map(|items| items.len())
        .unwrap_or_default();
    Some(GatewayAnalysisExportMessageView {
        role,
        name: record
            .get("name")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        tool_call_id: record
            .get("toolCallId")
            .and_then(Value::as_str)
            .or_else(|| record.get("tool_call_id").and_then(Value::as_str))
            .map(ToString::to_string),
        text: coerce_message_text(record),
        tool_call_count: tool_calls,
    })
}

fn coerce_message_text(record: &serde_json::Map<String, Value>) -> String {
    record
        .get("content")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .map(coerce_text_from_part)
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn coerce_text_from_part(part: &Value) -> String {
    let Some(record) = part.as_object() else {
        return String::new();
    };
    if let Some(value) = record.get("text").and_then(Value::as_str) {
        return value.to_string();
    }
    if let Some(value) = record.get("value").and_then(Value::as_str) {
        return value.to_string();
    }
    if let Some(value) = record.get("imageUrl").and_then(Value::as_str) {
        return format!("[image] {value}");
    }
    if let Some(value) = record.get("image_url").and_then(Value::as_str) {
        return format!("[image] {value}");
    }
    if let Some(url) = record
        .get("image_url")
        .and_then(Value::as_object)
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
    {
        return format!("[image] {url}");
    }
    if record
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "json" || value == "raw")
    {
        if let Some(value) = record.get("value") {
            return serde_json::to_string(value).unwrap_or_default();
        }
        return serde_json::to_string(part).unwrap_or_default();
    }
    String::new()
}

fn coerce_request_tool_names(request_artifact: Option<&Value>) -> Vec<String> {
    let tools = request_artifact
        .and_then(|artifact| artifact.get("canonicalRequest"))
        .and_then(|value| value.get("tools"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tool in tools {
        let Some(name) = tool
            .as_object()
            .and_then(|record| record.get("name"))
            .and_then(Value::as_str)
            .and_then(trimmed_owned_ref)
        else {
            continue;
        };
        if seen.insert(name.to_string()) {
            result.push(name.to_string());
        }
    }
    result
}

fn coerce_response_tool_names(response_artifact: Option<&Value>) -> Vec<String> {
    let tool_calls = response_artifact
        .and_then(|artifact| artifact.get("result"))
        .and_then(|value| value.get("toolCalls"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for tool_call in tool_calls {
        let Some(name) = tool_call
            .as_object()
            .and_then(|record| record.get("name"))
            .and_then(Value::as_str)
            .and_then(trimmed_owned_ref)
        else {
            continue;
        };
        if seen.insert(name.to_string()) {
            result.push(name.to_string());
        }
    }
    result
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
}

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn trimmed_owned_ref(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn trimmed_owned_ref_opt(value: Option<&str>) -> Option<&str> {
    value.and_then(trimmed_owned_ref)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_text_mode, build_analysis_export_file_view, build_analysis_export_inventory_summary,
        build_manifest_artifacts, matches_persisted_analysis_export_view_filters,
        normalize_export_tags, redact_sensitive_text, GatewayAnalysisExportFilterView,
        GatewayAnalysisExportManifest, GatewayPersistedAnalysisExportFilters,
        GatewayPersistedAnalysisExportView,
    };
    use time::OffsetDateTime;

    #[test]
    fn redact_sensitive_text_scrubs_common_secrets() {
        let redacted = redact_sensitive_text(
            "contact me at demo@example.com with neuro_abc and nl_tk_bundle-1_key-1 and sk-secret-123456789012 plus Bearer tokenvalue123456",
        );
        assert!(redacted.contains("[REDACTED_EMAIL]"));
        assert!(redacted.contains("[REDACTED_API_KEY]"));
        assert!(redacted.contains("[REDACTED_SECRET]"));
        assert!(redacted.contains("Bearer [REDACTED_TOKEN]"));
    }

    #[test]
    fn apply_text_mode_hides_text_for_none() {
        let applied = apply_text_mode("hello", "none", 10);
        assert!(applied.text.is_none());
        assert!(!applied.truncated);
    }

    #[test]
    fn normalize_export_tags_deduplicates_and_lowercases() {
        let tags = normalize_export_tags(Some(&vec![
            "Pinned".to_string(),
            "pinned".to_string(),
            "Trace".to_string(),
        ]))
        .expect("normalize tags");
        assert_eq!(tags, vec!["pinned".to_string(), "trace".to_string()]);
    }

    #[test]
    fn build_manifest_artifacts_emits_manifest_and_dataset_files() {
        let dataset = build_analysis_export_file_view(
            "dataset_jsonl",
            "ai-gateway/analysis-exports/export-1/dataset.jsonl".to_string(),
            "application/x-ndjson",
            b"{\"row\":1}\n",
            Some(1),
        );
        let manifest = build_manifest_artifacts(
            "export-1",
            Some("Export".to_string()),
            vec!["tag".to_string()],
            "2026-04-13T00:00:00Z".to_string(),
            None,
            GatewayAnalysisExportFilterView {
                project_id: Some("project-1".to_string()),
                route_policy_id: None,
                provider_account_id: None,
                session_id: None,
                api_key_id: None,
                response_id: None,
                protocol_family: None,
                status: None,
                endpoint_kind: None,
                stream: None,
                error_code: None,
                fallback_eligible: None,
                created_from: None,
                created_to: None,
                artifact_available: None,
                limit: 10,
                text_mode: "preview_redacted".to_string(),
                max_text_chars: 4000,
            },
            1,
            1,
            1,
            dataset,
            "ai-gateway/analysis-exports/export-1/manifest.json".to_string(),
        )
        .expect("build manifest");
        assert_eq!(manifest.manifest.files.len(), 2);
        assert_eq!(manifest.manifest.files[0].kind, "manifest");
        assert_eq!(manifest.manifest.files[1].kind, "dataset_jsonl");
        assert!(!manifest.manifest_body.is_empty());
    }

    #[test]
    fn persisted_export_filters_match_label_tag_and_created_window() {
        let export = sample_persisted_export(
            "export-1",
            Some("Pinned Export"),
            vec!["Pinned".to_string(), "trace".to_string()],
            Some("project-1"),
            "preview_redacted",
            Some("2026-04-14T10:00:00Z"),
            "2026-04-14T09:00:00Z",
        );
        let filters = GatewayPersistedAnalysisExportFilters {
            export_id: None,
            label: Some("pinned".to_string()),
            tag: Some("trace".to_string()),
            project_id: Some("project-1".to_string()),
            status: Some("active".to_string()),
            text_mode: Some("preview_redacted".to_string()),
            created_from: None,
            created_to: None,
            limit: None,
        };
        let created_from = OffsetDateTime::parse(
            "2026-04-14T08:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse created_from");
        let created_to = OffsetDateTime::parse(
            "2026-04-14T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse created_to");
        assert!(matches_persisted_analysis_export_view_filters(
            &export,
            &filters,
            Some(created_from),
            Some(created_to),
        ));
    }

    #[test]
    fn analysis_export_inventory_summary_tracks_pinned_and_expiring_counts() {
        let now = OffsetDateTime::parse(
            "2026-04-14T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("parse now");
        let exports = vec![
            sample_persisted_export(
                "export-1",
                Some("Pinned Export"),
                vec!["pinned".to_string(), "trace".to_string()],
                Some("project-1"),
                "preview_redacted",
                Some("2026-04-14T18:00:00Z"),
                "2026-04-14T09:00:00Z",
            ),
            sample_persisted_export(
                "export-2",
                Some("Expired Export"),
                vec![],
                Some("project-1"),
                "full",
                Some("2026-04-14T11:00:00Z"),
                "2026-04-14T08:00:00Z",
            ),
        ];
        let summary = build_analysis_export_inventory_summary(&exports, now);
        assert_eq!(summary.total_exports, 2);
        assert_eq!(summary.active_exports, 2);
        assert_eq!(summary.pinned_exports, 1);
        assert_eq!(summary.expiring_within_24_hours, 1);
        assert_eq!(summary.expired_active_exports, 1);
        assert_eq!(summary.by_project[0].key, "project-1");
        assert_eq!(summary.by_text_mode.len(), 2);
    }

    fn sample_persisted_export(
        export_id: &str,
        label: Option<&str>,
        tags: Vec<String>,
        project_id: Option<&str>,
        text_mode: &str,
        retention_expires_at: Option<&str>,
        created_at: &str,
    ) -> GatewayPersistedAnalysisExportView {
        let filters = GatewayAnalysisExportFilterView {
            project_id: project_id.map(ToString::to_string),
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            response_id: None,
            protocol_family: None,
            status: None,
            endpoint_kind: None,
            stream: None,
            error_code: None,
            fallback_eligible: None,
            created_from: None,
            created_to: None,
            artifact_available: None,
            limit: 10,
            text_mode: text_mode.to_string(),
            max_text_chars: 4000,
        };
        let manifest = GatewayAnalysisExportManifest {
            schema_version: 1,
            export_id: export_id.to_string(),
            label: label.map(ToString::to_string),
            tags: tags.clone(),
            created_at: created_at.to_string(),
            retention_expires_at: retention_expires_at.map(ToString::to_string),
            filters: filters.clone(),
            sample_count: 3,
            request_artifact_count: 2,
            response_artifact_count: 1,
            files: vec![build_analysis_export_file_view(
                "manifest",
                format!("ai-gateway/analysis-exports/{export_id}/manifest.json"),
                "application/json",
                b"{}",
                None,
            )],
        };
        GatewayPersistedAnalysisExportView {
            export_id: export_id.to_string(),
            label: label.map(ToString::to_string),
            tags,
            status: "active".to_string(),
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
            object_prefix: format!("ai-gateway/analysis-exports/{export_id}"),
            filters,
            sample_count: 3,
            request_artifact_count: 2,
            response_artifact_count: 1,
            retention_expires_at: retention_expires_at.map(ToString::to_string),
            cleaned_up_at: None,
            last_cleanup_error: None,
            files: manifest.files.clone(),
            manifest,
        }
    }
}
