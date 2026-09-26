use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Row};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;

use super::{format_timestamp, map_db_error};

mod analysis;
mod filters;
mod metrics;
mod persistence;
mod prompt_cache;
mod provider_routing;
mod queries;
mod summary;

pub use analysis::{list_analysis_samples, summarize_analysis};
pub use persistence::{
    create_request_audit, finalize_request_audit, update_request_audit_artifact_keys,
};
pub use prompt_cache::{
    get_prompt_cache_metrics, get_prompt_cache_trend_report, summarize_prompt_cache,
};
pub use provider_routing::{
    get_provider_routing_anomaly_report, summarize_provider_routing_analysis,
};
pub use queries::{get_request_audit, list_request_audits};
pub use summary::summarize_request_audits;

use filters::{non_empty, parse_request_audit_created_range, push_request_audit_filters};
use metrics::{
    accumulate_bucket, build_distribution, build_ts_distribution, into_keyed_summary_buckets,
    into_summary_buckets, round_metric,
};
#[cfg(test)]
use prompt_cache::build_prompt_cache_summary_view;
#[cfg(test)]
use provider_routing::{
    build_provider_routing_anomaly_report, build_provider_routing_anomaly_thresholds,
    build_provider_routing_summary,
};

#[derive(Debug, Clone)]
pub struct CreateRequestAuditInput {
    pub project_id: String,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub stream: bool,
    pub route_attempt_count: u32,
    pub response_id: String,
    pub previous_response_id: Option<String>,
    pub route_trace: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct FinalizeRequestAuditInput {
    pub status: String,
    pub upstream_status: Option<u16>,
    pub duration_ms: u64,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub error_summary: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub route_attempt_count: u32,
    pub route_trace: Option<Value>,
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuditFilters {
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditView {
    pub id: String,
    pub project_id: String,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub session_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub stream: bool,
    pub status: String,
    pub upstream_status: Option<i32>,
    pub duration_ms: Option<i32>,
    pub prompt_tokens: Option<i32>,
    pub completion_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub cache_creation_input_tokens: Option<i32>,
    pub cache_read_input_tokens: Option<i32>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub error_summary: Option<String>,
    pub route_trace: Option<Value>,
    pub analysis_profile: Option<Value>,
    pub request_artifact_object_key: Option<String>,
    pub response_artifact_object_key: Option<String>,
    pub response_id: String,
    pub previous_response_id: Option<String>,
    pub client_disconnected_at: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditSummaryBucketView {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySummaryBucketKeyView {
    pub key: String,
    pub count: usize,
}

/// One hourly slice of a provider account's traffic, used by the console to
/// draw an availability strip without downloading raw audit rows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditProviderWindowView {
    pub label: String,
    pub bucket_start: String,
    pub total_requests: usize,
    pub success_count: usize,
    pub failure_count: usize,
}

/// Per-provider-account status breakdown over the same window the summary was
/// asked for. `by_provider_account` only carries totals, which cannot express a
/// success rate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditProviderStatsView {
    pub provider_account_id: String,
    pub total_requests: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub running_count: usize,
    pub last_request_at: Option<String>,
    pub windows: Vec<GatewayRequestAuditProviderWindowView>,
    /// Same numbers split by upstream model, keyed the way the cost overview
    /// keys its model rows so the console can join the two without guessing.
    pub models: Vec<GatewayRequestAuditProviderModelStatsView>,
}

/// One provider account's traffic for a single upstream model. Model cards and
/// entitlement scope rows are per model, and a provider account that serves
/// several models cannot answer them from its totals.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditProviderModelStatsView {
    pub model: String,
    pub total_requests: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub running_count: usize,
    pub last_request_at: Option<String>,
    pub windows: Vec<GatewayRequestAuditProviderWindowView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRequestAuditSummaryView {
    pub total_requests: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub running_count: usize,
    pub fallback_eligible_failures: usize,
    pub fallback_exhausted_failures: usize,
    pub by_status: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_provider_account: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_endpoint_kind: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_error_code: Vec<GatewayRequestAuditSummaryBucketView>,
    pub provider_accounts: Vec<GatewayRequestAuditProviderStatsView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisSampleView {
    pub request_audit_id: String,
    pub response_id: String,
    pub project_id: String,
    pub route_policy_id: Option<String>,
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
    pub cache_creation_input_tokens: Option<i32>,
    pub cache_read_input_tokens: Option<i32>,
    pub analysis_profile: Option<Value>,
    pub request_artifact_object_key: Option<String>,
    pub response_artifact_object_key: Option<String>,
    pub route_trace: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisMetricDistributionView {
    pub avg: Option<f64>,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisSummaryView {
    pub total_samples: usize,
    pub completed_samples: usize,
    pub failed_samples: usize,
    pub cancelled_samples: usize,
    pub stream_samples: usize,
    pub tool_request_samples: usize,
    pub tool_response_samples: usize,
    pub system_prompt_samples: usize,
    pub reasoning_samples: usize,
    pub metadata_samples: usize,
    pub explicit_session_samples: usize,
    pub previous_response_samples: usize,
    pub request_artifact_samples: usize,
    pub response_artifact_samples: usize,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub total_cache_creation_input_tokens: i64,
    pub total_cache_read_input_tokens: i64,
    pub request_text_chars: GatewayAnalysisMetricDistributionView,
    pub response_text_chars: GatewayAnalysisMetricDistributionView,
    pub first_token_latency_ms: GatewayAnalysisMetricDistributionView,
    pub stream_chunk_count: GatewayAnalysisMetricDistributionView,
    pub by_protocol_family: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_endpoint_kind: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_resolved_model: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_provider_account: Vec<GatewayRequestAuditSummaryBucketView>,
    pub by_status: Vec<GatewayRequestAuditSummaryBucketView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisSummaryView {
    pub total_samples: usize,
    pub selected_provider_samples: usize,
    pub degraded_selected_provider_samples: usize,
    pub saturated_selected_provider_samples: usize,
    pub breaker_open_selected_provider_samples: usize,
    pub routing_score: GatewayAnalysisMetricDistributionView,
    pub health_weight: GatewayAnalysisMetricDistributionView,
    pub capacity_weight: GatewayAnalysisMetricDistributionView,
    pub by_selected_provider: Vec<GatewaySummaryBucketKeyView>,
    pub by_degradation_reason: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisFilterView {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub endpoint_kind: Option<String>,
    pub status: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
    pub routing_score_warning_threshold: f64,
    pub routing_score_critical_threshold: f64,
    pub degraded_route_warning_threshold: f64,
    pub degraded_route_critical_threshold: f64,
    pub saturated_route_warning_threshold: f64,
    pub saturated_route_critical_threshold: f64,
    pub breaker_open_route_warning_threshold: f64,
    pub breaker_open_route_critical_threshold: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderRoutingAnalysisAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayProviderRoutingAnalysisFilterView,
    pub profile_key: String,
    pub thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
    pub summary: GatewayProviderRoutingAnalysisSummaryView,
    pub anomalies: Vec<GatewayProviderRoutingAnalysisAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayProviderRoutingAnalysisAnomalyOverrides {
    pub routing_score_warning_threshold: Option<f64>,
    pub routing_score_critical_threshold: Option<f64>,
    pub degraded_route_warning_threshold: Option<f64>,
    pub degraded_route_critical_threshold: Option<f64>,
    pub saturated_route_warning_threshold: Option<f64>,
    pub saturated_route_critical_threshold: Option<f64>,
    pub breaker_open_route_warning_threshold: Option<f64>,
    pub breaker_open_route_critical_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheMetricsView {
    pub hit_requests: usize,
    pub creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheSummaryView {
    pub total_requests: usize,
    pub cache_hit_requests: usize,
    pub cache_creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_control_coverage_requests: usize,
    pub total_tokens_saved: i64,
    pub total_cache_creation_input_tokens: i64,
    pub estimated_cost_saved_usd: f64,
    pub cache_hit_rate: f64,
    pub cache_control_coverage_rate: f64,
    pub input_price_per_million: f64,
    pub cached_input_price_per_million: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheTrendPointView {
    pub bucket_start: String,
    pub total_requests: usize,
    pub cache_hit_requests: usize,
    pub cache_creation_requests: usize,
    pub client_marked_requests: usize,
    pub auto_applied_requests: usize,
    pub cache_control_coverage_requests: usize,
    pub total_tokens_saved: i64,
    pub total_cache_creation_input_tokens: i64,
    pub estimated_cost_saved_usd: f64,
    pub cache_hit_rate: f64,
    pub cache_control_coverage_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPromptCacheTrendReportView {
    pub bucket_size: String,
    pub summary: GatewayPromptCacheSummaryView,
    pub points: Vec<GatewayPromptCacheTrendPointView>,
}

#[cfg(test)]
mod tests;
