use std::collections::{BTreeMap, HashMap, HashSet};

use deadpool_redis::redis::AsyncCommands;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;
use crate::redis::{keys::provider_breaker_open_key, pool::RedisPool};

use super::analysis_exports::{
    get_analysis_export_anomaly_report, GatewayAnalysisExportAnomalyOverrides,
    GatewayPersistedAnalysisExportFilters,
};
use super::anomaly_incidents::{
    list_anomaly_incidents, sync_analysis_export_anomaly_incidents,
    sync_provider_routing_anomaly_incidents, sync_rate_limit_hotspot_anomaly_incidents,
    update_anomaly_incident_follow_up, GatewayAnalysisAnomalyIncidentFilters,
    GatewayAnalysisAnomalyIncidentFollowUpInput, GatewayAnalysisAnomalyIncidentView,
    GatewayAnalysisExportAutoEscalationConfig, GatewaySyncAnalysisExportAnomalyIncidentsInput,
    GatewaySyncAnalysisExportAnomalyIncidentsResult,
    GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult,
    GatewaySyncRateLimitHotspotAnomalyIncidentsResult,
};
use super::rate_limit_hotspots::{
    persist_rate_limit_hotspot_anomaly_snapshot, GatewayRateLimitHotspotAnomalyOverrides,
};
use super::request_audits::{
    summarize_analysis, GatewayAnalysisSummaryView, GatewayProviderRoutingAnalysisAnomalyOverrides,
    GatewaySummaryBucketKeyView, RequestAuditFilters,
};
use super::routing::{
    GatewayRateLimitDefinition, GatewayRoutePolicyConfig, GatewayRoutePolicyView,
};
use super::{format_timestamp, map_db_error, save_route_policy, SaveRoutePolicyInput};

const DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES: i32 = 180;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunFilters {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub dry_run: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicyFilters {
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
pub struct GatewayAnalysisAnomalyRemediationQueueFilters {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationActionView {
    pub action_key: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub priority: String,
    pub route_policy_id: Option<String>,
    pub executable: bool,
    pub execution_mode: String,
    pub default_execution_input: Option<Value>,
    pub recommended_changes: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
    pub provider_max_concurrent_requests: Option<i32>,
    pub pre_stream_fallback_enabled: Option<bool>,
    pub allowed_provider_account_ids: Option<Vec<String>>,
    pub project_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub api_key_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub model_rate_limit_key: Option<String>,
    pub model_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub endpoint_rate_limit_key: Option<String>,
    pub endpoint_rate_limit: Option<super::GatewayRateLimitDefinition>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteGatewayAnalysisAnomalyIncidentRemediationInput {
    pub action_key: String,
    pub dry_run: Option<bool>,
    pub note: Option<String>,
    pub incident_follow_up: Option<GatewayAnalysisAnomalyIncidentFollowUpInput>,
    pub route_policy_patch: Option<GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationPlanView {
    pub generated_at: String,
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub overview: String,
    pub actions: Vec<GatewayAnalysisAnomalyIncidentRemediationActionView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationRunView {
    pub id: String,
    pub incident_id: String,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: String,
    pub title: String,
    pub execution_mode: String,
    pub status: String,
    pub dry_run: bool,
    pub actor_user_id: String,
    pub note: Option<String>,
    pub input: Option<Value>,
    pub result: Option<Value>,
    pub before_incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub after_incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub before_route_policy: Option<GatewayRoutePolicyView>,
    pub after_route_policy: Option<GatewayRoutePolicyView>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationQueueItemView {
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub action: GatewayAnalysisAnomalyIncidentRemediationActionView,
    pub remediation_due: bool,
    pub next_execution_status: Option<String>,
    pub next_run_due_at: Option<String>,
    pub blocked_reason: Option<String>,
    pub latest_run: Option<GatewayAnalysisAnomalyIncidentRemediationRunView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicyView {
    pub id: String,
    pub name: String,
    pub status: String,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub profile_key: String,
    pub thresholds: Value,
    pub auto_sync_enabled: bool,
    pub auto_sync_interval_minutes: Option<i32>,
    pub last_synced_at: Option<String>,
    pub last_sync_status: Option<String>,
    pub last_sync_error: Option<String>,
    pub next_sync_due_at: Option<String>,
    pub sync_due: bool,
    pub auto_escalate_enabled: bool,
    pub escalate_severity_threshold: Option<String>,
    pub escalate_after_sync_count: Option<i32>,
    pub auto_escalate_owner_user_id: Option<String>,
    pub auto_escalate_follow_up_status: Option<String>,
    pub auto_remediation_enabled: bool,
    pub auto_remediation_interval_minutes: Option<i32>,
    pub auto_remediation_dry_run_first: bool,
    pub auto_remediation_action_keys: Option<Vec<String>>,
    pub auto_remediation_max_apply_runs_per_incident: Option<i32>,
    pub auto_remediation_require_alert_before_apply: bool,
    pub auto_remediation_freeze_on_provider_health_degrade: bool,
    pub alerting_enabled: bool,
    pub alert_interval_minutes: Option<i32>,
    pub notify_operators_on_escalation: bool,
    pub notify_owner_on_escalation: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySummaryView {
    pub total_policies: usize,
    pub enabled_policies: usize,
    pub disabled_policies: usize,
    pub auto_sync_enabled_policies: usize,
    pub auto_escalate_enabled_policies: usize,
    pub auto_remediation_enabled_policies: usize,
    pub alerting_enabled_policies: usize,
    pub due_policies: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_sync_status: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySyncView {
    pub policy: GatewayAnalysisAnomalyPolicyView,
    pub sync_kind: String,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
    pub sync: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSyncInput {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSyncView {
    pub policy: Option<GatewayAnalysisAnomalyPolicyView>,
    pub sync_kind: String,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
    pub sync: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySweepItemView {
    pub policy_id: String,
    pub policy_name: String,
    pub status: String,
    pub error: Option<String>,
    pub last_synced_at: Option<String>,
    pub next_sync_due_at: Option<String>,
    pub sync_due: bool,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySweepView {
    pub started_at: String,
    pub completed_at: String,
    pub limit: usize,
    pub attempted_count: usize,
    pub ok_count: usize,
    pub error_count: usize,
    pub skipped_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyPolicySweepItemView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertGatewayAnalysisAnomalyPolicyInput {
    pub id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub profile_key: Option<String>,
    pub thresholds: Option<Value>,
    pub auto_sync_enabled: Option<bool>,
    pub auto_sync_interval_minutes: Option<i32>,
    pub auto_escalate_enabled: Option<bool>,
    pub escalate_severity_threshold: Option<String>,
    pub escalate_after_sync_count: Option<i32>,
    pub auto_escalate_owner_user_id: Option<String>,
    pub auto_escalate_follow_up_status: Option<String>,
    pub auto_remediation_enabled: Option<bool>,
    pub auto_remediation_interval_minutes: Option<i32>,
    pub auto_remediation_dry_run_first: Option<bool>,
    pub auto_remediation_action_keys: Option<Vec<String>>,
    pub auto_remediation_max_apply_runs_per_incident: Option<i32>,
    pub auto_remediation_require_alert_before_apply: Option<bool>,
    pub auto_remediation_freeze_on_provider_health_degrade: Option<bool>,
    pub alerting_enabled: Option<bool>,
    pub alert_interval_minutes: Option<i32>,
    pub notify_operators_on_escalation: Option<bool>,
    pub notify_owner_on_escalation: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationQueueView {
    pub generated_at: String,
    pub limit: usize,
    pub due_only: bool,
    pub item_count: usize,
    pub due_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyIncidentRemediationQueueItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentAlertQueueItemView {
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub alert_interval_minutes: i32,
    pub alert_due: bool,
    pub next_alert_due_at: Option<String>,
    pub notify_operators: bool,
    pub notify_owner: bool,
    pub alert_level: i32,
    pub webhook_severity: String,
    pub remediation_action_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentAlertQueueView {
    pub generated_at: String,
    pub limit: usize,
    pub due_only: bool,
    pub incident_count: usize,
    pub due_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyIncidentAlertQueueItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationSweepItemView {
    pub incident_id: String,
    pub action_key: String,
    pub status: String,
    pub execution_status: Option<String>,
    pub run_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationSweepView {
    pub started_at: String,
    pub completed_at: String,
    pub limit: usize,
    pub attempted_count: usize,
    pub dry_run_count: usize,
    pub applied_count: usize,
    pub error_count: usize,
    pub skipped_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyRemediationSweepItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunSummaryView {
    pub total_runs: usize,
    pub dry_run_runs: usize,
    pub applied_runs: usize,
    pub failed_runs: usize,
    pub distinct_incident_count: usize,
    pub route_policy_changed_runs: usize,
    pub incident_changed_runs: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub by_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactMetricView {
    pub before_value: Option<f64>,
    pub after_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunImpactView {
    pub generated_at: String,
    pub run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    pub incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub anchor_at: String,
    pub window_minutes: i32,
    pub before_window: GatewayAnalysisAnomalyRemediationImpactWindowView,
    pub after_window: GatewayAnalysisAnomalyRemediationImpactWindowView,
    pub metrics: GatewayAnalysisAnomalyRemediationRunImpactMetricsView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactWindowView {
    pub started_at: String,
    pub ended_at: String,
    pub summary: GatewayAnalysisSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunImpactMetricsView {
    pub completion_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub cancellation_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub stream_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub tool_request_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub tool_response_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub prompt_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub completion_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub request_text_chars_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub response_text_chars_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub stream_chunk_count_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
    pub improved_runs: usize,
    pub regressed_runs: usize,
    pub neutral_runs: usize,
    pub unavailable_runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationActionEffectivenessView {
    pub action_key: String,
    pub run_count: usize,
    pub impacted_run_count: usize,
    pub unavailable_run_count: usize,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
    pub generated_at: String,
    pub window_minutes: i32,
    pub total_runs: usize,
    pub impacted_runs: usize,
    pub unavailable_runs: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub actions: Vec<GatewayAnalysisAnomalyRemediationActionEffectivenessView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactCaptureView {
    pub run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    pub impact: GatewayAnalysisAnomalyRemediationRunImpactView,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub dry_run: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
    pub lookback_hours: Option<i32>,
    pub window_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView,
    pub summary: GatewayAnalysisAnomalyRemediationEffectivenessSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
    pub total_snapshots: usize,
    pub total_runs: usize,
    pub total_impacted_runs: usize,
    pub total_unavailable_runs: usize,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_label: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView {
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
    pub improved_rate: Option<f64>,
    pub regressed_rate: Option<f64>,
    pub neutral_rate: Option<f64>,
    pub unavailable_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
    pub snapshot: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    pub total_runs: usize,
    pub impacted_run_rate: Option<f64>,
    pub unavailable_run_rate: Option<f64>,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub request_artifact_coverage:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub response_artifact_coverage:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub first_token_latency_ms_avg:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView {
    pub latest_snapshot_id: Option<String>,
    pub previous_snapshot_id: Option<String>,
    pub total_runs: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub impacted_run_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub unavailable_run_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub completion_rate_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub failure_rate_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub request_artifact_coverage_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub response_artifact_coverage_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub first_token_latency_ms_avg_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub total_tokens_per_sample_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    pub matched_snapshots_count: usize,
    pub window_size: usize,
    pub inventory_summary:
        GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView,
    pub points: Vec<GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView>,
    pub summary: Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
    pub impacted_run_rate_warning_threshold: f64,
    pub impacted_run_rate_critical_threshold: f64,
    pub unavailable_run_rate_warning_threshold: f64,
    pub unavailable_run_rate_critical_threshold: f64,
    pub completion_rate_regressed_warning_threshold: f64,
    pub completion_rate_regressed_critical_threshold: f64,
    pub failure_rate_regressed_warning_threshold: f64,
    pub failure_rate_regressed_critical_threshold: f64,
    pub request_artifact_regressed_warning_threshold: f64,
    pub request_artifact_regressed_critical_threshold: f64,
    pub response_artifact_regressed_warning_threshold: f64,
    pub response_artifact_regressed_critical_threshold: f64,
    pub first_token_latency_regressed_warning_threshold: f64,
    pub first_token_latency_regressed_critical_threshold: f64,
    pub total_tokens_regressed_warning_threshold: f64,
    pub total_tokens_regressed_critical_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub latest_snapshot_id: Option<String>,
    pub previous_snapshot_id: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    pub profile_key: String,
    pub thresholds: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
    pub latest_snapshot: Option<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>,
    pub previous_snapshot: Option<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>,
    pub trend_summary: Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView>,
    pub anomalies: Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub profile_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView {
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
    pub lookback_hours: Option<i32>,
    pub profile_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView,
    pub report: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GatewayRoutePolicyRoutingAnomalyAutoRemediationProfile {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    dry_run_first: Option<bool>,
    require_alert_before_apply: Option<bool>,
    freeze_on_provider_health_degrade: Option<bool>,
    max_apply_runs_per_incident: Option<i32>,
    action_keys_by_code: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GatewayRoutePolicyRateLimitHotspotAutoRemediationProfile {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    dry_run_first: Option<bool>,
    require_alert_before_apply: Option<bool>,
    freeze_on_provider_health_degrade: Option<bool>,
    max_apply_runs_per_incident: Option<i32>,
    action_by_code: Option<HashMap<String, Option<String>>>,
}

#[derive(Debug, Clone)]
struct AutoRemediationConfig {
    auto_remediation_enabled: bool,
    auto_remediation_interval_minutes: i32,
    auto_remediation_dry_run_first: bool,
    auto_remediation_action_keys: Option<Vec<String>>,
    auto_remediation_max_apply_runs_per_incident: Option<i32>,
    auto_remediation_require_alert_before_apply: bool,
    auto_remediation_freeze_on_provider_health_degrade: bool,
}

struct AlertConfig {
    alerting_enabled: bool,
    alert_interval_minutes: i32,
    notify_operators: bool,
    notify_owner: bool,
}

struct AlertSchedule {
    next_alert_due_at: Option<String>,
    alert_due: bool,
}

struct AlertDeliveryProfile {
    alert_level: i32,
    webhook_severity: String,
}

#[derive(Debug, Clone)]
struct AutoRemediationSchedule {
    remediation_due: bool,
    next_execution_status: Option<String>,
    next_run_due_at: Option<String>,
    blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct IncidentSyncContext {
    entity_key: Option<String>,
    snapshot_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyRemediationRunRow {
    id: String,
    incident_id: String,
    policy_id: Option<String>,
    route_policy_id: Option<String>,
    action_key: String,
    title: String,
    execution_mode: String,
    status: String,
    dry_run: bool,
    actor_user_id: String,
    note: Option<String>,
    input: Option<Json<Value>>,
    result: Option<Json<Value>>,
    before_incident: Option<Json<Value>>,
    after_incident: Option<Json<Value>>,
    before_route_policy: Option<Json<Value>>,
    after_route_policy: Option<Json<Value>>,
    error_summary: Option<String>,
    created_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyPolicyRow {
    id: String,
    name: String,
    status: String,
    project_id: Option<String>,
    route_policy_id: Option<String>,
    tag: Option<String>,
    text_mode: Option<String>,
    profile_key: String,
    thresholds: Json<Value>,
    auto_sync_enabled: bool,
    auto_sync_interval_minutes: Option<i32>,
    last_synced_at: Option<OffsetDateTime>,
    last_sync_status: Option<String>,
    last_sync_error: Option<String>,
    auto_escalate_enabled: bool,
    escalate_severity_threshold: Option<String>,
    escalate_after_sync_count: Option<i32>,
    auto_escalate_owner_user_id: Option<String>,
    auto_escalate_follow_up_status: Option<String>,
    auto_remediation_enabled: bool,
    auto_remediation_interval_minutes: Option<i32>,
    auto_remediation_dry_run_first: bool,
    auto_remediation_action_keys: Option<Json<Vec<String>>>,
    auto_remediation_max_apply_runs_per_incident: Option<i32>,
    auto_remediation_require_alert_before_apply: bool,
    auto_remediation_freeze_on_provider_health_degrade: bool,
    alerting_enabled: bool,
    alert_interval_minutes: Option<i32>,
    notify_operators_on_escalation: bool,
    notify_owner_on_escalation: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentHistoryLookupRow {
    metadata: Option<Json<Value>>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayRoutePolicyRow {
    id: String,
    project_id: String,
    name: String,
    is_default: bool,
    enabled: bool,
    config: Json<Value>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub async fn get_anomaly_incident_remediation_plan(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationPlanView, GatewayError> {
    let incident = get_incident_by_id(pool, incident_id).await?;
    let policy = if let Some(policy_id) = incident.policy_id.as_deref() {
        find_anomaly_policy_by_id(pool, policy_id).await?
    } else {
        None
    };
    let resolved_route_policy_id = policy
        .as_ref()
        .and_then(|item| item.route_policy_id.clone())
        .or_else(|| incident.route_policy_id.clone());
    let route_policy = if let Some(route_policy_id) = resolved_route_policy_id.as_deref() {
        find_route_policy_by_id(pool, route_policy_id).await?
    } else {
        None
    };
    let incident_context = load_incident_latest_sync_context(pool, &incident.id).await?;
    Ok(build_incident_remediation_plan(
        OffsetDateTime::now_utc(),
        incident,
        policy,
        route_policy,
        incident_context,
    ))
}

pub async fn list_anomaly_incident_alert_queue(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyIncidentFilters,
) -> Result<GatewayAnalysisAnomalyIncidentAlertQueueView, GatewayError> {
    let limit = filters.limit.unwrap_or(50).clamp(1, 200);
    let due_only = filters.due_only == Some(true);
    let reference_time = OffsetDateTime::now_utc();
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            escalation_status: Some("escalated".to_string()),
            limit: Some(limit.max(200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut items = Vec::new();
    for incident in incidents {
        let policy = if let Some(policy_id) = incident.policy_id.as_deref() {
            find_anomaly_policy_by_id(pool, policy_id).await?
        } else {
            None
        };
        let resolved_route_policy_id = policy
            .as_ref()
            .and_then(|item| item.route_policy_id.clone())
            .or_else(|| incident.route_policy_id.clone());
        let route_policy = if let Some(route_policy_id) = resolved_route_policy_id.as_deref() {
            find_route_policy_by_id(pool, route_policy_id).await?
        } else {
            None
        };
        let incident_context = load_incident_latest_sync_context(pool, &incident.id).await?;
        let alert_config = resolve_anomaly_policy_alert_config(policy.as_ref());
        let schedule = resolve_anomaly_incident_alert_schedule(
            &incident.status,
            &incident.escalation_status,
            &alert_config,
            incident.last_alert_attempt_at.as_deref(),
            reference_time,
        );
        if due_only && !schedule.alert_due {
            continue;
        }
        let delivery_profile = resolve_anomaly_alert_delivery_profile(&incident.severity);
        let plan = build_incident_remediation_plan(
            reference_time,
            incident.clone(),
            policy.clone(),
            route_policy.clone(),
            incident_context,
        );
        items.push(GatewayAnalysisAnomalyIncidentAlertQueueItemView {
            incident,
            policy: policy
                .as_ref()
                .and_then(|value| serde_json::to_value(value).ok()),
            route_policy,
            alert_interval_minutes: alert_config.alert_interval_minutes,
            alert_due: schedule.alert_due,
            next_alert_due_at: schedule.next_alert_due_at,
            notify_operators: alert_config.notify_operators,
            notify_owner: alert_config.notify_owner,
            alert_level: delivery_profile.alert_level,
            webhook_severity: delivery_profile.webhook_severity,
            remediation_action_keys: plan
                .actions
                .into_iter()
                .map(|action| action.action_key)
                .collect(),
        });
    }

    items.sort_by(|left, right| {
        if left.alert_due != right.alert_due {
            return if left.alert_due {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if left.incident.severity != right.incident.severity {
            return if left.incident.severity == "critical" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        right.incident.last_seen_at.cmp(&left.incident.last_seen_at)
    });
    items.truncate(limit);

    Ok(GatewayAnalysisAnomalyIncidentAlertQueueView {
        generated_at: format_timestamp(reference_time),
        limit,
        due_only,
        incident_count: items.len(),
        due_count: items.iter().filter(|item| item.alert_due).count(),
        items,
    })
}

pub async fn list_anomaly_remediation_queue(
    pool: &PgPool,
    redis_pool: &RedisPool,
    filters: &GatewayAnalysisAnomalyRemediationQueueFilters,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationQueueView, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let due_only = filters.due_only == Some(true);
    let reference_time = OffsetDateTime::now_utc();
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            incident_id: filters.incident_id.clone(),
            policy_id: filters.policy_id.clone(),
            project_id: filters.project_id.clone(),
            route_policy_id: filters.route_policy_id.clone(),
            owner_user_id: filters.owner_user_id.clone(),
            tag: filters.tag.clone(),
            text_mode: filters.text_mode.clone(),
            status: filters.status.clone(),
            follow_up_status: filters.follow_up_status.clone(),
            escalation_status: Some("escalated".to_string()),
            code: filters.code.clone(),
            severity: filters.severity.clone(),
            due_only: None,
            limit: Some(limit.max(200)),
        },
    )
    .await?;

    let mut items = Vec::new();
    for incident in incidents {
        let policy = if let Some(policy_id) = incident.policy_id.as_deref() {
            find_anomaly_policy_by_id(pool, policy_id).await?
        } else {
            None
        };
        let resolved_route_policy_id = policy
            .as_ref()
            .and_then(|item| item.route_policy_id.clone())
            .or_else(|| incident.route_policy_id.clone());
        let route_policy = if let Some(route_policy_id) = resolved_route_policy_id.as_deref() {
            find_route_policy_by_id(pool, route_policy_id).await?
        } else {
            None
        };
        let incident_context = load_incident_latest_sync_context(pool, &incident.id).await?;
        let plan = build_incident_remediation_plan(
            reference_time,
            incident.clone(),
            policy.clone(),
            route_policy.clone(),
            incident_context,
        );
        let policy_config = resolve_incident_auto_remediation_config(
            policy.as_ref(),
            &incident,
            route_policy.as_ref(),
        );
        for action in plan.actions {
            if !action.executable {
                continue;
            }
            if let Some(action_key) = filters.action_key.as_deref().and_then(trimmed_owned_ref) {
                if action.action_key != action_key {
                    continue;
                }
            }
            if let Some(execution_mode) = filters
                .execution_mode
                .as_deref()
                .and_then(trimmed_owned_ref)
            {
                if action.execution_mode != execution_mode {
                    continue;
                }
            }
            let action_allowed = policy_config
                .auto_remediation_action_keys
                .as_ref()
                .map(|items| items.iter().any(|item| item == &action.action_key))
                .unwrap_or(true);
            let latest_run =
                find_latest_anomaly_remediation_run(pool, &incident.id, &action.action_key).await?;
            let applied_run_count = count_anomaly_remediation_runs_by_status(
                pool,
                &incident.id,
                &action.action_key,
                "applied",
            )
            .await?;
            let provider_health_degraded =
                read_route_policy_health_degraded(pool, redis_pool, route_policy.as_ref()).await?;
            let schedule = resolve_anomaly_remediation_schedule(
                &incident,
                &policy_config,
                action_allowed,
                applied_run_count,
                provider_health_degraded,
                latest_run.as_ref(),
                reference_time,
            );
            if due_only && !schedule.remediation_due {
                continue;
            }
            items.push(GatewayAnalysisAnomalyIncidentRemediationQueueItemView {
                incident: incident.clone(),
                policy: policy
                    .as_ref()
                    .and_then(|value| serde_json::to_value(value).ok()),
                route_policy: route_policy.clone(),
                action,
                remediation_due: schedule.remediation_due,
                next_execution_status: schedule.next_execution_status,
                next_run_due_at: schedule.next_run_due_at,
                blocked_reason: schedule.blocked_reason,
                latest_run,
            });
        }
    }

    items.sort_by(|left, right| {
        if left.remediation_due != right.remediation_due {
            return if left.remediation_due {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if left.incident.severity != right.incident.severity {
            return if left.incident.severity == "critical" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        right.incident.updated_at.cmp(&left.incident.updated_at)
    });
    items.truncate(limit);

    Ok(GatewayAnalysisAnomalyIncidentRemediationQueueView {
        generated_at: format_timestamp(reference_time),
        limit,
        due_only,
        item_count: items.len(),
        due_count: items.iter().filter(|item| item.remediation_due).count(),
        items,
    })
}

pub async fn sweep_anomaly_remediations(
    pool: &PgPool,
    redis_pool: &RedisPool,
    actor_user_id: &str,
    filters: &GatewayAnalysisAnomalyRemediationQueueFilters,
) -> Result<GatewayAnalysisAnomalyRemediationSweepView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id)
        .ok_or_else(|| GatewayError::bad_request("actorUserId 不能为空"))?;
    let started_at = OffsetDateTime::now_utc();
    let limit = filters.limit.unwrap_or(20).clamp(1, 100);
    let queue = list_anomaly_remediation_queue(
        pool,
        redis_pool,
        &GatewayAnalysisAnomalyRemediationQueueFilters {
            due_only: Some(true),
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await?;

    let mut items = Vec::new();
    let mut dry_run_count = 0;
    let mut applied_count = 0;
    let mut error_count = 0;
    let mut skipped_count = 0;

    for item in queue.items {
        let Some(next_execution_status) = item.next_execution_status.as_deref() else {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                incident_id: item.incident.id,
                action_key: item.action.action_key,
                status: "skipped".to_string(),
                execution_status: None,
                run_id: None,
                error: None,
            });
            continue;
        };

        if !item.remediation_due {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                incident_id: item.incident.id,
                action_key: item.action.action_key,
                status: "skipped".to_string(),
                execution_status: Some(next_execution_status.to_string()),
                run_id: None,
                error: None,
            });
            continue;
        }

        let input = remediation_execution_input_from_action(&item.action, next_execution_status)?;
        match execute_anomaly_incident_remediation(pool, actor_user_id, &item.incident.id, input)
            .await
        {
            Ok(run) => {
                if run.status == "dry_run" {
                    dry_run_count += 1;
                } else {
                    applied_count += 1;
                }
                items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                    incident_id: item.incident.id,
                    action_key: item.action.action_key,
                    status: "ok".to_string(),
                    execution_status: Some(run.status.clone()),
                    run_id: Some(run.id),
                    error: None,
                });
            }
            Err(error) => {
                error_count += 1;
                items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                    incident_id: item.incident.id,
                    action_key: item.action.action_key,
                    status: "error".to_string(),
                    execution_status: Some(next_execution_status.to_string()),
                    run_id: None,
                    error: Some(truncate_error_summary(&error.to_string(), 240)),
                });
            }
        }
    }

    Ok(GatewayAnalysisAnomalyRemediationSweepView {
        started_at: format_timestamp(started_at),
        completed_at: format_timestamp(OffsetDateTime::now_utc()),
        limit,
        attempted_count: items.len(),
        dry_run_count,
        applied_count,
        error_count,
        skipped_count,
        items,
    })
}

pub async fn list_anomaly_incident_remediation_runs(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
) -> Result<Vec<GatewayAnalysisAnomalyIncidentRemediationRunView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);

    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        select
          id,
          incident_id,
          policy_id,
          route_policy_id,
          action_key,
          title,
          execution_mode,
          status,
          dry_run,
          actor_user_id,
          note,
          input,
          result,
          before_incident,
          after_incident,
          before_route_policy,
          after_route_policy,
          error_summary,
          created_at,
          completed_at
        from gateway_analysis_anomaly_remediation_runs
        where 1 = 1
        "#,
    );
    push_optional_filter(&mut builder, "incident_id", filters.incident_id.as_deref());
    push_optional_filter(&mut builder, "policy_id", filters.policy_id.as_deref());
    push_optional_filter(
        &mut builder,
        "route_policy_id",
        filters.route_policy_id.as_deref(),
    );
    push_optional_filter(&mut builder, "action_key", filters.action_key.as_deref());
    push_optional_filter(&mut builder, "status", filters.status.as_deref());
    push_optional_filter(
        &mut builder,
        "execution_mode",
        filters.execution_mode.as_deref(),
    );
    if let Some(dry_run) = filters.dry_run {
        builder.push(" and dry_run = ").push_bind(dry_run);
    }
    if let Some(created_from) = created_from {
        builder.push(" and created_at >= ").push_bind(created_from);
    }
    if let Some(created_to) = created_to {
        builder.push(" and created_at <= ").push_bind(created_to);
    }
    builder
        .push(" order by created_at desc limit ")
        .push_bind(i64::try_from(limit).unwrap_or(500));

    let rows = builder
        .build_query_as::<GatewayAnalysisAnomalyRemediationRunRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    rows.into_iter().map(to_remediation_run_view).collect()
}

pub async fn summarize_anomaly_incident_remediation_runs(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
) -> Result<GatewayAnalysisAnomalyRemediationRunSummaryView, GatewayError> {
    let runs = list_anomaly_incident_remediation_runs(
        pool,
        &GatewayAnalysisAnomalyRemediationRunFilters {
            limit: Some(filters.limit.unwrap_or(500).max(500)),
            ..filters.clone()
        },
    )
    .await?;
    Ok(build_remediation_run_summary(&runs))
}

pub async fn execute_anomaly_incident_remediation(
    pool: &PgPool,
    actor_user_id: &str,
    incident_id: &str,
    input: ExecuteGatewayAnalysisAnomalyIncidentRemediationInput,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id)
        .ok_or_else(|| GatewayError::bad_request("actorUserId 不能为空"))?;
    let action_key = trimmed_owned_ref(&input.action_key)
        .ok_or_else(|| GatewayError::bad_request("actionKey 不能为空"))?;
    let plan = get_anomaly_incident_remediation_plan(pool, incident_id).await?;
    let action = plan
        .actions
        .iter()
        .find(|item| item.action_key == action_key)
        .ok_or_else(|| {
            GatewayError::conflict(format!(
                "incident 当前不存在 remediation action: {action_key}"
            ))
        })?
        .clone();
    if !action.executable || action.execution_mode == "informational" {
        return Err(GatewayError::conflict(format!(
            "remediation action {} 仅提供建议，当前不支持直接执行。",
            action.action_key
        )));
    }

    let timestamp = OffsetDateTime::now_utc();
    let dry_run = input.dry_run == Some(true);
    let run_id = uuid::Uuid::new_v4().to_string();
    let note = trimmed_owned(input.note.as_deref());
    let before_incident = plan.incident.clone();
    let before_route_policy = plan.route_policy.clone();

    let mut after_incident = Some(before_incident.clone());
    let mut after_route_policy = before_route_policy.clone();
    let mut result = None;
    let mut error_summary = None;
    let mut status = if dry_run { "dry_run" } else { "applied" }.to_string();

    let execution_result: Result<(), GatewayError> = match action.execution_mode.as_str() {
        "incident_follow_up" => {
            let follow_up_input =
                build_follow_up_input(&before_incident, input.incident_follow_up.clone());
            if dry_run {
                after_incident = Some(simulate_follow_up_update(
                    &before_incident,
                    &follow_up_input,
                    timestamp,
                ));
            } else {
                after_incident = Some(
                    update_anomaly_incident_follow_up(pool, incident_id, follow_up_input).await?,
                );
            }
            result = Some(json!({
                "actionKey": action.action_key,
                "executionMode": action.execution_mode,
                "status": status,
                "changedFields": ["ownerUserId", "followUpStatus", "note", "resolutionNote"],
                "summary": "Updated incident ownership and follow-up fields."
            }));
            Ok(())
        }
        "route_policy_patch" => {
            let before = before_route_policy.clone().ok_or_else(|| {
                GatewayError::conflict("当前 remediation action 缺少可用 route policy")
            })?;
            let patch = build_route_policy_patch(
                &action.action_key,
                &before,
                input.route_policy_patch.clone(),
            )?;
            if dry_run {
                after_route_policy = Some(apply_route_policy_patch(&before, &patch));
            } else {
                let next_policy = apply_route_policy_patch(&before, &patch);
                after_route_policy = Some(
                    save_route_policy(
                        pool,
                        Some(&before.id),
                        SaveRoutePolicyInput {
                            project_id: before.project_id.clone(),
                            name: before.name.clone(),
                            is_default: before.is_default,
                            enabled: before.enabled,
                            config: next_policy.config.clone(),
                        },
                    )
                    .await?,
                );
            }
            result = Some(json!({
                "actionKey": action.action_key,
                "executionMode": action.execution_mode,
                "status": status,
                "changedFields": patch.changed_fields,
                "summary": patch.summary
            }));
            Ok(())
        }
        other => Err(GatewayError::conflict(format!(
            "当前尚未支持 remediation executionMode: {other}"
        ))),
    };

    if let Err(error) = execution_result {
        status = "failed".to_string();
        error_summary = Some(error.to_string());
        result = Some(json!({
            "actionKey": action.action_key,
            "executionMode": action.execution_mode,
            "status": status,
            "changedFields": [],
            "summary": error_summary
        }));
    }

    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        insert into gateway_analysis_anomaly_remediation_runs (
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9,
          $10, $11, $12, $13, $14, $15, $16, $17,
          $18, $19, $19
        )
        returning
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        "#,
    )
    .bind(&run_id)
    .bind(incident_id.trim())
    .bind(Option::<String>::None)
    .bind(after_route_policy.as_ref().map(|item| item.id.as_str()).or(before_route_policy.as_ref().map(|item| item.id.as_str())))
    .bind(&action.action_key)
    .bind(&action.title)
    .bind(&action.execution_mode)
    .bind(&status)
    .bind(dry_run)
    .bind(actor_user_id)
    .bind(note.as_deref())
    .bind(Some(sqlx::types::Json(serde_json::to_value(&input).map_err(|error| GatewayError::server_error(format!("serialize remediation input: {error}")))?)))
    .bind(result.clone().map(sqlx::types::Json))
    .bind(Some(sqlx::types::Json(serde_json::to_value(&before_incident).map_err(|error| GatewayError::server_error(format!("serialize remediation before incident: {error}")))?)))
    .bind(after_incident.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(before_route_policy.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(after_route_policy.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(error_summary.as_deref())
    .bind(timestamp)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    append_remediation_history(
        pool,
        incident_id.trim(),
        actor_user_id,
        &action,
        &status,
        dry_run,
        note.as_deref(),
        after_incident.as_ref().unwrap_or(&before_incident),
        after_route_policy.as_ref().or(before_route_policy.as_ref()),
        &run_id,
        result.as_ref(),
        timestamp,
    )
    .await?;

    to_remediation_run_view(row)
}

pub async fn get_anomaly_incident_remediation_run_impact(
    pool: &PgPool,
    run_id: &str,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationRunImpactView, GatewayError> {
    let run = get_remediation_run_view_by_id(pool, run_id).await?;
    let incident = run.after_incident.clone().or(run.before_incident.clone());
    let project_id = incident
        .as_ref()
        .and_then(|value| value.project_id.clone())
        .ok_or_else(|| {
            GatewayError::conflict("当前 remediation run 缺少 project 作用域，无法计算影响面。")
        })?;
    let route_policy_id = run
        .after_route_policy
        .as_ref()
        .map(|value| value.id.clone())
        .or(run
            .before_route_policy
            .as_ref()
            .map(|value| value.id.clone()))
        .or(run.route_policy_id.clone());
    let anchor_at = parse_filter_timestamp(
        run.completed_at
            .as_deref()
            .or(Some(run.created_at.as_str())),
        "anchorAt",
    )?
    .ok_or_else(|| GatewayError::conflict("当前 remediation run 缺少合法的时间锚点。"))?;
    let window_minutes = normalize_window_minutes(window_minutes);

    let before_started_at = anchor_at - time::Duration::minutes(i64::from(window_minutes));
    let before_ended_at = anchor_at;
    let after_started_at = anchor_at;
    let after_ended_at = anchor_at + time::Duration::minutes(i64::from(window_minutes));

    let before_summary = summarize_analysis(
        pool,
        &RequestAuditFilters {
            project_id: Some(project_id.clone()),
            route_policy_id: route_policy_id.clone(),
            created_from: Some(format_timestamp(before_started_at)),
            created_to: Some(format_timestamp(before_ended_at)),
            limit: Some(1000),
            ..RequestAuditFilters::default()
        },
    )
    .await?;
    let after_summary = summarize_analysis(
        pool,
        &RequestAuditFilters {
            project_id: Some(project_id.clone()),
            route_policy_id: route_policy_id.clone(),
            created_from: Some(format_timestamp(after_started_at)),
            created_to: Some(format_timestamp(after_ended_at)),
            limit: Some(1000),
            ..RequestAuditFilters::default()
        },
    )
    .await?;

    Ok(build_run_impact(
        OffsetDateTime::now_utc(),
        run,
        incident,
        Some(project_id),
        route_policy_id,
        anchor_at,
        window_minutes,
        before_started_at,
        before_ended_at,
        before_summary,
        after_started_at,
        after_ended_at,
        after_summary,
    ))
}

pub async fn capture_anomaly_incident_remediation_run_impact(
    pool: &PgPool,
    actor_user_id: &str,
    run_id: &str,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationImpactCaptureView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id).unwrap_or("management");
    let impact = get_anomaly_incident_remediation_run_impact(pool, run_id, window_minutes).await?;
    let row = get_remediation_run_row_by_id(pool, &impact.run.id).await?;

    let mut result_payload = row.result.map(|value| value.0).unwrap_or_else(|| json!({}));
    if !result_payload.is_object() {
        result_payload = json!({});
    }
    if let Some(object) = result_payload.as_object_mut() {
        object.insert(
            "impactCapture".to_string(),
            json!({
                "capturedAt": impact.generated_at,
                "windowMinutes": impact.window_minutes,
                "impact": impact
            }),
        );
    }

    let updated_row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        update gateway_analysis_anomaly_remediation_runs
        set result = $2
        where id = $1
        returning
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        "#,
    )
    .bind(&impact.run.id)
    .bind(sqlx::types::Json(result_payload))
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    if let Some(incident) = impact.incident.as_ref() {
        append_incident_history(
            pool,
            &impact.run.incident_id,
            "remediation_impact_captured",
            Some(actor_user_id),
            Some(&format!(
                "Captured remediation impact over {} minutes.",
                impact.window_minutes
            )),
            Some(json!({
                "policyId": incident.policy_id,
                "projectId": incident.project_id,
                "routePolicyId": impact.route_policy_id,
                "tag": incident.tag,
                "textMode": incident.text_mode,
                "code": incident.code,
                "severity": incident.severity,
                "status": incident.status,
                "ownerUserId": incident.owner_user_id,
                "followUpStatus": incident.follow_up_status,
                "syncHitCount": incident.sync_hit_count,
                "escalationStatus": incident.escalation_status,
                "escalatedAt": incident.escalated_at,
                "escalationReason": incident.escalation_reason,
                "latestExportId": incident.latest_export_id,
                "previousExportId": incident.previous_export_id,
                "latestValue": incident.latest_value,
                "previousValue": incident.previous_value,
                "deltaValue": incident.delta_value,
                "deltaRatio": incident.delta_ratio,
                "thresholdValue": incident.threshold_value,
                "remediationRunId": impact.run.id,
                "windowMinutes": impact.window_minutes,
                "completionRateDelta": impact.metrics.completion_rate.delta_value,
                "failureRateDelta": impact.metrics.failure_rate.delta_value,
                "requestArtifactCoverageDelta": impact.metrics.request_artifact_coverage.delta_value,
                "responseArtifactCoverageDelta": impact.metrics.response_artifact_coverage.delta_value,
                "firstTokenLatencyMsAvgDelta": impact.metrics.first_token_latency_ms_avg.delta_value,
                "totalTokensPerSampleDelta": impact.metrics.total_tokens_per_sample.delta_value
            })),
            OffsetDateTime::now_utc(),
        )
        .await?;
    }

    Ok(GatewayAnalysisAnomalyRemediationImpactCaptureView {
        run: to_remediation_run_view(updated_row)?,
        impact,
    })
}

pub async fn get_anomaly_remediation_effectiveness(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSummaryView, GatewayError> {
    let window_minutes = normalize_window_minutes(window_minutes);
    let runs = list_anomaly_incident_remediation_runs(
        pool,
        &GatewayAnalysisAnomalyRemediationRunFilters {
            limit: Some(filters.limit.unwrap_or(100).clamp(1, 200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut impacts = Vec::with_capacity(runs.len());
    for run in &runs {
        if run.status != "applied" {
            impacts.push(None);
            continue;
        }
        let impact =
            get_anomaly_incident_remediation_run_impact(pool, &run.id, Some(window_minutes))
                .await
                .ok();
        impacts.push(impact);
    }

    Ok(build_remediation_effectiveness_summary(
        OffsetDateTime::now_utc(),
        window_minutes,
        &runs,
        &impacts,
    ))
}

pub async fn persist_anomaly_remediation_effectiveness_snapshot(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
    label: Option<&str>,
    window_minutes: Option<i32>,
    lookback_hours: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let window_minutes = normalize_window_minutes(window_minutes);
    let lookback_hours = lookback_hours.map(|value| value.clamp(0, 24 * 365));
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let created_from = if filters.created_from.is_some() {
        filters.created_from.clone()
    } else if let Some(hours) = lookback_hours {
        Some(format_timestamp(
            timestamp - time::Duration::hours(i64::from(hours)),
        ))
    } else {
        None
    };

    let normalized_filters = GatewayAnalysisAnomalyRemediationRunFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let summary =
        get_anomaly_remediation_effectiveness(pool, &normalized_filters, Some(window_minutes))
            .await?;

    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let object_key = build_remediation_effectiveness_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView {
        snapshot_id: snapshot_id.clone(),
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView {
            incident_id: normalized_filters.incident_id,
            policy_id: normalized_filters.policy_id,
            route_policy_id: normalized_filters.route_policy_id,
            action_key: normalized_filters.action_key,
            status: normalized_filters.status,
            execution_mode: normalized_filters.execution_mode,
            dry_run: normalized_filters.dry_run,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
            window_minutes,
        },
        summary,
    };

    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize remediation effectiveness snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_anomaly_remediation_effectiveness_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<Vec<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects("ai-gateway/remediation-effectiveness-snapshots")
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys {
        if !object_key.ends_with("/snapshot.json") {
            continue;
        }
        let snapshot = gateway_object_storage()?
            .read_json(&object_key)
            .await
            .ok()
            .and_then(|value| {
                serde_json::from_value::<
                        GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
                    >(value)
                    .ok()
            });
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_remediation_effectiveness_snapshot_filters(
            &snapshot,
            filters,
            created_from,
            created_to,
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn summarize_anomaly_remediation_effectiveness_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView, GatewayError>
{
    let snapshots = list_anomaly_remediation_effectiveness_snapshots(
        &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
            limit: Some(filters.limit.unwrap_or(500).clamp(1, 500)),
            ..filters.clone()
        },
    )
    .await?;
    Ok(build_remediation_effectiveness_snapshot_inventory_summary(
        &snapshots,
    ))
}

pub async fn get_anomaly_remediation_effectiveness_trend_report(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView, GatewayError> {
    let limit = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
        limit: Some(limit),
        ..filters.clone()
    };
    let snapshots = list_anomaly_remediation_effectiveness_snapshots(&normalized_filters).await?;
    let inventory_summary = summarize_anomaly_remediation_effectiveness_snapshots(
        &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
            limit: Some(500),
            ..normalized_filters.clone()
        },
    )
    .await?;
    let points = snapshots
        .into_iter()
        .map(build_remediation_effectiveness_trend_point)
        .collect::<Vec<_>>();

    Ok(
        GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView {
            generated_at: format_timestamp(OffsetDateTime::now_utc()),
            filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView {
                label: normalized_filters.label,
                route_policy_id: normalized_filters.route_policy_id,
                action_key: normalized_filters.action_key,
                created_from: normalized_filters.created_from,
                created_to: normalized_filters.created_to,
            },
            matched_snapshots_count: points.len(),
            window_size: limit,
            inventory_summary,
            summary: build_remediation_effectiveness_trend_summary(&points),
            points,
        },
    )
}

pub async fn get_anomaly_remediation_effectiveness_snapshot(
    snapshot_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(snapshot_id)
        .ok_or_else(|| GatewayError::bad_request("snapshotId 不能为空"))?;
    let object_key = build_remediation_effectiveness_snapshot_object_key(snapshot_id);
    let value = gateway_object_storage()?
        .read_json(&object_key)
        .await
        .map_err(|_| {
            GatewayError::not_found("Gateway remediation effectiveness snapshot 不存在")
        })?;
    serde_json::from_value(value)
        .map_err(|_| GatewayError::not_found("Gateway remediation effectiveness snapshot 不存在"))
}

pub async fn get_anomaly_remediation_effectiveness_snapshot_anomaly_report(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_profile_key(profile_key);
    let thresholds =
        build_remediation_effectiveness_threshold_config(&normalized_profile_key, overrides);
    let trend_report = get_anomaly_remediation_effectiveness_trend_report(filters).await?;
    Ok(build_remediation_effectiveness_anomaly_report(
        trend_report,
        normalized_profile_key,
        thresholds,
    ))
}

pub async fn persist_anomaly_remediation_effectiveness_anomaly_snapshot(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let lookback_hours = lookback_hours.map(|value| value.clamp(0, 24 * 365));
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
    let normalized_filters = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let report = get_anomaly_remediation_effectiveness_snapshot_anomaly_report(
        &normalized_filters,
        profile_key,
        overrides,
    )
    .await?;

    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let object_key = build_remediation_effectiveness_anomaly_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
        snapshot_id: snapshot_id.clone(),
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView {
            label: normalized_filters.label,
            route_policy_id: normalized_filters.route_policy_id,
            action_key: normalized_filters.action_key,
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
                    "serialize remediation effectiveness anomaly snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_anomaly_remediation_effectiveness_anomaly_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
) -> Result<Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects("ai-gateway/remediation-effectiveness-anomaly-snapshots")
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys {
        if !object_key.ends_with("/snapshot.json") {
            continue;
        }
        let snapshot = gateway_object_storage()?
            .read_json(&object_key)
            .await
            .ok()
            .and_then(|value| {
                serde_json::from_value::<
                    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
                >(value)
                .ok()
            });
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_remediation_effectiveness_anomaly_snapshot_filters(
            &snapshot,
            filters,
            created_from,
            created_to,
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn get_anomaly_remediation_effectiveness_anomaly_snapshot(
    snapshot_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(snapshot_id)
        .ok_or_else(|| GatewayError::bad_request("snapshotId 不能为空"))?;
    let object_key = build_remediation_effectiveness_anomaly_snapshot_object_key(snapshot_id);
    let value = gateway_object_storage()?
        .read_json(&object_key)
        .await
        .map_err(|_| {
            GatewayError::not_found("Gateway remediation effectiveness anomaly snapshot 不存在")
        })?;
    serde_json::from_value(value).map_err(|_| {
        GatewayError::not_found("Gateway remediation effectiveness anomaly snapshot 不存在")
    })
}

fn normalize_anomaly_policy_status(value: Option<&str>) -> String {
    if trimmed_owned_ref_opt(value).is_some_and(|item| item.eq_ignore_ascii_case("disabled")) {
        "disabled".to_string()
    } else {
        "enabled".to_string()
    }
}

fn normalize_anomaly_policy_sync_status(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "ok" | "error" => Some(normalized),
        _ => None,
    }
}

fn normalize_anomaly_profile_key(value: Option<&str>) -> String {
    let normalized = trimmed_owned_ref_opt(value)
        .unwrap_or("balanced")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "conservative" | "aggressive" | "balanced" => normalized,
        _ => "balanced".to_string(),
    }
}

fn resolve_anomaly_policy_schedule(
    status: &str,
    auto_sync_enabled: bool,
    auto_sync_interval_minutes: Option<i32>,
    last_synced_at: Option<&str>,
    now: OffsetDateTime,
) -> (Option<String>, bool) {
    if status != "enabled" || !auto_sync_enabled {
        return (None, false);
    }

    let interval_minutes = auto_sync_interval_minutes.unwrap_or(60).max(1);
    let Some(last_synced_at) = trimmed_owned_ref_opt(last_synced_at) else {
        return (None, true);
    };
    let Ok(reference_time) = OffsetDateTime::parse(last_synced_at, &Rfc3339) else {
        return (None, true);
    };

    let next_sync_due_at = reference_time + time::Duration::minutes(i64::from(interval_minutes));
    (
        Some(format_timestamp(next_sync_due_at)),
        next_sync_due_at <= now,
    )
}

fn normalize_anomaly_severity(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "warning" | "critical" => Some(normalized),
        _ => None,
    }
}

fn normalize_follow_up_status(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "pending" | "investigating" | "monitoring" | "done" => Some(normalized),
        _ => None,
    }
}

fn normalize_non_negative_int(
    value: Option<i32>,
    fallback: Option<i32>,
    max_value: i32,
    label: &str,
) -> Result<Option<i32>, GatewayError> {
    let Some(value) = value.or(fallback) else {
        return Ok(None);
    };
    if value < 0 {
        return Err(GatewayError::bad_request(format!("{label} 必须是非负整数")));
    }
    if value > max_value {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        )));
    }
    Ok(Some(value))
}

fn normalize_string_list(values: Option<&[String]>) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or(&[]) {
        let Some(value) = trimmed_owned_ref(value) else {
            continue;
        };
        let value = value.to_string();
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_threshold_override(overrides: Option<&Value>, key: &str) -> Option<f64> {
    overrides
        .and_then(Value::as_object)
        .and_then(|items| items.get(key))
        .and_then(Value::as_f64)
}

fn build_analysis_anomaly_threshold_config(profile_key: &str, overrides: Option<&Value>) -> Value {
    let base = match profile_key {
        "conservative" => json!({
            "failureRateWarningThreshold": 0.2,
            "failureRateCriticalThreshold": 0.3,
            "failureRateDeltaRatioThreshold": 0.8,
            "completionRateWarningThreshold": 0.7,
            "completionRateCriticalThreshold": 0.55,
            "completionRateDeltaValueThreshold": -0.15,
            "responseArtifactCoverageWarningThreshold": 0.75,
            "responseArtifactCoverageCriticalThreshold": 0.55,
            "responseArtifactCoverageDeltaValueThreshold": -0.15,
            "requestArtifactCoverageWarningThreshold": 0.8,
            "requestArtifactCoverageCriticalThreshold": 0.6,
            "requestArtifactCoverageDeltaValueThreshold": -0.15,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.5,
            "tokensPerSampleCriticalDeltaRatioThreshold": 1.0,
            "tokensPerSampleCriticalAbsoluteThreshold": 2500.0
        }),
        "aggressive" => json!({
            "failureRateWarningThreshold": 0.12,
            "failureRateCriticalThreshold": 0.2,
            "failureRateDeltaRatioThreshold": 0.35,
            "completionRateWarningThreshold": 0.8,
            "completionRateCriticalThreshold": 0.7,
            "completionRateDeltaValueThreshold": -0.08,
            "responseArtifactCoverageWarningThreshold": 0.85,
            "responseArtifactCoverageCriticalThreshold": 0.7,
            "responseArtifactCoverageDeltaValueThreshold": -0.08,
            "requestArtifactCoverageWarningThreshold": 0.9,
            "requestArtifactCoverageCriticalThreshold": 0.75,
            "requestArtifactCoverageDeltaValueThreshold": -0.08,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.25,
            "tokensPerSampleCriticalDeltaRatioThreshold": 0.6,
            "tokensPerSampleCriticalAbsoluteThreshold": 1800.0
        }),
        _ => json!({
            "failureRateWarningThreshold": 0.15,
            "failureRateCriticalThreshold": 0.25,
            "failureRateDeltaRatioThreshold": 0.5,
            "completionRateWarningThreshold": 0.75,
            "completionRateCriticalThreshold": 0.6,
            "completionRateDeltaValueThreshold": -0.1,
            "responseArtifactCoverageWarningThreshold": 0.8,
            "responseArtifactCoverageCriticalThreshold": 0.6,
            "responseArtifactCoverageDeltaValueThreshold": -0.1,
            "requestArtifactCoverageWarningThreshold": 0.85,
            "requestArtifactCoverageCriticalThreshold": 0.65,
            "requestArtifactCoverageDeltaValueThreshold": -0.1,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.35,
            "tokensPerSampleCriticalDeltaRatioThreshold": 0.8,
            "tokensPerSampleCriticalAbsoluteThreshold": 2000.0
        }),
    };

    let mut object = base.as_object().cloned().unwrap_or_default();
    for key in [
        "failureRateWarningThreshold",
        "failureRateCriticalThreshold",
        "failureRateDeltaRatioThreshold",
        "completionRateWarningThreshold",
        "completionRateCriticalThreshold",
        "completionRateDeltaValueThreshold",
        "responseArtifactCoverageWarningThreshold",
        "responseArtifactCoverageCriticalThreshold",
        "responseArtifactCoverageDeltaValueThreshold",
        "requestArtifactCoverageWarningThreshold",
        "requestArtifactCoverageCriticalThreshold",
        "requestArtifactCoverageDeltaValueThreshold",
        "tokensPerSampleWarningDeltaRatioThreshold",
        "tokensPerSampleCriticalDeltaRatioThreshold",
        "tokensPerSampleCriticalAbsoluteThreshold",
    ] {
        if let Some(value) = parse_threshold_override(overrides, key) {
            object.insert(key.to_string(), json!(value));
        }
    }
    Value::Object(object)
}

fn to_anomaly_policy_view(
    row: GatewayAnalysisAnomalyPolicyRow,
    now: OffsetDateTime,
) -> GatewayAnalysisAnomalyPolicyView {
    let status = normalize_anomaly_policy_status(Some(&row.status));
    let last_synced_at = row.last_synced_at.map(format_timestamp);
    let (next_sync_due_at, sync_due) = resolve_anomaly_policy_schedule(
        &status,
        row.auto_sync_enabled,
        row.auto_sync_interval_minutes,
        last_synced_at.as_deref(),
        now,
    );
    GatewayAnalysisAnomalyPolicyView {
        id: row.id,
        name: row.name,
        status,
        project_id: row.project_id,
        route_policy_id: row.route_policy_id,
        tag: row.tag,
        text_mode: row.text_mode,
        profile_key: normalize_anomaly_profile_key(Some(&row.profile_key)),
        thresholds: row.thresholds.0,
        auto_sync_enabled: row.auto_sync_enabled,
        auto_sync_interval_minutes: row.auto_sync_interval_minutes,
        last_synced_at,
        last_sync_status: normalize_anomaly_policy_sync_status(row.last_sync_status.as_deref()),
        last_sync_error: row.last_sync_error,
        next_sync_due_at,
        sync_due,
        auto_escalate_enabled: row.auto_escalate_enabled,
        escalate_severity_threshold: row.escalate_severity_threshold,
        escalate_after_sync_count: row.escalate_after_sync_count,
        auto_escalate_owner_user_id: row.auto_escalate_owner_user_id,
        auto_escalate_follow_up_status: row.auto_escalate_follow_up_status,
        auto_remediation_enabled: row.auto_remediation_enabled,
        auto_remediation_interval_minutes: row.auto_remediation_interval_minutes,
        auto_remediation_dry_run_first: row.auto_remediation_dry_run_first,
        auto_remediation_action_keys: row.auto_remediation_action_keys.map(|value| value.0),
        auto_remediation_max_apply_runs_per_incident: row
            .auto_remediation_max_apply_runs_per_incident,
        auto_remediation_require_alert_before_apply: row
            .auto_remediation_require_alert_before_apply,
        auto_remediation_freeze_on_provider_health_degrade: row
            .auto_remediation_freeze_on_provider_health_degrade,
        alerting_enabled: row.alerting_enabled,
        alert_interval_minutes: row.alert_interval_minutes,
        notify_operators_on_escalation: row.notify_operators_on_escalation,
        notify_owner_on_escalation: row.notify_owner_on_escalation,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub async fn list_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<Vec<GatewayAnalysisAnomalyPolicyView>, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let due_only = filters.due_only == Some(true);
    let raw_limit = if due_only { limit.max(500) } else { limit };

    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        select
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          last_synced_at,
          last_sync_status,
          last_sync_error,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        from gateway_analysis_anomaly_policies
        where 1 = 1
        "#,
    );
    push_optional_filter(&mut builder, "id", filters.policy_id.as_deref());
    push_optional_filter(&mut builder, "project_id", filters.project_id.as_deref());
    push_optional_filter(
        &mut builder,
        "route_policy_id",
        filters.route_policy_id.as_deref(),
    );
    push_optional_filter(&mut builder, "status", filters.status.as_deref());
    if let Some(tag) = trimmed_owned_ref_opt(filters.tag.as_deref()) {
        builder
            .push(" and tag = ")
            .push_bind(tag.to_ascii_lowercase());
    }
    push_optional_filter(&mut builder, "text_mode", filters.text_mode.as_deref());
    if let Some(value) = filters.auto_sync_enabled {
        builder.push(" and auto_sync_enabled = ").push_bind(value);
    }
    if let Some(value) = filters.auto_escalate_enabled {
        builder
            .push(" and auto_escalate_enabled = ")
            .push_bind(value);
    }
    if let Some(value) = filters.auto_remediation_enabled {
        builder
            .push(" and auto_remediation_enabled = ")
            .push_bind(value);
    }
    if let Some(value) = filters.alerting_enabled {
        builder.push(" and alerting_enabled = ").push_bind(value);
    }
    builder
        .push(" order by updated_at desc limit ")
        .push_bind(i64::try_from(raw_limit).unwrap_or(500));

    let rows = builder
        .build_query_as::<GatewayAnalysisAnomalyPolicyRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;
    let now = OffsetDateTime::now_utc();
    let mut policies = rows
        .into_iter()
        .map(|row| to_anomaly_policy_view(row, now))
        .collect::<Vec<_>>();
    if due_only {
        policies.retain(|item| item.sync_due);
    }
    policies.truncate(limit);
    Ok(policies)
}

pub async fn summarize_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<GatewayAnalysisAnomalyPolicySummaryView, GatewayError> {
    let policies = list_anomaly_policies(
        pool,
        &GatewayAnalysisAnomalyPolicyFilters {
            limit: Some(filters.limit.unwrap_or(200).max(200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut by_status = BTreeMap::new();
    let mut by_sync_status = BTreeMap::new();
    let mut enabled_policies = 0usize;
    let mut disabled_policies = 0usize;
    let mut auto_sync_enabled_policies = 0usize;
    let mut auto_escalate_enabled_policies = 0usize;
    let mut auto_remediation_enabled_policies = 0usize;
    let mut alerting_enabled_policies = 0usize;
    let mut due_policies = 0usize;

    for policy in &policies {
        accumulate_key_bucket(&mut by_status, Some(&policy.status));
        accumulate_key_bucket(&mut by_sync_status, policy.last_sync_status.as_deref());
        if policy.status == "enabled" {
            enabled_policies += 1;
        } else if policy.status == "disabled" {
            disabled_policies += 1;
        }
        if policy.auto_sync_enabled {
            auto_sync_enabled_policies += 1;
        }
        if policy.auto_escalate_enabled {
            auto_escalate_enabled_policies += 1;
        }
        if policy.auto_remediation_enabled {
            auto_remediation_enabled_policies += 1;
        }
        if policy.alerting_enabled {
            alerting_enabled_policies += 1;
        }
        if policy.sync_due {
            due_policies += 1;
        }
    }

    Ok(GatewayAnalysisAnomalyPolicySummaryView {
        total_policies: policies.len(),
        enabled_policies,
        disabled_policies,
        auto_sync_enabled_policies,
        auto_escalate_enabled_policies,
        auto_remediation_enabled_policies,
        alerting_enabled_policies,
        due_policies,
        by_status: into_key_buckets(by_status),
        by_sync_status: into_key_buckets(by_sync_status),
    })
}

pub async fn save_anomaly_policy(
    pool: &PgPool,
    input: UpsertGatewayAnalysisAnomalyPolicyInput,
) -> Result<GatewayAnalysisAnomalyPolicyView, GatewayError> {
    let policy_id = input
        .id
        .and_then(|value| trimmed_owned(Some(&value)))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let name = trimmed_owned_ref(&input.name)
        .ok_or_else(|| GatewayError::bad_request("Policy 名称不能为空"))?;
    if name.chars().count() > 120 {
        return Err(GatewayError::bad_request("Policy 名称不能超过 120 个字符"));
    }

    let status = normalize_anomaly_policy_status(input.status.as_deref());
    let profile_key = normalize_anomaly_profile_key(input.profile_key.as_deref());
    let thresholds =
        build_analysis_anomaly_threshold_config(&profile_key, input.thresholds.as_ref());
    let tag = trimmed_owned(input.tag.as_deref()).map(|value| value.to_ascii_lowercase());
    if tag.as_ref().is_some_and(|value| value.chars().count() > 40) {
        return Err(GatewayError::bad_request("tag 不能超过 40 个字符"));
    }
    let route_policy =
        if let Some(route_policy_id) = trimmed_owned(input.route_policy_id.as_deref()) {
            find_route_policy_by_id(pool, &route_policy_id)
                .await?
                .ok_or_else(|| GatewayError::not_found("绑定的 route policy 不存在"))?
                .into()
        } else {
            None
        };
    let requested_project_id = trimmed_owned(input.project_id.as_deref());
    if let (Some(project_id), Some(route_policy)) =
        (requested_project_id.as_ref(), route_policy.as_ref())
    {
        if route_policy.project_id != *project_id {
            return Err(GatewayError::conflict(
                "anomaly policy 的 projectId 必须与 route policy 所属 project 一致。",
            ));
        }
    }
    let project_id =
        requested_project_id.or_else(|| route_policy.as_ref().map(|item| item.project_id.clone()));

    let auto_sync_enabled = input.auto_sync_enabled.unwrap_or(false);
    let auto_sync_interval_minutes = if auto_sync_enabled {
        normalize_non_negative_int(
            input.auto_sync_interval_minutes,
            Some(60),
            10_080,
            "autoSyncIntervalMinutes",
        )?
    } else {
        None
    };
    let auto_escalate_enabled = input.auto_escalate_enabled.unwrap_or(false);
    let escalate_severity_threshold = if auto_escalate_enabled {
        normalize_anomaly_severity(
            input
                .escalate_severity_threshold
                .as_deref()
                .or(Some("critical")),
        )
    } else {
        None
    };
    let escalate_after_sync_count = if auto_escalate_enabled {
        normalize_non_negative_int(
            input.escalate_after_sync_count,
            Some(3),
            1_000,
            "escalateAfterSyncCount",
        )?
    } else {
        None
    };
    let auto_escalate_owner_user_id = if auto_escalate_enabled {
        trimmed_owned(input.auto_escalate_owner_user_id.as_deref())
    } else {
        None
    };
    let auto_escalate_follow_up_status = if auto_escalate_enabled {
        normalize_follow_up_status(
            input
                .auto_escalate_follow_up_status
                .as_deref()
                .or(Some("investigating")),
        )
    } else {
        None
    };

    let auto_remediation_enabled = input.auto_remediation_enabled.unwrap_or(false);
    let auto_remediation_interval_minutes = if auto_remediation_enabled {
        normalize_non_negative_int(
            input.auto_remediation_interval_minutes,
            Some(180),
            10_080,
            "autoRemediationIntervalMinutes",
        )?
    } else {
        None
    };
    let auto_remediation_dry_run_first = if auto_remediation_enabled {
        input.auto_remediation_dry_run_first.unwrap_or(true)
    } else {
        true
    };
    let auto_remediation_action_keys = if auto_remediation_enabled {
        normalize_string_list(input.auto_remediation_action_keys.as_deref())
    } else {
        None
    };
    let auto_remediation_max_apply_runs_per_incident = if auto_remediation_enabled {
        normalize_non_negative_int(
            input.auto_remediation_max_apply_runs_per_incident,
            None,
            1_000,
            "autoRemediationMaxApplyRunsPerIncident",
        )?
    } else {
        None
    };
    let auto_remediation_require_alert_before_apply = if auto_remediation_enabled {
        input
            .auto_remediation_require_alert_before_apply
            .unwrap_or(false)
    } else {
        false
    };
    let auto_remediation_freeze_on_provider_health_degrade = if auto_remediation_enabled {
        input
            .auto_remediation_freeze_on_provider_health_degrade
            .unwrap_or(true)
    } else {
        true
    };

    let alerting_enabled = input.alerting_enabled.unwrap_or(true);
    let alert_interval_minutes = if alerting_enabled {
        normalize_non_negative_int(
            input.alert_interval_minutes,
            Some(DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES),
            10_080,
            "alertIntervalMinutes",
        )?
    } else {
        None
    };
    let notify_operators_on_escalation = if alerting_enabled {
        input.notify_operators_on_escalation.unwrap_or(true)
    } else {
        false
    };
    let notify_owner_on_escalation = if alerting_enabled {
        input.notify_owner_on_escalation.unwrap_or(true)
    } else {
        false
    };

    let timestamp = OffsetDateTime::now_utc();
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyPolicyRow>(
        r#"
        insert into gateway_analysis_anomaly_policies (
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        ) values (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$28
        )
        on conflict (id) do update set
          name = excluded.name,
          status = excluded.status,
          project_id = excluded.project_id,
          route_policy_id = excluded.route_policy_id,
          tag = excluded.tag,
          text_mode = excluded.text_mode,
          profile_key = excluded.profile_key,
          thresholds = excluded.thresholds,
          auto_sync_enabled = excluded.auto_sync_enabled,
          auto_sync_interval_minutes = excluded.auto_sync_interval_minutes,
          auto_escalate_enabled = excluded.auto_escalate_enabled,
          escalate_severity_threshold = excluded.escalate_severity_threshold,
          escalate_after_sync_count = excluded.escalate_after_sync_count,
          auto_escalate_owner_user_id = excluded.auto_escalate_owner_user_id,
          auto_escalate_follow_up_status = excluded.auto_escalate_follow_up_status,
          auto_remediation_enabled = excluded.auto_remediation_enabled,
          auto_remediation_interval_minutes = excluded.auto_remediation_interval_minutes,
          auto_remediation_dry_run_first = excluded.auto_remediation_dry_run_first,
          auto_remediation_action_keys = excluded.auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident = excluded.auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply = excluded.auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade = excluded.auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled = excluded.alerting_enabled,
          alert_interval_minutes = excluded.alert_interval_minutes,
          notify_operators_on_escalation = excluded.notify_operators_on_escalation,
          notify_owner_on_escalation = excluded.notify_owner_on_escalation,
          updated_at = excluded.updated_at
        returning
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          last_synced_at,
          last_sync_status,
          last_sync_error,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        "#,
    )
    .bind(policy_id)
    .bind(name)
    .bind(status)
    .bind(project_id)
    .bind(route_policy.as_ref().map(|item| item.id.as_str()))
    .bind(tag)
    .bind(trimmed_owned(input.text_mode.as_deref()))
    .bind(profile_key)
    .bind(sqlx::types::Json(thresholds))
    .bind(auto_sync_enabled)
    .bind(auto_sync_interval_minutes)
    .bind(auto_escalate_enabled)
    .bind(escalate_severity_threshold)
    .bind(escalate_after_sync_count)
    .bind(auto_escalate_owner_user_id)
    .bind(auto_escalate_follow_up_status)
    .bind(auto_remediation_enabled)
    .bind(auto_remediation_interval_minutes)
    .bind(auto_remediation_dry_run_first)
    .bind(auto_remediation_action_keys.map(sqlx::types::Json))
    .bind(auto_remediation_max_apply_runs_per_incident)
    .bind(auto_remediation_require_alert_before_apply)
    .bind(auto_remediation_freeze_on_provider_health_degrade)
    .bind(alerting_enabled)
    .bind(alert_interval_minutes)
    .bind(notify_operators_on_escalation)
    .bind(notify_owner_on_escalation)
    .bind(timestamp)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_anomaly_policy_view(row, timestamp))
}

async fn find_anomaly_policy_by_id(
    pool: &PgPool,
    policy_id: &str,
) -> Result<Option<GatewayAnalysisAnomalyPolicyView>, GatewayError> {
    let Some(policy_id) = trimmed_owned_ref(policy_id) else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyPolicyRow>(
        r#"
        select
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          last_synced_at,
          last_sync_status,
          last_sync_error,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        from gateway_analysis_anomaly_policies
        where id = $1
        limit 1
        "#,
    )
    .bind(policy_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(|value| to_anomaly_policy_view(value, OffsetDateTime::now_utc())))
}

pub async fn sync_anomaly_policy(
    pool: &PgPool,
    policy_id: &str,
) -> Result<GatewayAnalysisAnomalyPolicySyncView, GatewayError> {
    let policy_id = trimmed_owned_ref(policy_id)
        .ok_or_else(|| GatewayError::bad_request("policyId 不能为空"))?;
    let policy = find_anomaly_policy_by_id(pool, policy_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly policy 不存在"))?;
    let timestamp = OffsetDateTime::now_utc();

    let sync_result = match determine_supported_policy_sync_kind(&policy) {
        SupportedPolicySyncKind::ProviderRouting => {
            let result = sync_provider_routing_anomaly_incidents(
                pool,
                &build_policy_request_audit_filters(&policy),
                Some(policy.profile_key.as_str()),
                GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::ProviderRouting(result)
        }
        SupportedPolicySyncKind::RateLimitHotspot => {
            let filters = build_policy_request_audit_filters(&policy);
            let snapshot = persist_rate_limit_hotspot_anomaly_snapshot(
                pool,
                &filters,
                Some(policy.name.as_str()),
                Some(24),
                Some(policy.profile_key.as_str()),
                GatewayRateLimitHotspotAnomalyOverrides::default(),
            )
            .await?;
            let result =
                sync_rate_limit_hotspot_anomaly_incidents(pool, &snapshot.snapshot_id).await?;
            PolicySyncResult::RateLimitHotspot(result)
        }
        SupportedPolicySyncKind::AnalysisExport => {
            let export_filters = build_policy_analysis_export_filters(&policy);
            let report = get_analysis_export_anomaly_report(
                pool,
                &export_filters,
                Some(policy.id.as_str()),
                Some(policy.profile_key.as_str()),
                GatewayAnalysisExportAnomalyOverrides::default(),
            )
            .await?;
            let result = sync_analysis_export_anomaly_incidents(
                pool,
                report,
                GatewaySyncAnalysisExportAnomalyIncidentsInput {
                    policy_id: Some(policy.id.clone()),
                    project_id: export_filters.project_id.clone(),
                    route_policy_id: policy.route_policy_id.clone(),
                    tag: export_filters.tag.clone(),
                    text_mode: export_filters.text_mode.clone(),
                    auto_escalation: build_analysis_export_auto_escalation_config(Some(&policy)),
                },
            )
            .await?;
            PolicySyncResult::AnalysisExport(result)
        }
        SupportedPolicySyncKind::Unsupported(reason) => {
            update_anomaly_policy_sync_state(
                pool,
                policy_id,
                "error",
                timestamp,
                Some(reason.as_str()),
            )
            .await?;
            return Err(GatewayError::conflict(reason));
        }
    };

    update_anomaly_policy_sync_state(pool, policy_id, "ok", timestamp, None).await?;
    let refreshed_policy = find_anomaly_policy_by_id(pool, policy_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly policy 同步后不存在"))?;

    Ok(GatewayAnalysisAnomalyPolicySyncView {
        policy: refreshed_policy,
        sync_kind: sync_result.kind().to_string(),
        anomaly_count: sync_result.anomaly_count(),
        opened_incident_count: sync_result.opened_incident_count(),
        updated_incident_count: sync_result.updated_incident_count(),
        resolved_incident_count: sync_result.resolved_incident_count(),
        sync: sync_result.into_value()?,
    })
}

pub async fn sync_anomaly_incidents(
    pool: &PgPool,
    input: GatewayAnalysisAnomalyIncidentSyncInput,
) -> Result<GatewayAnalysisAnomalyIncidentSyncView, GatewayError> {
    let policy = if let Some(policy_id) = trimmed_owned_ref_opt(input.policy_id.as_deref()) {
        find_anomaly_policy_by_id(pool, policy_id).await?
    } else {
        None
    };
    let effective_tag = trimmed_owned_ref_opt(input.tag.as_deref())
        .map(str::to_string)
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|item| trimmed_owned(item.tag.as_deref()))
        });
    let sync_kind = determine_supported_ad_hoc_sync_kind(effective_tag.as_deref(), &input);
    let filters =
        build_ad_hoc_request_audit_filters(policy.as_ref(), &input, effective_tag.as_deref());
    let effective_profile_key = trimmed_owned_ref_opt(input.profile_key.as_deref())
        .map(str::to_string)
        .or_else(|| {
            effective_tag
                .as_deref()
                .and_then(|tag| split_policy_tag_parts(tag).0)
        })
        .or_else(|| policy.as_ref().map(|item| item.profile_key.clone()));

    let sync_result = match sync_kind {
        SupportedPolicySyncKind::ProviderRouting => PolicySyncResult::ProviderRouting(
            sync_provider_routing_anomaly_incidents(
                pool,
                &filters,
                effective_profile_key.as_deref(),
                GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
            )
            .await?,
        ),
        SupportedPolicySyncKind::RateLimitHotspot => {
            let snapshot = persist_rate_limit_hotspot_anomaly_snapshot(
                pool,
                &filters,
                trimmed_owned_ref_opt(input.label.as_deref()),
                None,
                effective_profile_key.as_deref(),
                GatewayRateLimitHotspotAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::RateLimitHotspot(
                sync_rate_limit_hotspot_anomaly_incidents(pool, &snapshot.snapshot_id).await?,
            )
        }
        SupportedPolicySyncKind::AnalysisExport => {
            let export_filters = build_ad_hoc_analysis_export_filters(
                policy.as_ref(),
                &input,
                effective_tag.as_deref(),
            );
            let report = get_analysis_export_anomaly_report(
                pool,
                &export_filters,
                policy.as_ref().map(|item| item.id.as_str()),
                effective_profile_key.as_deref(),
                GatewayAnalysisExportAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::AnalysisExport(
                sync_analysis_export_anomaly_incidents(
                    pool,
                    report,
                    GatewaySyncAnalysisExportAnomalyIncidentsInput {
                        policy_id: policy.as_ref().map(|item| item.id.clone()),
                        project_id: export_filters.project_id.clone(),
                        route_policy_id: input.route_policy_id.clone().or_else(|| {
                            policy
                                .as_ref()
                                .and_then(|item| item.route_policy_id.clone())
                        }),
                        tag: export_filters.tag.clone(),
                        text_mode: export_filters.text_mode.clone(),
                        auto_escalation: build_analysis_export_auto_escalation_config(
                            policy.as_ref(),
                        ),
                    },
                )
                .await?,
            )
        }
        SupportedPolicySyncKind::Unsupported(reason) => {
            return Err(GatewayError::conflict(reason));
        }
    };

    Ok(GatewayAnalysisAnomalyIncidentSyncView {
        policy,
        sync_kind: sync_result.kind().to_string(),
        anomaly_count: sync_result.anomaly_count(),
        opened_incident_count: sync_result.opened_incident_count(),
        updated_incident_count: sync_result.updated_incident_count(),
        resolved_incident_count: sync_result.resolved_incident_count(),
        sync: sync_result.into_value()?,
    })
}

pub async fn sweep_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<GatewayAnalysisAnomalyPolicySweepView, GatewayError> {
    let started_at = OffsetDateTime::now_utc();
    let limit = filters.limit.unwrap_or(20).clamp(1, 100);
    let candidates = list_anomaly_policies(
        pool,
        &GatewayAnalysisAnomalyPolicyFilters {
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await?;
    let mut items = Vec::with_capacity(candidates.len());
    let mut ok_count = 0usize;
    let mut error_count = 0usize;
    let mut skipped_count = 0usize;

    for policy in candidates {
        if policy.status != "enabled" || !policy.auto_sync_enabled || !policy.sync_due {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                policy_id: policy.id,
                policy_name: policy.name,
                status: "skipped".to_string(),
                error: None,
                last_synced_at: policy.last_synced_at,
                next_sync_due_at: policy.next_sync_due_at,
                sync_due: policy.sync_due,
                anomaly_count: 0,
                opened_incident_count: 0,
                updated_incident_count: 0,
                resolved_incident_count: 0,
            });
            continue;
        }

        match sync_anomaly_policy(pool, &policy.id).await {
            Ok(result) => {
                ok_count += 1;
                items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                    policy_id: result.policy.id,
                    policy_name: result.policy.name,
                    status: "ok".to_string(),
                    error: None,
                    last_synced_at: result.policy.last_synced_at,
                    next_sync_due_at: result.policy.next_sync_due_at,
                    sync_due: result.policy.sync_due,
                    anomaly_count: result.anomaly_count,
                    opened_incident_count: result.opened_incident_count,
                    updated_incident_count: result.updated_incident_count,
                    resolved_incident_count: result.resolved_incident_count,
                });
            }
            Err(error) => {
                error_count += 1;
                let refreshed_policy = find_anomaly_policy_by_id(pool, &policy.id).await?;
                let (last_synced_at, next_sync_due_at, sync_due) =
                    if let Some(refreshed) = refreshed_policy {
                        (
                            refreshed.last_synced_at,
                            refreshed.next_sync_due_at,
                            refreshed.sync_due,
                        )
                    } else {
                        (
                            policy.last_synced_at,
                            policy.next_sync_due_at,
                            policy.sync_due,
                        )
                    };
                items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                    policy_id: policy.id,
                    policy_name: policy.name,
                    status: "error".to_string(),
                    error: Some(truncate_error_summary(&error.to_string(), 240)),
                    last_synced_at,
                    next_sync_due_at,
                    sync_due,
                    anomaly_count: 0,
                    opened_incident_count: 0,
                    updated_incident_count: 0,
                    resolved_incident_count: 0,
                });
            }
        }
    }

    Ok(GatewayAnalysisAnomalyPolicySweepView {
        started_at: format_timestamp(started_at),
        completed_at: format_timestamp(OffsetDateTime::now_utc()),
        limit,
        attempted_count: items.len(),
        ok_count,
        error_count,
        skipped_count,
        items,
    })
}

async fn load_incident_latest_sync_context(
    pool: &PgPool,
    incident_id: &str,
) -> Result<IncidentSyncContext, GatewayError> {
    let incident_id = trimmed_owned_ref(incident_id)
        .ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))?;
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyIncidentHistoryLookupRow>(
        r#"
        select metadata
        from gateway_analysis_anomaly_incident_history
        where incident_id = $1
          and event_type = any($2)
        order by created_at desc
        limit 1
        "#,
    )
    .bind(incident_id)
    .bind(vec!["sync_opened".to_string(), "sync_updated".to_string()])
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let Some(metadata) = row.and_then(|value| value.metadata.map(|item| item.0)) else {
        return Ok(IncidentSyncContext::default());
    };
    let object = metadata.as_object();
    Ok(IncidentSyncContext {
        entity_key: object
            .and_then(|item| item.get("entityKey"))
            .and_then(Value::as_str)
            .and_then(|item| trimmed_owned(Some(item))),
        snapshot_id: object
            .and_then(|item| item.get("snapshotId"))
            .and_then(Value::as_str)
            .and_then(|item| trimmed_owned(Some(item))),
    })
}

fn build_incident_remediation_plan(
    generated_at: OffsetDateTime,
    incident: GatewayAnalysisAnomalyIncidentView,
    policy: Option<GatewayAnalysisAnomalyPolicyView>,
    route_policy: Option<GatewayRoutePolicyView>,
    incident_context: IncidentSyncContext,
) -> GatewayAnalysisAnomalyIncidentRemediationPlanView {
    let route_policy_id = route_policy
        .as_ref()
        .map(|item| item.id.clone())
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|item| item.route_policy_id.clone())
        })
        .or_else(|| incident.route_policy_id.clone());
    let mut actions = Vec::new();

    if matches!(
        incident.code.as_str(),
        "provider_routing_score_drop"
            | "degraded_provider_route_spike"
            | "saturated_provider_route_spike"
            | "breaker_open_provider_route_detected"
    ) {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "provider-routing-review".to_string(),
                title: "Review Provider Routing Degradation".to_string(),
                description: "Inspect provider routing score, degradation reasons, saturation, and breaker-open drift for the linked route policy before traffic keeps concentrating on unhealthy providers.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(if let Some(route_policy) = route_policy.as_ref() {
                    json!({
                        "routePolicyId": route_policy.id,
                        "inspect": [
                            "allowedProviderAccountIds",
                            "providerMaxConcurrentRequests",
                            "providerLoadAwareRoutingEnabled",
                            "circuitBreakerThreshold",
                            "circuitBreakerCooldownSeconds"
                        ]
                    })
                } else {
                    json!({
                        "inspect": ["route_policy", "provider_routing_score", "degradation_reasons"]
                    })
                }),
            },
        );

        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "disable-prestream-fallback".to_string(),
                title: "Disable Pre-stream Fallback".to_string(),
                description: "Stop broad pre-stream fallback while unhealthy providers are being isolated, so routing no longer fan-outs across already degraded candidates.".to_string(),
                category: "routing".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy_id.as_ref().map(|_| {
                    json!({
                        "routePolicyPatch": {
                            "preStreamFallbackEnabled": false
                        }
                    })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "patch": {
                            "preStreamFallbackEnabled": false
                        }
                    })
                }),
            },
        );

        let provider_max_concurrent_requests = route_policy
            .as_ref()
            .and_then(|item| item.config.provider_max_concurrent_requests)
            .map(|value| value.saturating_sub(1).max(1))
            .unwrap_or(1);
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "reduce-provider-concurrency".to_string(),
                title: "Reduce Provider Concurrency Cap".to_string(),
                description: "Lower the per-provider concurrency cap so unhealthy providers stop receiving the same level of parallel pressure while routing stabilizes.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy_id.as_ref().map(|_| {
                    json!({
                        "routePolicyPatch": {
                            "providerMaxConcurrentRequests": provider_max_concurrent_requests
                        }
                    })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "patch": {
                            "providerMaxConcurrentRequests": provider_max_concurrent_requests
                        }
                    })
                }),
            },
        );

        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "provider-isolation".to_string(),
                title: "Isolate Degraded Providers".to_string(),
                description: "Temporarily narrow the provider allowlist so degraded, saturated, or breaker-open providers are removed from the active route pool.".to_string(),
                category: "provider".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: route_policy_id.is_some(),
                execution_mode: if route_policy_id.is_some() {
                    "route_policy_patch".to_string()
                } else {
                    "informational".to_string()
                },
                default_execution_input: route_policy.as_ref().and_then(|route_policy| {
                    route_policy
                        .config
                        .allowed_provider_account_ids
                        .as_ref()
                        .and_then(|items| items.first().cloned())
                        .map(|provider_account_id| {
                            json!({
                                "routePolicyPatch": {
                                    "allowedProviderAccountIds": [provider_account_id]
                                }
                            })
                        })
                }),
                recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                    json!({
                        "routePolicyId": route_policy_id,
                        "mode": "narrow_provider_pool"
                    })
                }),
            },
        );
    }

    if matches!(
        incident.code.as_str(),
        "rate_limit_request_spike"
            | "rate_limit_code_concentration"
            | "rate_limit_project_hotspot"
            | "rate_limit_api_key_hotspot"
            | "rate_limit_model_hotspot"
            | "rate_limit_endpoint_hotspot"
    ) {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "rate-limit-hotspot-review".to_string(),
                title: "Review Rate-limit Hotspot".to_string(),
                description: "Inspect hotspot entity key, preflight rate limits, requested model, api key, and endpoint distribution before auto-remediation keeps tightening the same route policy.".to_string(),
                category: "routing".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(json!({
                    "focusCode": incident.code,
                    "focusValue": incident.latest_value,
                    "inspect": ["hotspot_entity_key", "preflight_rate_limits", "requested_model", "api_key_id", "endpoint_kind"]
                })),
            },
        );

        if route_policy_id.is_some()
            && matches!(
                incident.code.as_str(),
                "rate_limit_request_spike"
                    | "rate_limit_code_concentration"
                    | "rate_limit_project_hotspot"
            )
        {
            let current_project_rate_limit = route_policy.as_ref().and_then(|item| {
                match (
                    item.config.rate_limit_window_seconds,
                    item.config.rate_limit_max_requests,
                ) {
                    (Some(window_seconds), Some(max_requests)) => {
                        Some(super::GatewayRateLimitDefinition {
                            window_seconds,
                            max_requests,
                        })
                    }
                    _ => None,
                }
            });
            let target_project_rate_limit = build_tighter_rate_limit(
                current_project_rate_limit.as_ref(),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 30,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-project-rate-limit".to_string(),
                    title: "Tighten Project Rate Limit".to_string(),
                    description: "Apply a stricter route-level request cap so project-wide hotspot pressure stops spilling into the same route policy.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "projectRateLimit": target_project_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "projectRateLimit": target_project_rate_limit
                            }
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some() && incident.code == "rate_limit_api_key_hotspot" {
            let target_api_key_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.api_key_rate_limit.as_ref()),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-api-key-rate-limit".to_string(),
                    title: "Tighten Per-key Rate Limit".to_string(),
                    description: "Lower the route policy's per-key rate limit so one hot key stops dominating the request budget.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "apiKeyRateLimit": target_api_key_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "apiKeyRateLimit": target_api_key_rate_limit
                            }
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some()
            && incident.code == "rate_limit_model_hotspot"
            && incident_context.entity_key.is_some()
        {
            let entity_key = incident_context.entity_key.clone().unwrap_or_default();
            let target_model_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.model_rate_limits.as_ref())
                    .and_then(|limits| limits.get(&entity_key)),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-model-rate-limit".to_string(),
                    title: "Tighten Hot Model Rate Limit".to_string(),
                    description: "Apply a stricter model-specific limit for the dominant requested model so hotspot traffic does not keep concentrating on the same model.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "modelRateLimitKey": entity_key,
                            "modelRateLimit": target_model_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "modelRateLimitKey": entity_key,
                                "modelRateLimit": target_model_rate_limit
                            },
                            "snapshotId": incident_context.snapshot_id
                        })
                    }),
                },
            );
        }

        if route_policy_id.is_some()
            && incident.code == "rate_limit_endpoint_hotspot"
            && incident_context.entity_key.is_some()
        {
            let entity_key = incident_context.entity_key.clone().unwrap_or_default();
            let target_endpoint_rate_limit = build_tighter_rate_limit(
                route_policy
                    .as_ref()
                    .and_then(|item| item.config.endpoint_rate_limits.as_ref())
                    .and_then(|limits| limits.get(&entity_key.to_lowercase())),
                &super::GatewayRateLimitDefinition {
                    window_seconds: 60,
                    max_requests: 20,
                },
            );
            push_action(
                &mut actions,
                GatewayAnalysisAnomalyIncidentRemediationActionView {
                    action_key: "tighten-endpoint-rate-limit".to_string(),
                    title: "Tighten Hot Endpoint Rate Limit".to_string(),
                    description: "Apply a stricter endpoint-specific rate limit for the dominant public endpoint so hotspot traffic stops collapsing onto the same call surface.".to_string(),
                    category: "routing".to_string(),
                    priority: "high".to_string(),
                    route_policy_id: route_policy_id.clone(),
                    executable: true,
                    execution_mode: "route_policy_patch".to_string(),
                    default_execution_input: Some(json!({
                        "routePolicyPatch": {
                            "endpointRateLimitKey": entity_key,
                            "endpointRateLimit": target_endpoint_rate_limit
                        }
                    })),
                    recommended_changes: route_policy_id.as_ref().map(|route_policy_id| {
                        json!({
                            "routePolicyId": route_policy_id,
                            "patch": {
                                "endpointRateLimitKey": entity_key,
                                "endpointRateLimit": target_endpoint_rate_limit
                            },
                            "snapshotId": incident_context.snapshot_id
                        })
                    }),
                },
            );
        }
    }

    if incident.escalation_status == "escalated" {
        let incident_owner_user_id = incident.owner_user_id.clone();
        let incident_follow_up_status = incident.follow_up_status.clone();
        let incident_latest_note = incident.latest_note.clone();
        let incident_resolution_note = incident.resolution_note.clone();
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "owner-followup".to_string(),
                title: "Drive Owner Follow-up".to_string(),
                description: "This incident is escalated. Confirm the assigned owner, follow-up status, and remediation note so it does not stay as an unowned escalated signal.".to_string(),
                category: "manual".to_string(),
                priority: "high".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: true,
                execution_mode: "incident_follow_up".to_string(),
                default_execution_input: Some(json!({
                    "incidentFollowUp": {
                        "ownerUserId": incident_owner_user_id,
                        "followUpStatus": if incident_follow_up_status == "pending" {
                            "investigating"
                        } else {
                            incident_follow_up_status.as_str()
                        },
                        "note": incident_latest_note,
                        "resolutionNote": incident_resolution_note
                    }
                })),
                recommended_changes: Some(json!({
                    "ownerUserId": incident.owner_user_id.clone(),
                    "followUpStatus": incident.follow_up_status.clone()
                })),
            },
        );
    }

    if route_policy_id.is_none() {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "bind-route-policy".to_string(),
                title: "Bind a Route Policy to the Anomaly Policy".to_string(),
                description: "Link this anomaly policy to a concrete route policy so future anomalies can map directly to routing remediation rather than staying detached.".to_string(),
                category: "routing".to_string(),
                priority: "medium".to_string(),
                route_policy_id: None,
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: Some(json!({ "field": "routePolicyId" })),
            },
        );
    }

    if actions.is_empty() {
        push_action(
            &mut actions,
            GatewayAnalysisAnomalyIncidentRemediationActionView {
                action_key: "manual-triage".to_string(),
                title: "Perform Manual Triage".to_string(),
                description: "No specialized remediation rule matched. Review the incident artifacts, trend deltas, and route trace before deciding whether to tune routing, providers, or prompt behavior.".to_string(),
                category: "manual".to_string(),
                priority: "medium".to_string(),
                route_policy_id: route_policy_id.clone(),
                executable: false,
                execution_mode: "informational".to_string(),
                default_execution_input: None,
                recommended_changes: None,
            },
        );
    }

    GatewayAnalysisAnomalyIncidentRemediationPlanView {
        generated_at: format_timestamp(generated_at),
        incident,
        policy: policy
            .as_ref()
            .and_then(|value| serde_json::to_value(value).ok()),
        route_policy,
        overview: if actions
            .iter()
            .any(|item| item.action_key == "owner-followup")
        {
            "Escalated incident. Prioritize routing and ownership actions first.".to_string()
        } else {
            "Actionable remediation suggestions based on anomaly code, policy scope, and routing context.".to_string()
        },
        actions,
    }
}

fn build_remediation_run_summary(
    runs: &[GatewayAnalysisAnomalyIncidentRemediationRunView],
) -> GatewayAnalysisAnomalyRemediationRunSummaryView {
    let mut by_status = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut by_policy_id = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut incident_ids = HashSet::new();
    let mut dry_run_runs = 0;
    let mut applied_runs = 0;
    let mut failed_runs = 0;
    let mut route_policy_changed_runs = 0;
    let mut incident_changed_runs = 0;

    for run in runs {
        accumulate_key_bucket(&mut by_status, Some(run.status.as_str()));
        accumulate_key_bucket(&mut by_execution_mode, Some(run.execution_mode.as_str()));
        accumulate_key_bucket(&mut by_action_key, Some(run.action_key.as_str()));
        accumulate_key_bucket(&mut by_policy_id, run.policy_id.as_deref());
        accumulate_key_bucket(&mut by_route_policy_id, run.route_policy_id.as_deref());
        incident_ids.insert(run.incident_id.clone());

        match run.status.as_str() {
            "dry_run" => dry_run_runs += 1,
            "applied" => applied_runs += 1,
            "failed" => failed_runs += 1,
            _ => {}
        }
        if has_route_policy_meaningful_changes(run) {
            route_policy_changed_runs += 1;
        }
        if has_incident_meaningful_changes(
            run.before_incident.as_ref(),
            run.after_incident.as_ref(),
        ) {
            incident_changed_runs += 1;
        }
    }

    GatewayAnalysisAnomalyRemediationRunSummaryView {
        total_runs: runs.len(),
        dry_run_runs,
        applied_runs,
        failed_runs,
        distinct_incident_count: incident_ids.len(),
        route_policy_changed_runs,
        incident_changed_runs,
        by_status: into_key_buckets(by_status),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_action_key: into_key_buckets(by_action_key),
        by_policy_id: into_key_buckets(by_policy_id),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
    }
}

fn build_run_impact(
    generated_at: OffsetDateTime,
    run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    incident: Option<GatewayAnalysisAnomalyIncidentView>,
    project_id: Option<String>,
    route_policy_id: Option<String>,
    anchor_at: OffsetDateTime,
    window_minutes: i32,
    before_started_at: OffsetDateTime,
    before_ended_at: OffsetDateTime,
    before_summary: GatewayAnalysisSummaryView,
    after_started_at: OffsetDateTime,
    after_ended_at: OffsetDateTime,
    after_summary: GatewayAnalysisSummaryView,
) -> GatewayAnalysisAnomalyRemediationRunImpactView {
    GatewayAnalysisAnomalyRemediationRunImpactView {
        generated_at: format_timestamp(generated_at),
        run,
        incident,
        project_id,
        route_policy_id,
        anchor_at: format_timestamp(anchor_at),
        window_minutes,
        before_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
            started_at: format_timestamp(before_started_at),
            ended_at: format_timestamp(before_ended_at),
            summary: before_summary.clone(),
        },
        after_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
            started_at: format_timestamp(after_started_at),
            ended_at: format_timestamp(after_ended_at),
            summary: after_summary.clone(),
        },
        metrics: GatewayAnalysisAnomalyRemediationRunImpactMetricsView {
            completion_rate: build_metric_delta(
                ratio(
                    before_summary.completed_samples,
                    before_summary.total_samples,
                ),
                ratio(after_summary.completed_samples, after_summary.total_samples),
            ),
            failure_rate: build_metric_delta(
                ratio(before_summary.failed_samples, before_summary.total_samples),
                ratio(after_summary.failed_samples, after_summary.total_samples),
            ),
            cancellation_rate: build_metric_delta(
                ratio(
                    before_summary.cancelled_samples,
                    before_summary.total_samples,
                ),
                ratio(after_summary.cancelled_samples, after_summary.total_samples),
            ),
            stream_rate: build_metric_delta(
                ratio(before_summary.stream_samples, before_summary.total_samples),
                ratio(after_summary.stream_samples, after_summary.total_samples),
            ),
            tool_request_rate: build_metric_delta(
                ratio(
                    before_summary.tool_request_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.tool_request_samples,
                    after_summary.total_samples,
                ),
            ),
            tool_response_rate: build_metric_delta(
                ratio(
                    before_summary.tool_response_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.tool_response_samples,
                    after_summary.total_samples,
                ),
            ),
            request_artifact_coverage: build_metric_delta(
                ratio(
                    before_summary.request_artifact_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.request_artifact_samples,
                    after_summary.total_samples,
                ),
            ),
            response_artifact_coverage: build_metric_delta(
                ratio(
                    before_summary.response_artifact_samples,
                    before_summary.total_samples,
                ),
                ratio(
                    after_summary.response_artifact_samples,
                    after_summary.total_samples,
                ),
            ),
            prompt_tokens_per_sample: build_metric_delta(
                per_sample(
                    before_summary.total_prompt_tokens,
                    before_summary.total_samples,
                ),
                per_sample(
                    after_summary.total_prompt_tokens,
                    after_summary.total_samples,
                ),
            ),
            completion_tokens_per_sample: build_metric_delta(
                per_sample(
                    before_summary.total_completion_tokens,
                    before_summary.total_samples,
                ),
                per_sample(
                    after_summary.total_completion_tokens,
                    after_summary.total_samples,
                ),
            ),
            total_tokens_per_sample: build_metric_delta(
                per_sample(before_summary.total_tokens, before_summary.total_samples),
                per_sample(after_summary.total_tokens, after_summary.total_samples),
            ),
            request_text_chars_avg: build_metric_delta(
                before_summary.request_text_chars.avg,
                after_summary.request_text_chars.avg,
            ),
            response_text_chars_avg: build_metric_delta(
                before_summary.response_text_chars.avg,
                after_summary.response_text_chars.avg,
            ),
            first_token_latency_ms_avg: build_metric_delta(
                before_summary.first_token_latency_ms.avg,
                after_summary.first_token_latency_ms.avg,
            ),
            stream_chunk_count_avg: build_metric_delta(
                before_summary.stream_chunk_count.avg,
                after_summary.stream_chunk_count.avg,
            ),
        },
    }
}

fn build_remediation_effectiveness_summary(
    generated_at: OffsetDateTime,
    window_minutes: i32,
    runs: &[GatewayAnalysisAnomalyIncidentRemediationRunView],
    impacts: &[Option<GatewayAnalysisAnomalyRemediationRunImpactView>],
) -> GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
    let mut by_status = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut action_map = BTreeMap::new();

    let mut completion_rate = empty_effectiveness_metric();
    let mut failure_rate = empty_effectiveness_metric();
    let mut request_artifact_coverage = empty_effectiveness_metric();
    let mut response_artifact_coverage = empty_effectiveness_metric();
    let mut first_token_latency_ms_avg = empty_effectiveness_metric();
    let mut total_tokens_per_sample = empty_effectiveness_metric();

    let mut impacted_runs = 0;
    let mut unavailable_runs = 0;

    for (index, run) in runs.iter().enumerate() {
        accumulate_key_bucket(&mut by_status, Some(run.status.as_str()));
        accumulate_key_bucket(&mut by_execution_mode, Some(run.execution_mode.as_str()));
        accumulate_key_bucket(&mut by_action_key, Some(run.action_key.as_str()));

        let entry = action_map
            .entry(run.action_key.clone())
            .or_insert_with(|| empty_action_effectiveness(run.action_key.clone()));
        entry.run_count += 1;

        let Some(impact) = impacts.get(index).and_then(|item| item.as_ref()) else {
            unavailable_runs += 1;
            entry.unavailable_run_count += 1;
            add_metric_classification(&mut completion_rate, Classification::Unavailable);
            add_metric_classification(&mut failure_rate, Classification::Unavailable);
            add_metric_classification(&mut request_artifact_coverage, Classification::Unavailable);
            add_metric_classification(&mut response_artifact_coverage, Classification::Unavailable);
            add_metric_classification(&mut first_token_latency_ms_avg, Classification::Unavailable);
            add_metric_classification(&mut total_tokens_per_sample, Classification::Unavailable);
            add_metric_classification(&mut entry.completion_rate, Classification::Unavailable);
            add_metric_classification(&mut entry.failure_rate, Classification::Unavailable);
            add_metric_classification(
                &mut entry.request_artifact_coverage,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.response_artifact_coverage,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.first_token_latency_ms_avg,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.total_tokens_per_sample,
                Classification::Unavailable,
            );
            continue;
        };

        impacted_runs += 1;
        entry.impacted_run_count += 1;
        let classifications = [
            (
                "completion_rate",
                classify_metric(
                    &impact.metrics.completion_rate,
                    MetricDirection::HigherBetter,
                    Some(0.02),
                    None,
                ),
            ),
            (
                "failure_rate",
                classify_metric(
                    &impact.metrics.failure_rate,
                    MetricDirection::LowerBetter,
                    Some(0.02),
                    None,
                ),
            ),
            (
                "request_artifact_coverage",
                classify_metric(
                    &impact.metrics.request_artifact_coverage,
                    MetricDirection::HigherBetter,
                    Some(0.05),
                    None,
                ),
            ),
            (
                "response_artifact_coverage",
                classify_metric(
                    &impact.metrics.response_artifact_coverage,
                    MetricDirection::HigherBetter,
                    Some(0.05),
                    None,
                ),
            ),
            (
                "first_token_latency_ms_avg",
                classify_metric(
                    &impact.metrics.first_token_latency_ms_avg,
                    MetricDirection::LowerBetter,
                    Some(50.0),
                    None,
                ),
            ),
            (
                "total_tokens_per_sample",
                classify_metric(
                    &impact.metrics.total_tokens_per_sample,
                    MetricDirection::LowerBetter,
                    None,
                    Some(0.1),
                ),
            ),
        ];
        for (key, classification) in classifications {
            match key {
                "completion_rate" => {
                    add_metric_classification(&mut completion_rate, classification);
                    add_metric_classification(&mut entry.completion_rate, classification);
                }
                "failure_rate" => {
                    add_metric_classification(&mut failure_rate, classification);
                    add_metric_classification(&mut entry.failure_rate, classification);
                }
                "request_artifact_coverage" => {
                    add_metric_classification(&mut request_artifact_coverage, classification);
                    add_metric_classification(&mut entry.request_artifact_coverage, classification);
                }
                "response_artifact_coverage" => {
                    add_metric_classification(&mut response_artifact_coverage, classification);
                    add_metric_classification(
                        &mut entry.response_artifact_coverage,
                        classification,
                    );
                }
                "first_token_latency_ms_avg" => {
                    add_metric_classification(&mut first_token_latency_ms_avg, classification);
                    add_metric_classification(
                        &mut entry.first_token_latency_ms_avg,
                        classification,
                    );
                }
                "total_tokens_per_sample" => {
                    add_metric_classification(&mut total_tokens_per_sample, classification);
                    add_metric_classification(&mut entry.total_tokens_per_sample, classification);
                }
                _ => {}
            }
        }
    }

    GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
        generated_at: format_timestamp(generated_at),
        window_minutes,
        total_runs: runs.len(),
        impacted_runs,
        unavailable_runs,
        by_status: into_key_buckets(by_status),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_action_key: into_key_buckets(by_action_key),
        completion_rate,
        failure_rate,
        request_artifact_coverage,
        response_artifact_coverage,
        first_token_latency_ms_avg,
        total_tokens_per_sample,
        actions: action_map.into_values().collect(),
    }
}

fn build_remediation_effectiveness_snapshot_inventory_summary(
    snapshots: &[GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView],
) -> GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_label = BTreeMap::new();
    let mut total_runs = 0;
    let mut total_impacted_runs = 0;
    let mut total_unavailable_runs = 0;

    for snapshot in snapshots {
        total_runs += snapshot.summary.total_runs;
        total_impacted_runs += snapshot.summary.impacted_runs;
        total_unavailable_runs += snapshot.summary.unavailable_runs;
        accumulate_key_bucket(
            &mut by_route_policy_id,
            snapshot.filters.route_policy_id.as_deref(),
        );
        accumulate_key_bucket(&mut by_action_key, snapshot.filters.action_key.as_deref());
        accumulate_key_bucket(
            &mut by_execution_mode,
            snapshot.filters.execution_mode.as_deref(),
        );
        accumulate_key_bucket(&mut by_label, snapshot.label.as_deref());
    }

    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
        total_snapshots: snapshots.len(),
        total_runs,
        total_impacted_runs,
        total_unavailable_runs,
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_action_key: into_key_buckets(by_action_key),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_label: into_key_buckets(by_label),
    }
}

fn build_remediation_effectiveness_trend_point(
    snapshot: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
    let total_runs = snapshot.summary.total_runs;
    GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
        total_runs,
        impacted_run_rate: ratio(snapshot.summary.impacted_runs, total_runs),
        unavailable_run_rate: ratio(snapshot.summary.unavailable_runs, total_runs),
        completion_rate: build_trend_metric_point(&snapshot.summary.completion_rate),
        failure_rate: build_trend_metric_point(&snapshot.summary.failure_rate),
        request_artifact_coverage: build_trend_metric_point(
            &snapshot.summary.request_artifact_coverage,
        ),
        response_artifact_coverage: build_trend_metric_point(
            &snapshot.summary.response_artifact_coverage,
        ),
        first_token_latency_ms_avg: build_trend_metric_point(
            &snapshot.summary.first_token_latency_ms_avg,
        ),
        total_tokens_per_sample: build_trend_metric_point(
            &snapshot.summary.total_tokens_per_sample,
        ),
        snapshot,
    }
}

fn build_trend_metric_point(
    metric: &GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
    let total = metric.improved_runs
        + metric.regressed_runs
        + metric.neutral_runs
        + metric.unavailable_runs;
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
        improved_rate: ratio(metric.improved_runs, total),
        regressed_rate: ratio(metric.regressed_runs, total),
        neutral_rate: ratio(metric.neutral_runs, total),
        unavailable_rate: ratio(metric.unavailable_runs, total),
    }
}

fn build_remediation_effectiveness_trend_summary(
    points: &[GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView],
) -> Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(
        GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView {
            latest_snapshot_id: Some(latest.snapshot.snapshot_id.clone()),
            previous_snapshot_id: previous.map(|item| item.snapshot.snapshot_id.clone()),
            total_runs: build_trend_metric_summary(
                Some(latest.total_runs as f64),
                previous.map(|item| item.total_runs as f64),
            ),
            impacted_run_rate: build_trend_metric_summary(
                latest.impacted_run_rate,
                previous.and_then(|item| item.impacted_run_rate),
            ),
            unavailable_run_rate: build_trend_metric_summary(
                latest.unavailable_run_rate,
                previous.and_then(|item| item.unavailable_run_rate),
            ),
            completion_rate_regressed: build_trend_metric_summary(
                latest.completion_rate.regressed_rate,
                previous.and_then(|item| item.completion_rate.regressed_rate),
            ),
            failure_rate_regressed: build_trend_metric_summary(
                latest.failure_rate.regressed_rate,
                previous.and_then(|item| item.failure_rate.regressed_rate),
            ),
            request_artifact_coverage_regressed: build_trend_metric_summary(
                latest.request_artifact_coverage.regressed_rate,
                previous.and_then(|item| item.request_artifact_coverage.regressed_rate),
            ),
            response_artifact_coverage_regressed: build_trend_metric_summary(
                latest.response_artifact_coverage.regressed_rate,
                previous.and_then(|item| item.response_artifact_coverage.regressed_rate),
            ),
            first_token_latency_ms_avg_regressed: build_trend_metric_summary(
                latest.first_token_latency_ms_avg.regressed_rate,
                previous.and_then(|item| item.first_token_latency_ms_avg.regressed_rate),
            ),
            total_tokens_per_sample_regressed: build_trend_metric_summary(
                latest.total_tokens_per_sample.regressed_rate,
                previous.and_then(|item| item.total_tokens_per_sample.regressed_rate),
            ),
        },
    )
}

fn build_trend_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta)) if previous != 0.0 => Some(delta / previous),
        _ => None,
    };
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}

fn normalize_profile_key(profile_key: Option<&str>) -> String {
    match profile_key
        .and_then(trimmed_owned_ref)
        .unwrap_or("balanced")
        .to_lowercase()
        .as_str()
    {
        "conservative" => "conservative".to_string(),
        "aggressive" => "aggressive".to_string(),
        _ => "balanced".to_string(),
    }
}

fn build_remediation_effectiveness_threshold_config(
    profile_key: &str,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.5,
            impacted_run_rate_critical_threshold: 0.35,
            unavailable_run_rate_warning_threshold: 0.4,
            unavailable_run_rate_critical_threshold: 0.6,
            completion_rate_regressed_warning_threshold: 0.35,
            completion_rate_regressed_critical_threshold: 0.55,
            failure_rate_regressed_warning_threshold: 0.35,
            failure_rate_regressed_critical_threshold: 0.55,
            request_artifact_regressed_warning_threshold: 0.35,
            request_artifact_regressed_critical_threshold: 0.55,
            response_artifact_regressed_warning_threshold: 0.35,
            response_artifact_regressed_critical_threshold: 0.55,
            first_token_latency_regressed_warning_threshold: 0.35,
            first_token_latency_regressed_critical_threshold: 0.55,
            total_tokens_regressed_warning_threshold: 0.35,
            total_tokens_regressed_critical_threshold: 0.55,
        },
        "aggressive" => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.75,
            impacted_run_rate_critical_threshold: 0.6,
            unavailable_run_rate_warning_threshold: 0.18,
            unavailable_run_rate_critical_threshold: 0.3,
            completion_rate_regressed_warning_threshold: 0.18,
            completion_rate_regressed_critical_threshold: 0.3,
            failure_rate_regressed_warning_threshold: 0.18,
            failure_rate_regressed_critical_threshold: 0.3,
            request_artifact_regressed_warning_threshold: 0.18,
            request_artifact_regressed_critical_threshold: 0.3,
            response_artifact_regressed_warning_threshold: 0.18,
            response_artifact_regressed_critical_threshold: 0.3,
            first_token_latency_regressed_warning_threshold: 0.18,
            first_token_latency_regressed_critical_threshold: 0.3,
            total_tokens_regressed_warning_threshold: 0.18,
            total_tokens_regressed_critical_threshold: 0.3,
        },
        _ => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.65,
            impacted_run_rate_critical_threshold: 0.45,
            unavailable_run_rate_warning_threshold: 0.25,
            unavailable_run_rate_critical_threshold: 0.4,
            completion_rate_regressed_warning_threshold: 0.25,
            completion_rate_regressed_critical_threshold: 0.4,
            failure_rate_regressed_warning_threshold: 0.25,
            failure_rate_regressed_critical_threshold: 0.4,
            request_artifact_regressed_warning_threshold: 0.25,
            request_artifact_regressed_critical_threshold: 0.4,
            response_artifact_regressed_warning_threshold: 0.25,
            response_artifact_regressed_critical_threshold: 0.4,
            first_token_latency_regressed_warning_threshold: 0.25,
            first_token_latency_regressed_critical_threshold: 0.4,
            total_tokens_regressed_warning_threshold: 0.25,
            total_tokens_regressed_critical_threshold: 0.4,
        },
    };
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
        impacted_run_rate_warning_threshold: overrides
            .impacted_run_rate_warning_threshold
            .unwrap_or(base.impacted_run_rate_warning_threshold),
        impacted_run_rate_critical_threshold: overrides
            .impacted_run_rate_critical_threshold
            .unwrap_or(base.impacted_run_rate_critical_threshold),
        unavailable_run_rate_warning_threshold: overrides
            .unavailable_run_rate_warning_threshold
            .unwrap_or(base.unavailable_run_rate_warning_threshold),
        unavailable_run_rate_critical_threshold: overrides
            .unavailable_run_rate_critical_threshold
            .unwrap_or(base.unavailable_run_rate_critical_threshold),
        completion_rate_regressed_warning_threshold: overrides
            .completion_rate_regressed_warning_threshold
            .unwrap_or(base.completion_rate_regressed_warning_threshold),
        completion_rate_regressed_critical_threshold: overrides
            .completion_rate_regressed_critical_threshold
            .unwrap_or(base.completion_rate_regressed_critical_threshold),
        failure_rate_regressed_warning_threshold: overrides
            .failure_rate_regressed_warning_threshold
            .unwrap_or(base.failure_rate_regressed_warning_threshold),
        failure_rate_regressed_critical_threshold: overrides
            .failure_rate_regressed_critical_threshold
            .unwrap_or(base.failure_rate_regressed_critical_threshold),
        request_artifact_regressed_warning_threshold: overrides
            .request_artifact_regressed_warning_threshold
            .unwrap_or(base.request_artifact_regressed_warning_threshold),
        request_artifact_regressed_critical_threshold: overrides
            .request_artifact_regressed_critical_threshold
            .unwrap_or(base.request_artifact_regressed_critical_threshold),
        response_artifact_regressed_warning_threshold: overrides
            .response_artifact_regressed_warning_threshold
            .unwrap_or(base.response_artifact_regressed_warning_threshold),
        response_artifact_regressed_critical_threshold: overrides
            .response_artifact_regressed_critical_threshold
            .unwrap_or(base.response_artifact_regressed_critical_threshold),
        first_token_latency_regressed_warning_threshold: overrides
            .first_token_latency_regressed_warning_threshold
            .unwrap_or(base.first_token_latency_regressed_warning_threshold),
        first_token_latency_regressed_critical_threshold: overrides
            .first_token_latency_regressed_critical_threshold
            .unwrap_or(base.first_token_latency_regressed_critical_threshold),
        total_tokens_regressed_warning_threshold: overrides
            .total_tokens_regressed_warning_threshold
            .unwrap_or(base.total_tokens_regressed_warning_threshold),
        total_tokens_regressed_critical_threshold: overrides
            .total_tokens_regressed_critical_threshold
            .unwrap_or(base.total_tokens_regressed_critical_threshold),
    }
}

fn build_remediation_effectiveness_anomaly_report(
    trend_report: GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView,
    profile_key: String,
    thresholds: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
) -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
    let mut anomalies = Vec::new();
    if let Some(summary) = trend_report.summary.as_ref() {
        push_remediation_effectiveness_anomaly(
            &mut anomalies,
            "impacted_run_rate_drop",
            "治理效果样本命中率偏低，当前窗口里可评估 run 太少，效果结论不稳定。",
            summary.impacted_run_rate.latest_value,
            summary.impacted_run_rate.previous_value,
            summary.impacted_run_rate.delta_value,
            summary.impacted_run_rate.delta_ratio,
            thresholds.impacted_run_rate_warning_threshold,
            thresholds.impacted_run_rate_critical_threshold,
            true,
            summary.latest_snapshot_id.as_deref(),
            summary.previous_snapshot_id.as_deref(),
        );
        push_remediation_effectiveness_anomaly(
            &mut anomalies,
            "unavailable_run_rate_spike",
            "治理效果快照里的 unavailable run 比例过高，impact capture 链路需要排查。",
            summary.unavailable_run_rate.latest_value,
            summary.unavailable_run_rate.previous_value,
            summary.unavailable_run_rate.delta_value,
            summary.unavailable_run_rate.delta_ratio,
            thresholds.unavailable_run_rate_warning_threshold,
            thresholds.unavailable_run_rate_critical_threshold,
            false,
            summary.latest_snapshot_id.as_deref(),
            summary.previous_snapshot_id.as_deref(),
        );
        let checks = [
            (
                "completion_effectiveness_regressed",
                "completion 效果回归比例偏高，最近一批 remediation 可能没有改善完成率。",
                &summary.completion_rate_regressed,
                thresholds.completion_rate_regressed_warning_threshold,
                thresholds.completion_rate_regressed_critical_threshold,
            ),
            (
                "failure_effectiveness_regressed",
                "failure 效果回归比例偏高，最近一批 remediation 可能放大了失败率。",
                &summary.failure_rate_regressed,
                thresholds.failure_rate_regressed_warning_threshold,
                thresholds.failure_rate_regressed_critical_threshold,
            ),
            (
                "request_artifact_effectiveness_regressed",
                "request artifact 效果回归比例偏高，治理动作可能伤到了留存链路。",
                &summary.request_artifact_coverage_regressed,
                thresholds.request_artifact_regressed_warning_threshold,
                thresholds.request_artifact_regressed_critical_threshold,
            ),
            (
                "response_artifact_effectiveness_regressed",
                "response artifact 效果回归比例偏高，治理动作可能伤到了 response 留存覆盖率。",
                &summary.response_artifact_coverage_regressed,
                thresholds.response_artifact_regressed_warning_threshold,
                thresholds.response_artifact_regressed_critical_threshold,
            ),
            (
                "latency_effectiveness_regressed",
                "首 token latency 的回归比例偏高，最近一批 remediation 可能拖慢了热路径。",
                &summary.first_token_latency_ms_avg_regressed,
                thresholds.first_token_latency_regressed_warning_threshold,
                thresholds.first_token_latency_regressed_critical_threshold,
            ),
            (
                "token_effectiveness_regressed",
                "单样本 token 成本的回归比例偏高，最近一批 remediation 可能提高了成本。",
                &summary.total_tokens_per_sample_regressed,
                thresholds.total_tokens_regressed_warning_threshold,
                thresholds.total_tokens_regressed_critical_threshold,
            ),
        ];
        for (code, message, metric, warning, critical) in checks {
            if metric.latest_value.unwrap_or(0.0) < warning {
                continue;
            }
            anomalies.push(GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
                code: code.to_string(),
                severity: if metric.latest_value.unwrap_or(0.0) >= critical {
                    "critical".to_string()
                } else {
                    "warning".to_string()
                },
                message: message.to_string(),
                latest_snapshot_id: summary.latest_snapshot_id.clone(),
                previous_snapshot_id: summary.previous_snapshot_id.clone(),
                latest_value: metric.latest_value,
                previous_value: metric.previous_value,
                delta_value: metric.delta_value,
                delta_ratio: metric.delta_ratio,
                threshold_value: Some(warning),
            });
        }
    }
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    for anomaly in &anomalies {
        accumulate_key_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_key_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
        generated_at: trend_report.generated_at.clone(),
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        latest_snapshot: trend_report
            .points
            .first()
            .map(|item| item.snapshot.clone()),
        previous_snapshot: trend_report.points.get(1).map(|item| item.snapshot.clone()),
        trend_summary: trend_report.summary,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

fn push_remediation_effectiveness_anomaly(
    anomalies: &mut Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView>,
    code: &str,
    message: &str,
    latest_value: Option<f64>,
    previous_value: Option<f64>,
    delta_value: Option<f64>,
    delta_ratio: Option<f64>,
    warning: f64,
    critical: f64,
    lower_is_worse: bool,
    latest_snapshot_id: Option<&str>,
    previous_snapshot_id: Option<&str>,
) {
    let value = latest_value.unwrap_or(if lower_is_worse { 1.0 } else { 0.0 });
    let triggered = if lower_is_worse {
        value <= warning
    } else {
        value >= warning
    };
    if !triggered {
        return;
    }
    let severity = if if lower_is_worse {
        value <= critical
    } else {
        value >= critical
    } {
        "critical"
    } else {
        "warning"
    };
    anomalies.push(GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_snapshot_id: latest_snapshot_id.map(|value| value.to_string()),
        previous_snapshot_id: previous_snapshot_id.map(|value| value.to_string()),
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
        threshold_value: Some(warning),
    });
}

fn matches_remediation_effectiveness_snapshot_filters(
    snapshot: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = filters.snapshot_id.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = filters.label.as_deref().and_then(trimmed_owned_ref) {
        let needle = label.to_lowercase();
        let haystack = snapshot.label.as_deref().unwrap_or_default().to_lowercase();
        if !haystack.contains(&needle) {
            return false;
        }
    }
    if let Some(route_policy_id) = filters
        .route_policy_id
        .as_deref()
        .and_then(trimmed_owned_ref)
    {
        if snapshot.filters.route_policy_id.as_deref() != Some(route_policy_id) {
            return false;
        }
    }
    if let Some(action_key) = filters.action_key.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.filters.action_key.as_deref() != Some(action_key) {
            return false;
        }
    }
    let created_at = OffsetDateTime::parse(&snapshot.created_at, &Rfc3339).ok();
    if let (Some(created_from), Some(created_at)) = (created_from, created_at) {
        if created_at < created_from {
            return false;
        }
    }
    if let (Some(created_to), Some(created_at)) = (created_to, created_at) {
        if created_at > created_to {
            return false;
        }
    }
    true
}

fn matches_remediation_effectiveness_anomaly_snapshot_filters(
    snapshot: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = filters.snapshot_id.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = filters.label.as_deref().and_then(trimmed_owned_ref) {
        let needle = label.to_lowercase();
        let haystack = snapshot.label.as_deref().unwrap_or_default().to_lowercase();
        if !haystack.contains(&needle) {
            return false;
        }
    }
    if let Some(route_policy_id) = filters
        .route_policy_id
        .as_deref()
        .and_then(trimmed_owned_ref)
    {
        if snapshot.filters.route_policy_id.as_deref() != Some(route_policy_id) {
            return false;
        }
    }
    if let Some(action_key) = filters.action_key.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.filters.action_key.as_deref() != Some(action_key) {
            return false;
        }
    }
    if let Some(profile_key) = filters.profile_key.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.filters.profile_key != normalize_profile_key(Some(profile_key)) {
            return false;
        }
    }
    let created_at = OffsetDateTime::parse(&snapshot.created_at, &Rfc3339).ok();
    if let (Some(created_from), Some(created_at)) = (created_from, created_at) {
        if created_at < created_from {
            return false;
        }
    }
    if let (Some(created_to), Some(created_at)) = (created_to, created_at) {
        if created_at > created_to {
            return false;
        }
    }
    true
}

fn build_remediation_effectiveness_snapshot_object_key(snapshot_id: &str) -> String {
    format!("ai-gateway/remediation-effectiveness-snapshots/{snapshot_id}/snapshot.json")
}

fn build_remediation_effectiveness_anomaly_snapshot_object_key(snapshot_id: &str) -> String {
    format!("ai-gateway/remediation-effectiveness-anomaly-snapshots/{snapshot_id}/snapshot.json")
}

fn ratio(count: usize, total: usize) -> Option<f64> {
    if total == 0 {
        None
    } else {
        Some(round4(count as f64 / total as f64))
    }
}

fn per_sample(total: i64, sample_count: usize) -> Option<f64> {
    if sample_count == 0 {
        None
    } else {
        Some(round4(total as f64 / sample_count as f64))
    }
}

fn build_metric_delta(
    before_value: Option<f64>,
    after_value: Option<f64>,
) -> GatewayAnalysisAnomalyRemediationImpactMetricView {
    let delta_value = match (before_value, after_value) {
        (Some(before), Some(after)) => Some(round4(after - before)),
        _ => None,
    };
    let delta_ratio = match (before_value, after_value) {
        (Some(before), Some(after)) if before != 0.0 => Some(round4((after - before) / before)),
        _ => None,
    };
    GatewayAnalysisAnomalyRemediationImpactMetricView {
        before_value,
        after_value,
        delta_value,
        delta_ratio,
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn build_follow_up_input(
    incident: &GatewayAnalysisAnomalyIncidentView,
    requested: Option<GatewayAnalysisAnomalyIncidentFollowUpInput>,
) -> GatewayAnalysisAnomalyIncidentFollowUpInput {
    let requested = requested.unwrap_or_default();
    GatewayAnalysisAnomalyIncidentFollowUpInput {
        owner_user_id: Some(
            requested
                .owner_user_id
                .or_else(|| incident.owner_user_id.clone())
                .unwrap_or_default(),
        ),
        follow_up_status: Some(requested.follow_up_status.unwrap_or_else(|| {
            if incident.follow_up_status == "pending" {
                "investigating".to_string()
            } else {
                incident.follow_up_status.clone()
            }
        })),
        note: Some(
            requested
                .note
                .or_else(|| incident.latest_note.clone())
                .unwrap_or_default(),
        ),
        resolution_note: Some(
            requested
                .resolution_note
                .or_else(|| incident.resolution_note.clone())
                .unwrap_or_default(),
        ),
    }
}

fn simulate_follow_up_update(
    incident: &GatewayAnalysisAnomalyIncidentView,
    input: &GatewayAnalysisAnomalyIncidentFollowUpInput,
    timestamp: OffsetDateTime,
) -> GatewayAnalysisAnomalyIncidentView {
    let mut updated = incident.clone();
    if let Some(owner_user_id) = input.owner_user_id.as_deref() {
        updated.owner_user_id = trimmed_owned(Some(owner_user_id));
    }
    if let Some(follow_up_status) = input.follow_up_status.as_deref() {
        updated.follow_up_status = follow_up_status.trim().to_string();
    }
    if let Some(note) = input.note.as_deref() {
        updated.latest_note = trimmed_owned(Some(note));
    }
    if let Some(resolution_note) = input.resolution_note.as_deref() {
        updated.resolution_note = trimmed_owned(Some(resolution_note));
    }
    updated.last_action_at = Some(format_timestamp(timestamp));
    updated.updated_at = format_timestamp(timestamp);
    updated
}

#[derive(Debug, Clone, Copy)]
enum MetricDirection {
    HigherBetter,
    LowerBetter,
}

#[derive(Debug, Clone, Copy)]
enum Classification {
    Improved,
    Regressed,
    Neutral,
    Unavailable,
}

fn empty_effectiveness_metric() -> GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
    GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
        improved_runs: 0,
        regressed_runs: 0,
        neutral_runs: 0,
        unavailable_runs: 0,
    }
}

fn empty_action_effectiveness(
    action_key: String,
) -> GatewayAnalysisAnomalyRemediationActionEffectivenessView {
    GatewayAnalysisAnomalyRemediationActionEffectivenessView {
        action_key,
        run_count: 0,
        impacted_run_count: 0,
        unavailable_run_count: 0,
        completion_rate: empty_effectiveness_metric(),
        failure_rate: empty_effectiveness_metric(),
        request_artifact_coverage: empty_effectiveness_metric(),
        response_artifact_coverage: empty_effectiveness_metric(),
        first_token_latency_ms_avg: empty_effectiveness_metric(),
        total_tokens_per_sample: empty_effectiveness_metric(),
    }
}

fn classify_metric(
    metric: &GatewayAnalysisAnomalyRemediationImpactMetricView,
    direction: MetricDirection,
    absolute_threshold: Option<f64>,
    ratio_threshold: Option<f64>,
) -> Classification {
    if metric.before_value.is_none() || metric.after_value.is_none() || metric.delta_value.is_none()
    {
        return Classification::Unavailable;
    }
    if let Some(ratio_threshold) = ratio_threshold {
        let Some(delta_ratio) = metric.delta_ratio else {
            return Classification::Unavailable;
        };
        if delta_ratio.abs() < ratio_threshold {
            return Classification::Neutral;
        }
        return match direction {
            MetricDirection::HigherBetter => {
                if delta_ratio > 0.0 {
                    Classification::Improved
                } else {
                    Classification::Regressed
                }
            }
            MetricDirection::LowerBetter => {
                if delta_ratio < 0.0 {
                    Classification::Improved
                } else {
                    Classification::Regressed
                }
            }
        };
    }

    let delta_value = metric.delta_value.unwrap_or(0.0);
    if delta_value.abs() < absolute_threshold.unwrap_or(0.0) {
        return Classification::Neutral;
    }
    match direction {
        MetricDirection::HigherBetter => {
            if delta_value > 0.0 {
                Classification::Improved
            } else {
                Classification::Regressed
            }
        }
        MetricDirection::LowerBetter => {
            if delta_value < 0.0 {
                Classification::Improved
            } else {
                Classification::Regressed
            }
        }
    }
}

fn add_metric_classification(
    metric: &mut GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    classification: Classification,
) {
    match classification {
        Classification::Improved => metric.improved_runs += 1,
        Classification::Regressed => metric.regressed_runs += 1,
        Classification::Neutral => metric.neutral_runs += 1,
        Classification::Unavailable => metric.unavailable_runs += 1,
    }
}

#[derive(Debug, Clone)]
struct RoutePolicyPatchSpec {
    next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput,
    changed_fields: Vec<String>,
    summary: String,
}

fn build_tighter_rate_limit(
    current: Option<&GatewayRateLimitDefinition>,
    fallback: &GatewayRateLimitDefinition,
) -> GatewayRateLimitDefinition {
    let Some(current) = current else {
        return fallback.clone();
    };
    if current.window_seconds <= 0 || current.max_requests <= 0 {
        return fallback.clone();
    }
    let reduction = (f64::from(current.max_requests) * 0.2).ceil() as i32;
    GatewayRateLimitDefinition {
        window_seconds: current.window_seconds,
        max_requests: (current.max_requests - reduction.max(1)).max(1),
    }
}

fn normalize_provider_account_ids(values: Option<&[String]>) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or(&[]) {
        let Some(value) = trimmed_owned_ref(value) else {
            continue;
        };
        if seen.insert(value.to_string()) {
            normalized.push(value.to_string());
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_scoped_key(
    label: &str,
    value: Option<&str>,
    lower_case: bool,
) -> Result<String, GatewayError> {
    let value = trimmed_owned_ref_opt(value)
        .ok_or_else(|| GatewayError::conflict(format!("{label} 不能为空。")))?;
    Ok(if lower_case {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    })
}

fn normalize_rate_limit_patch_definition(
    label: &str,
    value: Option<GatewayRateLimitDefinition>,
    fallback: Option<GatewayRateLimitDefinition>,
) -> Result<Option<GatewayRateLimitDefinition>, GatewayError> {
    let definition = value.or(fallback);
    let Some(definition) = definition else {
        return Ok(None);
    };
    if definition.window_seconds <= 0 || definition.max_requests <= 0 {
        return Err(GatewayError::conflict(format!(
            "{label} 需要同时提供大于 0 的 windowSeconds 和 maxRequests。"
        )));
    }
    Ok(Some(definition))
}

fn ensure_tightened_rate_limit(
    label: &str,
    current: Option<&GatewayRateLimitDefinition>,
    next: &GatewayRateLimitDefinition,
) -> Result<(), GatewayError> {
    if next.window_seconds <= 0 || next.max_requests <= 0 {
        return Err(GatewayError::conflict(format!(
            "{label} 需要同时提供 windowSeconds 和 maxRequests。"
        )));
    }
    let Some(current) = current else {
        return Ok(());
    };
    if current.window_seconds <= 0 || current.max_requests <= 0 {
        return Ok(());
    }
    if next.max_requests > current.max_requests {
        return Err(GatewayError::conflict(format!(
            "{label} 当前 remediation 只允许下调 maxRequests，不允许上调。"
        )));
    }
    if next.window_seconds < current.window_seconds {
        return Err(GatewayError::conflict(format!(
            "{label} 当前 remediation 只允许保持或放大 windowSeconds，不允许缩短窗口。"
        )));
    }
    Ok(())
}

fn build_route_policy_patch(
    action_key: &str,
    route_policy: &GatewayRoutePolicyView,
    input: Option<GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput>,
) -> Result<RoutePolicyPatchSpec, GatewayError> {
    let input = input.unwrap_or_default();
    match action_key {
        "disable-prestream-fallback" => {
            let next = GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                pre_stream_fallback_enabled: Some(false),
                ..Default::default()
            };
            let changed_fields = if route_policy.config.pre_stream_fallback_enabled {
                vec!["preStreamFallbackEnabled".to_string()]
            } else {
                Vec::new()
            };
            Ok(RoutePolicyPatchSpec {
                next,
                changed_fields,
                summary: "Set preStreamFallbackEnabled=false for the linked route policy."
                    .to_string(),
            })
        }
        "reduce-provider-concurrency" => {
            let current_value = route_policy.config.provider_max_concurrent_requests;
            let target_value = input.provider_max_concurrent_requests.unwrap_or_else(|| {
                current_value
                    .map(|value| value.saturating_sub(1).max(1))
                    .unwrap_or(1)
            });
            if target_value < 1 {
                return Err(GatewayError::conflict(
                    "providerMaxConcurrentRequests 必须是大于等于 1 的整数。",
                ));
            }
            if current_value.is_some_and(|value| target_value > value) {
                return Err(GatewayError::conflict(
                    "当前 remediation 只允许下调 providerMaxConcurrentRequests，不允许上调。",
                ));
            }
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    provider_max_concurrent_requests: Some(target_value),
                    ..Default::default()
                },
                changed_fields: if current_value == Some(target_value) {
                    Vec::new()
                } else {
                    vec!["providerMaxConcurrentRequests".to_string()]
                },
                summary: format!("Set providerMaxConcurrentRequests={target_value}."),
            })
        }
        "provider-isolation" => {
            let next_provider_ids =
                normalize_provider_account_ids(input.allowed_provider_account_ids.as_deref())
                    .ok_or_else(|| {
                        GatewayError::conflict(
                    "provider-isolation remediation 需要显式提供 allowedProviderAccountIds。",
                )
                    })?;
            let current_allowlist = normalize_provider_account_ids(
                route_policy.config.allowed_provider_account_ids.as_deref(),
            );
            if current_allowlist.as_ref().is_some_and(|items| {
                next_provider_ids
                    .iter()
                    .any(|value| !items.iter().any(|item| item == value))
            }) {
                return Err(GatewayError::conflict(
                    "provider-isolation 只能收窄当前 allowlist，不允许引入新的 providerAccountId。",
                ));
            }
            let changed_fields = if current_allowlist.as_ref() == Some(&next_provider_ids) {
                Vec::new()
            } else {
                vec!["allowedProviderAccountIds".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    allowed_provider_account_ids: Some(next_provider_ids.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Narrow allowedProviderAccountIds to {}.",
                    next_provider_ids.join(", ")
                ),
            })
        }
        "tighten-project-rate-limit" => {
            let current_definition = match (
                route_policy.config.rate_limit_window_seconds,
                route_policy.config.rate_limit_max_requests,
            ) {
                (Some(window_seconds), Some(max_requests)) => Some(GatewayRateLimitDefinition {
                    window_seconds,
                    max_requests,
                }),
                _ => None,
            };
            let next_definition = normalize_rate_limit_patch_definition(
                "projectRateLimit",
                input.project_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-project-rate-limit 需要显式提供 projectRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                "projectRateLimit",
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy.config.rate_limit_window_seconds
                == Some(next_definition.window_seconds)
                && route_policy.config.rate_limit_max_requests == Some(next_definition.max_requests)
            {
                Vec::new()
            } else {
                vec![
                    "rateLimitWindowSeconds".to_string(),
                    "rateLimitMaxRequests".to_string(),
                ]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    project_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set project rate limit to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-api-key-rate-limit" => {
            let next_definition = normalize_rate_limit_patch_definition(
                "apiKeyRateLimit",
                input.api_key_rate_limit,
                route_policy.config.api_key_rate_limit.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-api-key-rate-limit 需要显式提供 apiKeyRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                "apiKeyRateLimit",
                route_policy.config.api_key_rate_limit.as_ref(),
                &next_definition,
            )?;
            let changed_fields =
                if route_policy.config.api_key_rate_limit.as_ref() == Some(&next_definition) {
                    Vec::new()
                } else {
                    vec!["apiKeyRateLimit".to_string()]
                };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    api_key_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set apiKeyRateLimit to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-model-rate-limit" => {
            let key = normalize_scoped_key(
                "routePolicyPatch.modelRateLimitKey",
                input.model_rate_limit_key.as_deref(),
                false,
            )?;
            let current_definition = route_policy
                .config
                .model_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                .cloned();
            let next_definition = normalize_rate_limit_patch_definition(
                &format!("modelRateLimit.{key}"),
                input.model_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict("tighten-model-rate-limit 需要显式提供 modelRateLimit。")
            })?;
            ensure_tightened_rate_limit(
                &format!("modelRateLimit.{key}"),
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy
                .config
                .model_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                == Some(&next_definition)
            {
                Vec::new()
            } else {
                vec!["modelRateLimits".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    model_rate_limit_key: Some(key.clone()),
                    model_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set modelRateLimits[{key}] to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        "tighten-endpoint-rate-limit" => {
            let key = normalize_scoped_key(
                "routePolicyPatch.endpointRateLimitKey",
                input.endpoint_rate_limit_key.as_deref(),
                true,
            )?;
            let current_definition = route_policy
                .config
                .endpoint_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                .cloned();
            let next_definition = normalize_rate_limit_patch_definition(
                &format!("endpointRateLimit.{key}"),
                input.endpoint_rate_limit,
                current_definition.clone(),
            )?
            .ok_or_else(|| {
                GatewayError::conflict(
                    "tighten-endpoint-rate-limit 需要显式提供 endpointRateLimit。",
                )
            })?;
            ensure_tightened_rate_limit(
                &format!("endpointRateLimit.{key}"),
                current_definition.as_ref(),
                &next_definition,
            )?;
            let changed_fields = if route_policy
                .config
                .endpoint_rate_limits
                .as_ref()
                .and_then(|items| items.get(&key))
                == Some(&next_definition)
            {
                Vec::new()
            } else {
                vec!["endpointRateLimits".to_string()]
            };
            Ok(RoutePolicyPatchSpec {
                next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    endpoint_rate_limit_key: Some(key.clone()),
                    endpoint_rate_limit: Some(next_definition.clone()),
                    ..Default::default()
                },
                changed_fields,
                summary: format!(
                    "Set endpointRateLimits[{key}] to {}/{}s.",
                    next_definition.max_requests, next_definition.window_seconds
                ),
            })
        }
        other => Err(GatewayError::conflict(format!(
            "当前 remediation actionKey={other} 不支持 route policy 自动执行。"
        ))),
    }
}

fn apply_route_policy_patch(
    route_policy: &GatewayRoutePolicyView,
    patch: &RoutePolicyPatchSpec,
) -> GatewayRoutePolicyView {
    let mut updated = route_policy.clone();
    if let Some(value) = patch.next.provider_max_concurrent_requests {
        updated.config.provider_max_concurrent_requests = Some(value);
    }
    if let Some(value) = patch.next.pre_stream_fallback_enabled {
        updated.config.pre_stream_fallback_enabled = value;
    }
    if let Some(ref value) = patch.next.allowed_provider_account_ids {
        updated.config.allowed_provider_account_ids = Some(value.clone());
    }
    if let Some(ref value) = patch.next.project_rate_limit {
        updated.config.rate_limit_window_seconds = Some(value.window_seconds);
        updated.config.rate_limit_max_requests = Some(value.max_requests);
    }
    if let Some(ref value) = patch.next.api_key_rate_limit {
        updated.config.api_key_rate_limit = Some(value.clone());
    }
    if let (Some(key), Some(value)) = (
        patch.next.model_rate_limit_key.as_ref(),
        patch.next.model_rate_limit.as_ref(),
    ) {
        updated
            .config
            .model_rate_limits
            .get_or_insert_with(HashMap::new)
            .insert(key.clone(), value.clone());
    }
    if let (Some(key), Some(value)) = (
        patch.next.endpoint_rate_limit_key.as_ref(),
        patch.next.endpoint_rate_limit.as_ref(),
    ) {
        updated
            .config
            .endpoint_rate_limits
            .get_or_insert_with(HashMap::new)
            .insert(key.to_ascii_lowercase(), value.clone());
    }
    updated
}

fn has_incident_meaningful_changes(
    before_incident: Option<&GatewayAnalysisAnomalyIncidentView>,
    after_incident: Option<&GatewayAnalysisAnomalyIncidentView>,
) -> bool {
    match (before_incident, after_incident) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(before), Some(after)) => {
            before.owner_user_id != after.owner_user_id
                || before.follow_up_status != after.follow_up_status
                || before.status != after.status
                || before.escalation_status != after.escalation_status
                || before.latest_note != after.latest_note
                || before.resolution_note != after.resolution_note
        }
    }
}

fn has_route_policy_meaningful_changes(
    run: &GatewayAnalysisAnomalyIncidentRemediationRunView,
) -> bool {
    match (&run.before_route_policy, &run.after_route_policy) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(before), Some(after)) => {
            before.enabled != after.enabled
                || before.is_default != after.is_default
                || before.config != after.config
        }
    }
}

fn to_remediation_run_view(
    row: GatewayAnalysisAnomalyRemediationRunRow,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    Ok(GatewayAnalysisAnomalyIncidentRemediationRunView {
        id: row.id,
        incident_id: row.incident_id,
        policy_id: row.policy_id,
        route_policy_id: row.route_policy_id,
        action_key: row.action_key,
        title: row.title,
        execution_mode: row.execution_mode,
        status: row.status,
        dry_run: row.dry_run,
        actor_user_id: row.actor_user_id,
        note: row.note,
        input: row.input.map(|value| value.0),
        result: row.result.map(|value| value.0),
        before_incident: parse_optional_json_field(row.before_incident)?,
        after_incident: parse_optional_json_field(row.after_incident)?,
        before_route_policy: parse_optional_json_field(row.before_route_policy)?,
        after_route_policy: parse_optional_json_field(row.after_route_policy)?,
        error_summary: row.error_summary,
        created_at: format_timestamp(row.created_at),
        completed_at: row.completed_at.map(format_timestamp),
    })
}

async fn get_remediation_run_row_by_id(
    pool: &PgPool,
    run_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationRunRow, GatewayError> {
    let run_id =
        trimmed_owned_ref(run_id).ok_or_else(|| GatewayError::bad_request("runId 不能为空"))?;
    sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        select
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        from gateway_analysis_anomaly_remediation_runs
        where id = $1
        limit 1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Gateway anomaly remediation run 不存在"))
}

async fn get_remediation_run_view_by_id(
    pool: &PgPool,
    run_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    to_remediation_run_view(get_remediation_run_row_by_id(pool, run_id).await?)
}

fn normalize_window_minutes(value: Option<i32>) -> i32 {
    value.unwrap_or(180).clamp(5, 10_080)
}

async fn append_remediation_history(
    pool: &PgPool,
    incident_id: &str,
    actor_user_id: &str,
    action: &GatewayAnalysisAnomalyIncidentRemediationActionView,
    status: &str,
    dry_run: bool,
    note: Option<&str>,
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
    remediation_run_id: &str,
    result: Option<&Value>,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let event_type = if status == "failed" {
        "remediation_failed"
    } else if dry_run {
        "remediation_dry_run"
    } else {
        "remediation_applied"
    };
    let mut metadata = json!({
        "policyId": incident.policy_id,
        "projectId": incident.project_id,
        "routePolicyId": incident.route_policy_id,
        "tag": incident.tag,
        "textMode": incident.text_mode,
        "code": incident.code,
        "severity": incident.severity,
        "status": incident.status,
        "ownerUserId": incident.owner_user_id,
        "followUpStatus": incident.follow_up_status,
        "syncHitCount": incident.sync_hit_count,
        "escalationStatus": incident.escalation_status,
        "escalatedAt": incident.escalated_at,
        "escalationReason": incident.escalation_reason,
        "latestExportId": incident.latest_export_id,
        "previousExportId": incident.previous_export_id,
        "latestValue": incident.latest_value,
        "previousValue": incident.previous_value,
        "deltaValue": incident.delta_value,
        "deltaRatio": incident.delta_ratio,
        "thresholdValue": incident.threshold_value,
        "remediationRunId": remediation_run_id,
        "actionKey": action.action_key,
        "executionMode": action.execution_mode,
        "runStatus": status,
        "dryRun": dry_run,
        "routePolicy": route_policy,
        "result": result
    });
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "routePolicyId".to_string(),
            json!(route_policy
                .map(|item| item.id.clone())
                .or_else(|| incident.route_policy_id.clone())),
        );
    }
    append_incident_history(
        pool,
        incident_id,
        event_type,
        Some(actor_user_id),
        note.or(Some(action.title.as_str())),
        Some(metadata),
        timestamp,
    )
    .await
}

async fn append_incident_history(
    pool: &PgPool,
    incident_id: &str,
    event_type: &str,
    actor_user_id: Option<&str>,
    note: Option<&str>,
    metadata: Option<Value>,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incident_history (
          id,
          incident_id,
          event_type,
          actor_user_id,
          note,
          metadata,
          created_at
        ) values ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(incident_id)
    .bind(event_type)
    .bind(actor_user_id)
    .bind(note)
    .bind(metadata.map(sqlx::types::Json))
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

fn route_policy_view_from_row(
    row: GatewayRoutePolicyRow,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    let config =
        serde_json::from_value::<GatewayRoutePolicyConfig>(row.config.0).map_err(|error| {
            GatewayError::server_error(format!("parse route policy config: {error}"))
        })?;
    Ok(GatewayRoutePolicyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        is_default: row.is_default,
        enabled: row.enabled,
        config,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

async fn find_route_policy_by_id(
    pool: &PgPool,
    route_policy_id: &str,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let Some(route_policy_id) = trimmed_owned_ref(route_policy_id) else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, GatewayRoutePolicyRow>(
        r#"
        select id, project_id, name, is_default, enabled, config, created_at, updated_at
        from gateway_route_policies
        where id = $1
        limit 1
        "#,
    )
    .bind(route_policy_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    row.map(route_policy_view_from_row).transpose()
}

async fn get_incident_by_id(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = trimmed_owned_ref(incident_id)
        .ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))?;
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            incident_id: Some(incident_id.to_string()),
            limit: Some(1),
            ..GatewayAnalysisAnomalyIncidentFilters::default()
        },
    )
    .await?;
    incidents
        .into_iter()
        .next()
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly incident 不存在"))
}

async fn find_latest_anomaly_remediation_run(
    pool: &PgPool,
    incident_id: &str,
    action_key: &str,
) -> Result<Option<GatewayAnalysisAnomalyIncidentRemediationRunView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        select
          id,
          incident_id,
          policy_id,
          route_policy_id,
          action_key,
          title,
          execution_mode,
          status,
          dry_run,
          actor_user_id,
          note,
          input,
          result,
          before_incident,
          after_incident,
          before_route_policy,
          after_route_policy,
          error_summary,
          created_at,
          completed_at
        from gateway_analysis_anomaly_remediation_runs
        where incident_id = $1 and action_key = $2
        order by created_at desc
        limit 1
        "#,
    )
    .bind(incident_id)
    .bind(action_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    row.map(to_remediation_run_view).transpose()
}

async fn count_anomaly_remediation_runs_by_status(
    pool: &PgPool,
    incident_id: &str,
    action_key: &str,
    status: &str,
) -> Result<i32, GatewayError> {
    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from gateway_analysis_anomaly_remediation_runs
        where incident_id = $1 and action_key = $2 and status = $3
        "#,
    )
    .bind(incident_id)
    .bind(action_key)
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;
    Ok(count.clamp(0, i64::from(i32::MAX)) as i32)
}

async fn read_route_policy_health_degraded(
    pool: &PgPool,
    redis_pool: &RedisPool,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> Result<bool, GatewayError> {
    let provider_ids = route_policy
        .and_then(|item| item.config.allowed_provider_account_ids.as_ref())
        .cloned()
        .unwrap_or_default();
    if provider_ids.is_empty() {
        return Ok(false);
    }

    #[derive(Debug, Clone, FromRow)]
    struct ProviderStatusRow {
        status: String,
    }

    let provider_rows = sqlx::query_as::<_, ProviderStatusRow>(
        r#"
        select status
        from gateway_provider_accounts
        where id = any($1)
        "#,
    )
    .bind(&provider_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    if provider_rows.iter().any(|row| row.status != "active") {
        return Ok(true);
    }

    if let Ok(mut connection) = redis_pool.get().await {
        for provider_id in &provider_ids {
            if connection
                .exists::<_, bool>(provider_breaker_open_key(provider_id))
                .await
                .unwrap_or(false)
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn resolve_anomaly_policy_auto_remediation_config(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> AutoRemediationConfig {
    AutoRemediationConfig {
        auto_remediation_enabled: policy.auto_remediation_enabled,
        auto_remediation_interval_minutes: policy.auto_remediation_interval_minutes.unwrap_or(180),
        auto_remediation_dry_run_first: policy.auto_remediation_dry_run_first,
        auto_remediation_action_keys: policy.auto_remediation_action_keys.clone(),
        auto_remediation_max_apply_runs_per_incident: policy
            .auto_remediation_max_apply_runs_per_incident,
        auto_remediation_require_alert_before_apply: policy
            .auto_remediation_require_alert_before_apply,
        auto_remediation_freeze_on_provider_health_degrade: policy
            .auto_remediation_freeze_on_provider_health_degrade,
    }
}

fn resolve_rate_limit_hotspot_auto_remediation_config(
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    let disabled = || AutoRemediationConfig {
        auto_remediation_enabled: false,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(Vec::new()),
        auto_remediation_max_apply_runs_per_incident: None,
        auto_remediation_require_alert_before_apply: false,
        auto_remediation_freeze_on_provider_health_degrade: true,
    };
    if incident.route_policy_id.is_none() {
        return disabled();
    }

    if let Some(configured_profile) = route_policy
        .and_then(|item| item.config.rate_limit_hotspot_auto_remediation.as_ref())
        .and_then(|value| {
            serde_json::from_value::<GatewayRoutePolicyRateLimitHotspotAutoRemediationProfile>(
                value.clone(),
            )
            .ok()
        })
    {
        let action_key = configured_profile
            .action_by_code
            .as_ref()
            .and_then(|items| items.get(&incident.code))
            .cloned()
            .flatten();
        return AutoRemediationConfig {
            auto_remediation_enabled: configured_profile.enabled.unwrap_or(true),
            auto_remediation_interval_minutes: configured_profile.interval_minutes.unwrap_or(180),
            auto_remediation_dry_run_first: configured_profile.dry_run_first.unwrap_or(true),
            auto_remediation_action_keys: Some(action_key.into_iter().collect()),
            auto_remediation_max_apply_runs_per_incident: configured_profile
                .max_apply_runs_per_incident,
            auto_remediation_require_alert_before_apply: configured_profile
                .require_alert_before_apply
                .unwrap_or(true),
            auto_remediation_freeze_on_provider_health_degrade: configured_profile
                .freeze_on_provider_health_degrade
                .unwrap_or(true),
        };
    }

    let action_keys = match incident.code.as_str() {
        "rate_limit_request_spike"
        | "rate_limit_code_concentration"
        | "rate_limit_project_hotspot" => vec!["tighten-project-rate-limit".to_string()],
        "rate_limit_api_key_hotspot" => vec!["tighten-api-key-rate-limit".to_string()],
        "rate_limit_model_hotspot" => vec!["tighten-model-rate-limit".to_string()],
        "rate_limit_endpoint_hotspot" => vec!["tighten-endpoint-rate-limit".to_string()],
        _ => Vec::new(),
    };
    if action_keys.is_empty() {
        return disabled();
    }

    AutoRemediationConfig {
        auto_remediation_enabled: true,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(action_keys),
        auto_remediation_max_apply_runs_per_incident: Some(1),
        auto_remediation_require_alert_before_apply: true,
        auto_remediation_freeze_on_provider_health_degrade: true,
    }
}

fn resolve_routing_anomaly_auto_remediation_config(
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    let disabled = || AutoRemediationConfig {
        auto_remediation_enabled: false,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(Vec::new()),
        auto_remediation_max_apply_runs_per_incident: None,
        auto_remediation_require_alert_before_apply: false,
        auto_remediation_freeze_on_provider_health_degrade: true,
    };
    let Some(route_policy) = route_policy else {
        return disabled();
    };
    if incident.route_policy_id.is_none() {
        return disabled();
    }

    if let Some(configured_profile) = route_policy
        .config
        .routing_anomaly_auto_remediation
        .as_ref()
        .and_then(|value| {
            serde_json::from_value::<GatewayRoutePolicyRoutingAnomalyAutoRemediationProfile>(
                value.clone(),
            )
            .ok()
        })
    {
        let action_keys = configured_profile
            .action_keys_by_code
            .as_ref()
            .and_then(|items| items.get(&incident.code))
            .cloned()
            .unwrap_or_default();
        return AutoRemediationConfig {
            auto_remediation_enabled: configured_profile.enabled.unwrap_or(true),
            auto_remediation_interval_minutes: configured_profile.interval_minutes.unwrap_or(180),
            auto_remediation_dry_run_first: configured_profile.dry_run_first.unwrap_or(true),
            auto_remediation_action_keys: Some(action_keys),
            auto_remediation_max_apply_runs_per_incident: configured_profile
                .max_apply_runs_per_incident,
            auto_remediation_require_alert_before_apply: configured_profile
                .require_alert_before_apply
                .unwrap_or(true),
            auto_remediation_freeze_on_provider_health_degrade: configured_profile
                .freeze_on_provider_health_degrade
                .unwrap_or(true),
        };
    }

    let allow_provider_isolation = route_policy
        .config
        .allowed_provider_account_ids
        .as_ref()
        .map(|items| items.len() > 1)
        .unwrap_or(false);
    let action_keys = match incident.code.as_str() {
        "saturated_provider_route_spike" => vec!["reduce-provider-concurrency".to_string()],
        "breaker_open_provider_route_detected" => {
            let mut keys = Vec::new();
            if allow_provider_isolation {
                keys.push("provider-isolation".to_string());
            }
            keys.push("disable-prestream-fallback".to_string());
            keys
        }
        "failure_rate_spike"
        | "completion_rate_drop"
        | "provider_routing_score_drop"
        | "degraded_provider_route_spike" => {
            let mut keys = vec![
                "disable-prestream-fallback".to_string(),
                "reduce-provider-concurrency".to_string(),
            ];
            if allow_provider_isolation {
                keys.push("provider-isolation".to_string());
            }
            keys
        }
        _ => Vec::new(),
    };

    AutoRemediationConfig {
        auto_remediation_enabled: !action_keys.is_empty(),
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(action_keys),
        auto_remediation_max_apply_runs_per_incident: Some(1),
        auto_remediation_require_alert_before_apply: true,
        auto_remediation_freeze_on_provider_health_degrade: true,
    }
}

fn resolve_incident_auto_remediation_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    if let Some(policy) = policy {
        return resolve_anomaly_policy_auto_remediation_config(policy);
    }
    let hotspot = resolve_rate_limit_hotspot_auto_remediation_config(incident, route_policy);
    if hotspot.auto_remediation_enabled {
        return hotspot;
    }
    resolve_routing_anomaly_auto_remediation_config(incident, route_policy)
}

fn resolve_anomaly_policy_alert_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
) -> AlertConfig {
    AlertConfig {
        alerting_enabled: policy.map(|item| item.alerting_enabled).unwrap_or(true),
        alert_interval_minutes: policy
            .and_then(|item| item.alert_interval_minutes)
            .unwrap_or(DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES),
        notify_operators: policy
            .map(|item| item.notify_operators_on_escalation)
            .unwrap_or(true),
        notify_owner: policy
            .map(|item| item.notify_owner_on_escalation)
            .unwrap_or(true),
    }
}

fn resolve_anomaly_incident_alert_schedule(
    status: &str,
    escalation_status: &str,
    config: &AlertConfig,
    last_alert_attempt_at: Option<&str>,
    now: OffsetDateTime,
) -> AlertSchedule {
    let normalized_status = trimmed_owned_ref_opt(Some(status))
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "open".to_string());
    let normalized_escalation_status = trimmed_owned_ref_opt(Some(escalation_status))
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "none".to_string());
    if !config.alerting_enabled
        || normalized_escalation_status != "escalated"
        || !matches!(normalized_status.as_str(), "open" | "acknowledged")
    {
        return AlertSchedule {
            next_alert_due_at: None,
            alert_due: false,
        };
    }

    let next_alert_due_at = last_alert_attempt_at
        .and_then(trimmed_owned_ref)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        .map(|value| value + time::Duration::minutes(i64::from(config.alert_interval_minutes)));

    if let Some(next_due_at) = next_alert_due_at {
        return AlertSchedule {
            next_alert_due_at: Some(format_timestamp(next_due_at)),
            alert_due: next_due_at <= now,
        };
    }

    AlertSchedule {
        next_alert_due_at: None,
        alert_due: true,
    }
}

fn resolve_anomaly_alert_delivery_profile(severity: &str) -> AlertDeliveryProfile {
    if trimmed_owned_ref_opt(Some(severity))
        .is_some_and(|value| value.eq_ignore_ascii_case("critical"))
    {
        return AlertDeliveryProfile {
            alert_level: 3,
            webhook_severity: "danger".to_string(),
        };
    }

    AlertDeliveryProfile {
        alert_level: 2,
        webhook_severity: "warning".to_string(),
    }
}

fn resolve_anomaly_remediation_schedule(
    incident: &GatewayAnalysisAnomalyIncidentView,
    config: &AutoRemediationConfig,
    action_enabled: bool,
    applied_run_count: i32,
    provider_health_degraded: bool,
    latest_run: Option<&GatewayAnalysisAnomalyIncidentRemediationRunView>,
    now: OffsetDateTime,
) -> AutoRemediationSchedule {
    if incident.status == "resolved"
        || incident.escalation_status != "escalated"
        || !config.auto_remediation_enabled
        || !action_enabled
    {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: if !config.auto_remediation_enabled {
                Some("policy_disabled".to_string())
            } else if !action_enabled {
                Some("action_not_allowed".to_string())
            } else {
                None
            },
        };
    }

    let latest_reference = latest_run
        .and_then(|item| item.completed_at.as_ref())
        .or_else(|| latest_run.map(|item| &item.created_at))
        .cloned();
    if latest_reference.is_none() {
        let next_execution_status = if config.auto_remediation_dry_run_first {
            "dry_run".to_string()
        } else {
            "applied".to_string()
        };
        return build_schedule_without_reference(
            &next_execution_status,
            config,
            incident.last_alerted_at.as_deref(),
            applied_run_count,
            provider_health_degraded,
        );
    }

    let latest_run = latest_run.expect("reference implies latest run exists");
    if latest_run.status == "failed" {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: Some("previous_failure".to_string()),
        };
    }
    if latest_run.status == "applied" {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: Some("already_applied".to_string()),
        };
    }

    let reference_time =
        match OffsetDateTime::parse(latest_reference.as_deref().unwrap_or_default(), &Rfc3339) {
            Ok(value) => value,
            Err(_) => {
                return AutoRemediationSchedule {
                    remediation_due: false,
                    next_execution_status: None,
                    next_run_due_at: None,
                    blocked_reason: Some("invalid_reference_time".to_string()),
                }
            }
        };
    let next_run_due_at = reference_time
        + time::Duration::minutes(i64::from(config.auto_remediation_interval_minutes));
    let next_execution_status =
        if latest_run.status == "dry_run" && config.auto_remediation_dry_run_first {
            "applied".to_string()
        } else if config.auto_remediation_dry_run_first && latest_run.dry_run {
            "applied".to_string()
        } else {
            "dry_run".to_string()
        };

    if let Some(blocked_reason) = remediation_blocked_reason(
        &next_execution_status,
        config,
        incident.last_alerted_at.as_deref(),
        applied_run_count,
        provider_health_degraded,
    ) {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: Some(next_execution_status),
            next_run_due_at: Some(format_timestamp(next_run_due_at)),
            blocked_reason: Some(blocked_reason),
        };
    }

    AutoRemediationSchedule {
        remediation_due: next_run_due_at <= now,
        next_execution_status: Some(next_execution_status),
        next_run_due_at: Some(format_timestamp(next_run_due_at)),
        blocked_reason: None,
    }
}

fn build_schedule_without_reference(
    next_execution_status: &str,
    config: &AutoRemediationConfig,
    last_alerted_at: Option<&str>,
    applied_run_count: i32,
    provider_health_degraded: bool,
) -> AutoRemediationSchedule {
    if let Some(blocked_reason) = remediation_blocked_reason(
        next_execution_status,
        config,
        last_alerted_at,
        applied_run_count,
        provider_health_degraded,
    ) {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: Some(next_execution_status.to_string()),
            next_run_due_at: None,
            blocked_reason: Some(blocked_reason),
        };
    }
    AutoRemediationSchedule {
        remediation_due: true,
        next_execution_status: Some(next_execution_status.to_string()),
        next_run_due_at: None,
        blocked_reason: None,
    }
}

fn remediation_blocked_reason(
    next_execution_status: &str,
    config: &AutoRemediationConfig,
    last_alerted_at: Option<&str>,
    applied_run_count: i32,
    provider_health_degraded: bool,
) -> Option<String> {
    if next_execution_status == "applied"
        && config.auto_remediation_require_alert_before_apply
        && trimmed_owned_ref_opt(last_alerted_at).is_none()
    {
        return Some("alert_pending".to_string());
    }
    if next_execution_status == "applied"
        && config
            .auto_remediation_max_apply_runs_per_incident
            .is_some_and(|cap| applied_run_count >= cap)
    {
        return Some("apply_cap_reached".to_string());
    }
    if next_execution_status == "applied"
        && config.auto_remediation_freeze_on_provider_health_degrade
        && provider_health_degraded
    {
        return Some("provider_health_degraded".to_string());
    }
    None
}

fn parse_optional_json_field<T: DeserializeOwned>(
    value: Option<Json<Value>>,
) -> Result<Option<T>, GatewayError> {
    value
        .map(|value| {
            serde_json::from_value::<T>(value.0).map_err(|error| {
                GatewayError::server_error(format!("parse remediation snapshot: {error}"))
            })
        })
        .transpose()
}

fn remediation_execution_input_from_action(
    action: &GatewayAnalysisAnomalyIncidentRemediationActionView,
    status: &str,
) -> Result<ExecuteGatewayAnalysisAnomalyIncidentRemediationInput, GatewayError> {
    let mut input = action
        .default_execution_input
        .clone()
        .map(|value| {
            serde_json::from_value::<ExecuteGatewayAnalysisAnomalyIncidentRemediationInput>(value)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "parse remediation default execution input: {error}"
                    ))
                })
        })
        .transpose()?
        .unwrap_or_else(|| ExecuteGatewayAnalysisAnomalyIncidentRemediationInput {
            action_key: action.action_key.clone(),
            ..ExecuteGatewayAnalysisAnomalyIncidentRemediationInput::default()
        });
    input.action_key = action.action_key.clone();
    input.dry_run = Some(status == "dry_run");
    Ok(input)
}

fn parse_filter_timestamp(
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

fn push_action(
    actions: &mut Vec<GatewayAnalysisAnomalyIncidentRemediationActionView>,
    action: GatewayAnalysisAnomalyIncidentRemediationActionView,
) {
    if !actions
        .iter()
        .any(|item| item.action_key == action.action_key)
    {
        actions.push(action);
    }
}

fn push_optional_filter(
    builder: &mut sqlx::QueryBuilder<'_, sqlx::Postgres>,
    column: &str,
    value: Option<&str>,
) {
    if let Some(value) = trimmed_owned(value) {
        builder
            .push(" and ")
            .push(column)
            .push(" = ")
            .push_bind(value);
    }
}

fn accumulate_key_bucket(map: &mut BTreeMap<String, usize>, value: Option<&str>) {
    let Some(key) = trimmed_owned_ref_opt(value) else {
        return;
    };
    *map.entry(key.to_string()).or_insert(0) += 1;
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

fn trimmed_owned_ref_opt<'a>(value: Option<&'a str>) -> Option<&'a str> {
    value.and_then(trimmed_owned_ref)
}

enum SupportedPolicySyncKind {
    ProviderRouting,
    RateLimitHotspot,
    AnalysisExport,
    Unsupported(String),
}

enum PolicySyncResult {
    ProviderRouting(GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult),
    RateLimitHotspot(GatewaySyncRateLimitHotspotAnomalyIncidentsResult),
    AnalysisExport(GatewaySyncAnalysisExportAnomalyIncidentsResult),
}

impl PolicySyncResult {
    fn kind(&self) -> &'static str {
        match self {
            Self::ProviderRouting(_) => "provider_routing",
            Self::RateLimitHotspot(_) => "rate_limit_hotspot",
            Self::AnalysisExport(_) => "analysis_export",
        }
    }

    fn anomaly_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.report.anomalies.len(),
            Self::RateLimitHotspot(result) => result.snapshot.report.anomalies.len(),
            Self::AnalysisExport(result) => result.report.anomalies.len(),
        }
    }

    fn opened_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.opened_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.opened_incident_ids.len(),
            Self::AnalysisExport(result) => result.opened_incident_ids.len(),
        }
    }

    fn updated_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.updated_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.updated_incident_ids.len(),
            Self::AnalysisExport(result) => result.updated_incident_ids.len(),
        }
    }

    fn resolved_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.resolved_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.resolved_incident_ids.len(),
            Self::AnalysisExport(result) => result.resolved_incident_ids.len(),
        }
    }

    fn into_value(self) -> Result<Value, GatewayError> {
        serde_json::to_value(self).map_err(|error| {
            GatewayError::server_error(format!("serialize anomaly policy sync result: {error}"))
        })
    }
}

impl Serialize for PolicySyncResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::ProviderRouting(result) => result.serialize(serializer),
            Self::RateLimitHotspot(result) => result.serialize(serializer),
            Self::AnalysisExport(result) => result.serialize(serializer),
        }
    }
}

fn determine_supported_policy_sync_kind(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> SupportedPolicySyncKind {
    match determine_supported_sync_kind_from_tag(policy.tag.as_deref()) {
        SupportedPolicySyncKind::Unsupported(_) => SupportedPolicySyncKind::AnalysisExport,
        supported => supported,
    }
}

fn determine_supported_sync_kind_from_tag(tag: Option<&str>) -> SupportedPolicySyncKind {
    let Some(tag) = trimmed_owned_ref_opt(tag) else {
        return SupportedPolicySyncKind::Unsupported(
            "当前 anomaly policy 缺少可识别 tag，Rust 还无法判断应同步哪类异常源。".to_string(),
        );
    };
    if tag.starts_with("provider-routing:") {
        return SupportedPolicySyncKind::ProviderRouting;
    }
    if tag.starts_with("rate-limit-hotspot:") {
        return SupportedPolicySyncKind::RateLimitHotspot;
    }
    if tag.starts_with("analysis-export:") {
        return SupportedPolicySyncKind::AnalysisExport;
    }
    SupportedPolicySyncKind::Unsupported(format!(
        "当前 Rust 仅支持 provider-routing / rate-limit-hotspot / analysis-export anomaly sync，收到 tag: {tag}"
    ))
}

fn determine_supported_ad_hoc_sync_kind(
    tag: Option<&str>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
) -> SupportedPolicySyncKind {
    match determine_supported_sync_kind_from_tag(tag) {
        SupportedPolicySyncKind::Unsupported(_) => {
            if has_request_audit_specific_sync_hints(input) {
                SupportedPolicySyncKind::Unsupported(
                    "当前 ad-hoc anomaly sync 若使用 request-audit 维度，必须显式提供 provider-routing 或 rate-limit-hotspot tag。".to_string(),
                )
            } else {
                SupportedPolicySyncKind::AnalysisExport
            }
        }
        supported => supported,
    }
}

fn build_policy_request_audit_filters(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> RequestAuditFilters {
    let mut filters = RequestAuditFilters {
        project_id: policy.project_id.clone(),
        route_policy_id: policy.route_policy_id.clone(),
        provider_account_id: None,
        session_id: None,
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: None,
        protocol_family: None,
        status: None,
        endpoint_kind: None,
        stream: None,
        error_code: None,
        fallback_eligible: None,
        artifact_available: None,
        created_from: None,
        created_to: None,
        limit: Some(200),
    };
    if let Some(tag) = trimmed_owned_ref_opt(policy.tag.as_deref()) {
        let (_, parts) = split_policy_tag_parts(tag);
        for (key, value) in parts {
            match key.as_str() {
                "provider" => filters.provider_account_id = Some(value),
                "protocol" => filters.protocol_family = Some(value),
                "endpoint" => filters.endpoint_kind = Some(value),
                "api-key" => filters.api_key_id = Some(value),
                "session" => filters.session_id = Some(value),
                "response" => filters.response_id = Some(value),
                "status" => filters.status = Some(value),
                _ => {}
            }
        }
    }
    filters
}

fn build_policy_analysis_export_filters(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        label: None,
        tag: trimmed_owned(policy.tag.as_deref()),
        project_id: trimmed_owned(policy.project_id.as_deref()),
        status: Some("active".to_string()),
        text_mode: trimmed_owned(policy.text_mode.as_deref()),
        created_from: None,
        created_to: None,
        limit: None,
        ..GatewayPersistedAnalysisExportFilters::default()
    }
}

fn build_ad_hoc_analysis_export_filters(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
    effective_tag: Option<&str>,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        label: trimmed_owned(input.label.as_deref()),
        tag: trimmed_owned(input.tag.as_deref())
            .or_else(|| {
                effective_tag
                    .and_then(trimmed_owned_ref)
                    .map(str::to_string)
            })
            .or_else(|| policy.and_then(|item| trimmed_owned(item.tag.as_deref()))),
        project_id: trimmed_owned(input.project_id.as_deref())
            .or_else(|| policy.and_then(|item| trimmed_owned(item.project_id.as_deref()))),
        status: trimmed_owned(input.status.as_deref()).or_else(|| Some("active".to_string())),
        text_mode: trimmed_owned(input.text_mode.as_deref())
            .or_else(|| policy.and_then(|item| trimmed_owned(item.text_mode.as_deref()))),
        created_from: trimmed_owned(input.created_from.as_deref()),
        created_to: trimmed_owned(input.created_to.as_deref()),
        limit: input.limit,
        ..GatewayPersistedAnalysisExportFilters::default()
    }
}

fn build_analysis_export_auto_escalation_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
) -> GatewayAnalysisExportAutoEscalationConfig {
    let Some(policy) = policy else {
        return GatewayAnalysisExportAutoEscalationConfig::default();
    };
    GatewayAnalysisExportAutoEscalationConfig {
        enabled: policy.auto_escalate_enabled,
        severity_threshold: trimmed_owned(policy.escalate_severity_threshold.as_deref()),
        after_sync_count: policy.escalate_after_sync_count,
        owner_user_id: trimmed_owned(policy.auto_escalate_owner_user_id.as_deref()),
        follow_up_status: trimmed_owned(policy.auto_escalate_follow_up_status.as_deref()),
    }
}

fn has_request_audit_specific_sync_hints(input: &GatewayAnalysisAnomalyIncidentSyncInput) -> bool {
    input.provider_account_id.is_some()
        || input.session_id.is_some()
        || input.api_key_id.is_some()
        || input.user_credential_id.is_some()
        || input.response_id.is_some()
        || input.protocol_family.is_some()
        || input.endpoint_kind.is_some()
        || input.stream.is_some()
        || input.error_code.is_some()
        || input.fallback_eligible.is_some()
        || input.artifact_available.is_some()
}

fn build_ad_hoc_request_audit_filters(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
    effective_tag: Option<&str>,
) -> RequestAuditFilters {
    let mut filters = policy
        .map(build_policy_request_audit_filters)
        .unwrap_or_else(|| RequestAuditFilters {
            project_id: None,
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            user_credential_id: None,
            access_key_id: None,
            source_access_key_id: None,
            response_id: None,
            protocol_family: None,
            status: None,
            endpoint_kind: None,
            stream: None,
            error_code: None,
            fallback_eligible: None,
            artifact_available: None,
            created_from: None,
            created_to: None,
            limit: Some(200),
        });

    if policy.is_none() {
        if let Some(tag) = trimmed_owned_ref_opt(effective_tag) {
            let (_, parts) = split_policy_tag_parts(tag);
            for (key, value) in parts {
                match key.as_str() {
                    "provider" => filters.provider_account_id = Some(value),
                    "protocol" => filters.protocol_family = Some(value),
                    "endpoint" => filters.endpoint_kind = Some(value),
                    "api-key" => filters.api_key_id = Some(value),
                    "session" => filters.session_id = Some(value),
                    "response" => filters.response_id = Some(value),
                    "status" => filters.status = Some(value),
                    _ => {}
                }
            }
        }
    }

    if input.project_id.is_some() {
        filters.project_id = trimmed_owned(input.project_id.as_deref());
    }
    if input.route_policy_id.is_some() {
        filters.route_policy_id = trimmed_owned(input.route_policy_id.as_deref());
    }
    if input.provider_account_id.is_some() {
        filters.provider_account_id = trimmed_owned(input.provider_account_id.as_deref());
    }
    if input.session_id.is_some() {
        filters.session_id = trimmed_owned(input.session_id.as_deref());
    }
    if input.api_key_id.is_some() {
        filters.api_key_id = trimmed_owned(input.api_key_id.as_deref());
    }
    if input.user_credential_id.is_some() {
        filters.user_credential_id = trimmed_owned(input.user_credential_id.as_deref());
    }
    if input.response_id.is_some() {
        filters.response_id = trimmed_owned(input.response_id.as_deref());
    }
    if input.protocol_family.is_some() {
        filters.protocol_family = trimmed_owned(input.protocol_family.as_deref());
    }
    if input.status.is_some() {
        filters.status = trimmed_owned(input.status.as_deref());
    }
    if input.endpoint_kind.is_some() {
        filters.endpoint_kind = trimmed_owned(input.endpoint_kind.as_deref());
    }
    if input.stream.is_some() {
        filters.stream = input.stream;
    }
    if input.error_code.is_some() {
        filters.error_code = trimmed_owned(input.error_code.as_deref());
    }
    if input.fallback_eligible.is_some() {
        filters.fallback_eligible = input.fallback_eligible;
    }
    if input.artifact_available.is_some() {
        filters.artifact_available = input.artifact_available;
    }
    if input.created_from.is_some() {
        filters.created_from = trimmed_owned(input.created_from.as_deref());
    }
    if input.created_to.is_some() {
        filters.created_to = trimmed_owned(input.created_to.as_deref());
    }
    if input.limit.is_some() {
        filters.limit = input.limit;
    }

    filters
}

fn split_policy_tag_parts(tag: &str) -> (Option<String>, Vec<(String, String)>) {
    let segments = tag
        .split(':')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_string())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return (None, Vec::new());
    }
    let profile_key = Some(segments[1].clone());
    let mut pairs = Vec::new();
    let mut index = 2usize;
    while index + 1 < segments.len() {
        pairs.push((segments[index].clone(), segments[index + 1].clone()));
        index += 2;
    }
    (profile_key, pairs)
}

async fn update_anomaly_policy_sync_state(
    pool: &PgPool,
    policy_id: &str,
    status: &str,
    synced_at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), GatewayError> {
    let policy_id = trimmed_owned_ref(policy_id)
        .ok_or_else(|| GatewayError::bad_request("policyId 不能为空"))?;
    let sync_status = normalize_anomaly_policy_sync_status(Some(status))
        .ok_or_else(|| GatewayError::bad_request("lastSyncStatus 不合法"))?;
    let error = error
        .and_then(trimmed_owned_ref)
        .map(|value| truncate_error_summary(value, 2_000));
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_policies
        set
          last_synced_at = $2,
          last_sync_status = $3,
          last_sync_error = $4,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(policy_id)
    .bind(synced_at)
    .bind(sync_status)
    .bind(error.as_deref())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

fn truncate_error_summary(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut count = 0usize;
    for ch in value.chars() {
        if count == max_chars {
            break;
        }
        result.push(ch);
        count += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        build_ad_hoc_analysis_export_filters, build_ad_hoc_request_audit_filters,
        build_analysis_anomaly_threshold_config, build_incident_remediation_plan,
        build_metric_delta, build_remediation_effectiveness_summary, build_remediation_run_summary,
        build_route_policy_patch, determine_supported_ad_hoc_sync_kind,
        determine_supported_sync_kind_from_tag,
        matches_remediation_effectiveness_anomaly_snapshot_filters,
        resolve_anomaly_alert_delivery_profile, resolve_anomaly_incident_alert_schedule,
        resolve_anomaly_policy_schedule, resolve_anomaly_remediation_schedule,
        resolve_rate_limit_hotspot_auto_remediation_config, AlertConfig, AutoRemediationConfig,
        GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput,
        GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayAnalysisAnomalyIncidentSyncInput,
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView,
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView,
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
        GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
        GatewayAnalysisAnomalyRemediationImpactWindowView,
        GatewayAnalysisAnomalyRemediationRunImpactMetricsView,
        GatewayAnalysisAnomalyRemediationRunImpactView, IncidentSyncContext,
        DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES,
    };
    use crate::db::{
        request_audits::GatewayAnalysisMetricDistributionView, GatewayAnalysisAnomalyIncidentView,
        GatewayAnalysisSummaryView, GatewayRateLimitDefinition, GatewayRoutePolicyConfig,
        GatewayRoutePolicyView,
    };
    use serde_json::Value;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    fn base_incident(code: &str) -> GatewayAnalysisAnomalyIncidentView {
        GatewayAnalysisAnomalyIncidentView {
            id: "incident-1".to_string(),
            policy_id: None,
            fingerprint: "fingerprint".to_string(),
            project_id: Some("project-1".to_string()),
            route_policy_id: Some("route-1".to_string()),
            tag: Some("provider-routing:balanced".to_string()),
            text_mode: None,
            code: code.to_string(),
            severity: "critical".to_string(),
            status: "open".to_string(),
            owner_user_id: None,
            follow_up_status: "pending".to_string(),
            sync_hit_count: 1,
            escalation_status: "escalated".to_string(),
            escalated_at: Some("2026-04-13T00:00:00Z".to_string()),
            escalation_reason: Some("reason".to_string()),
            latest_note: None,
            resolution_note: None,
            last_action_at: None,
            last_alert_attempt_at: None,
            last_alerted_at: None,
            last_alert_severity: None,
            alert_delivery_count: 0,
            summary: "summary".to_string(),
            latest_export_id: None,
            previous_export_id: None,
            latest_value: Some(1.0),
            previous_value: Some(0.5),
            delta_value: Some(0.5),
            delta_ratio: Some(1.0),
            threshold_value: Some(0.4),
            first_seen_at: "2026-04-13T00:00:00Z".to_string(),
            last_seen_at: "2026-04-13T00:00:00Z".to_string(),
            acknowledged_at: None,
            resolved_at: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            updated_at: "2026-04-13T00:00:00Z".to_string(),
        }
    }

    fn base_route_policy() -> GatewayRoutePolicyView {
        GatewayRoutePolicyView {
            id: "route-1".to_string(),
            project_id: "project-1".to_string(),
            name: "Default".to_string(),
            is_default: true,
            enabled: true,
            config: GatewayRoutePolicyConfig {
                provider_max_concurrent_requests: Some(3),
                allowed_provider_account_ids: Some(vec![
                    "provider-a".to_string(),
                    "provider-b".to_string(),
                ]),
                ..GatewayRoutePolicyConfig::default()
            },
            created_at: "2026-04-13T00:00:00Z".to_string(),
            updated_at: "2026-04-13T00:00:00Z".to_string(),
        }
    }

    fn empty_summary() -> GatewayAnalysisSummaryView {
        GatewayAnalysisSummaryView {
            total_samples: 0,
            completed_samples: 0,
            failed_samples: 0,
            cancelled_samples: 0,
            stream_samples: 0,
            tool_request_samples: 0,
            tool_response_samples: 0,
            system_prompt_samples: 0,
            reasoning_samples: 0,
            metadata_samples: 0,
            explicit_session_samples: 0,
            previous_response_samples: 0,
            request_artifact_samples: 0,
            response_artifact_samples: 0,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_tokens: 0,
            total_cache_creation_input_tokens: 0,
            total_cache_read_input_tokens: 0,
            request_text_chars: GatewayAnalysisMetricDistributionView {
                avg: None,
                p50: None,
                p95: None,
            },
            response_text_chars: GatewayAnalysisMetricDistributionView {
                avg: None,
                p50: None,
                p95: None,
            },
            first_token_latency_ms: GatewayAnalysisMetricDistributionView {
                avg: None,
                p50: None,
                p95: None,
            },
            stream_chunk_count: GatewayAnalysisMetricDistributionView {
                avg: None,
                p50: None,
                p95: None,
            },
            by_protocol_family: Vec::new(),
            by_endpoint_kind: Vec::new(),
            by_resolved_model: Vec::new(),
            by_provider_account: Vec::new(),
            by_status: Vec::new(),
        }
    }

    fn empty_anomaly_snapshot() -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView
    {
        GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
            snapshot_id: "snapshot-1".to_string(),
            label: Some("Latency Review".to_string()),
            created_at: "2026-04-13T03:00:00Z".to_string(),
            object_key:
                "ai-gateway/remediation-effectiveness-anomaly-snapshots/snapshot-1/snapshot.json"
                    .to_string(),
            filters: GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView {
                label: Some("Latency Review".to_string()),
                route_policy_id: Some("route-1".to_string()),
                action_key: Some("provider-isolation".to_string()),
                created_from: Some("2026-04-13T00:00:00Z".to_string()),
                created_to: None,
                limit: 10,
                lookback_hours: Some(24),
                profile_key: "balanced".to_string(),
            },
            report: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
                generated_at: "2026-04-13T03:00:00Z".to_string(),
                filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView {
                    label: Some("Latency Review".to_string()),
                    route_policy_id: Some("route-1".to_string()),
                    action_key: Some("provider-isolation".to_string()),
                    created_from: Some("2026-04-13T00:00:00Z".to_string()),
                    created_to: None,
                },
                profile_key: "balanced".to_string(),
                thresholds: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
                    impacted_run_rate_warning_threshold: 0.65,
                    impacted_run_rate_critical_threshold: 0.45,
                    unavailable_run_rate_warning_threshold: 0.25,
                    unavailable_run_rate_critical_threshold: 0.4,
                    completion_rate_regressed_warning_threshold: 0.25,
                    completion_rate_regressed_critical_threshold: 0.4,
                    failure_rate_regressed_warning_threshold: 0.25,
                    failure_rate_regressed_critical_threshold: 0.4,
                    request_artifact_regressed_warning_threshold: 0.25,
                    request_artifact_regressed_critical_threshold: 0.4,
                    response_artifact_regressed_warning_threshold: 0.25,
                    response_artifact_regressed_critical_threshold: 0.4,
                    first_token_latency_regressed_warning_threshold: 0.25,
                    first_token_latency_regressed_critical_threshold: 0.4,
                    total_tokens_regressed_warning_threshold: 0.25,
                    total_tokens_regressed_critical_threshold: 0.4,
                },
                latest_snapshot: None,
                previous_snapshot: None,
                trend_summary: None,
                anomalies: Vec::new(),
                by_severity: Vec::new(),
                by_code: Vec::new(),
            },
        }
    }

    #[test]
    fn provider_routing_plan_contains_expected_actions() {
        let plan = build_incident_remediation_plan(
            OffsetDateTime::now_utc(),
            base_incident("provider_routing_score_drop"),
            None,
            Some(base_route_policy()),
            IncidentSyncContext::default(),
        );
        assert!(plan
            .actions
            .iter()
            .any(|item| item.action_key == "provider-routing-review"));
        assert!(plan
            .actions
            .iter()
            .any(|item| item.action_key == "disable-prestream-fallback"));
        assert!(plan
            .actions
            .iter()
            .any(|item| item.action_key == "reduce-provider-concurrency"));
        assert!(plan
            .actions
            .iter()
            .any(|item| item.action_key == "provider-isolation"));
        assert!(plan
            .actions
            .iter()
            .any(|item| item.action_key == "owner-followup"));
    }

    #[test]
    fn hotspot_config_fallback_maps_api_key_hotspot_action() {
        let config = resolve_rate_limit_hotspot_auto_remediation_config(
            &base_incident("rate_limit_api_key_hotspot"),
            Some(&base_route_policy()),
        );
        assert!(config.auto_remediation_enabled);
        assert_eq!(
            config.auto_remediation_action_keys,
            Some(vec!["tighten-api-key-rate-limit".to_string()])
        );
        assert!(config.auto_remediation_dry_run_first);
        assert!(config.auto_remediation_require_alert_before_apply);
        assert_eq!(config.auto_remediation_max_apply_runs_per_incident, Some(1));
    }

    #[test]
    fn route_policy_patch_tightens_project_rate_limit() {
        let route_policy = base_route_policy();
        let patch = build_route_policy_patch(
            "tighten-project-rate-limit",
            &route_policy,
            Some(
                GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    project_rate_limit: Some(GatewayRateLimitDefinition {
                        window_seconds: 60,
                        max_requests: 24,
                    }),
                    ..Default::default()
                },
            ),
        )
        .expect("patch should build");
        assert_eq!(
            patch.next.project_rate_limit,
            Some(GatewayRateLimitDefinition {
                window_seconds: 60,
                max_requests: 24,
            })
        );
        assert!(patch
            .changed_fields
            .iter()
            .any(|item| item == "rateLimitWindowSeconds"));
        assert!(patch
            .changed_fields
            .iter()
            .any(|item| item == "rateLimitMaxRequests"));
    }

    #[test]
    fn route_policy_patch_normalizes_endpoint_rate_limit_key() {
        let route_policy = base_route_policy();
        let patch = build_route_policy_patch(
            "tighten-endpoint-rate-limit",
            &route_policy,
            Some(
                GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
                    endpoint_rate_limit_key: Some("POST /V1/CHAT/COMPLETIONS".to_string()),
                    endpoint_rate_limit: Some(GatewayRateLimitDefinition {
                        window_seconds: 60,
                        max_requests: 20,
                    }),
                    ..Default::default()
                },
            ),
        )
        .expect("patch should build");
        assert_eq!(
            patch.next.endpoint_rate_limit_key.as_deref(),
            Some("post /v1/chat/completions")
        );
    }

    #[test]
    fn anomaly_policy_schedule_marks_due_when_last_sync_expires() {
        let now = OffsetDateTime::parse("2026-04-06T00:45:00Z", &Rfc3339).unwrap();
        let (next_sync_due_at, sync_due) = resolve_anomaly_policy_schedule(
            "enabled",
            true,
            Some(30),
            Some("2026-04-06T00:00:00Z"),
            now,
        );
        assert_eq!(next_sync_due_at.as_deref(), Some("2026-04-06T00:30:00Z"));
        assert!(sync_due);
    }

    #[test]
    fn anomaly_incident_alert_schedule_is_due_without_previous_attempt() {
        let schedule = resolve_anomaly_incident_alert_schedule(
            "open",
            "escalated",
            &AlertConfig {
                alerting_enabled: true,
                alert_interval_minutes: DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES,
                notify_operators: true,
                notify_owner: true,
            },
            None,
            OffsetDateTime::parse("2026-04-06T00:00:00Z", &Rfc3339).unwrap(),
        );
        assert!(schedule.alert_due);
        assert!(schedule.next_alert_due_at.is_none());
    }

    #[test]
    fn anomaly_incident_alert_schedule_respects_interval_from_last_attempt() {
        let schedule = resolve_anomaly_incident_alert_schedule(
            "acknowledged",
            "escalated",
            &AlertConfig {
                alerting_enabled: true,
                alert_interval_minutes: 30,
                notify_operators: true,
                notify_owner: true,
            },
            Some("2026-04-06T00:00:00Z"),
            OffsetDateTime::parse("2026-04-06T00:20:00Z", &Rfc3339).unwrap(),
        );
        assert!(!schedule.alert_due);
        assert_eq!(
            schedule.next_alert_due_at.as_deref(),
            Some("2026-04-06T00:30:00Z")
        );
    }

    #[test]
    fn anomaly_incident_alert_schedule_suppresses_non_escalated_or_disabled_incidents() {
        let schedule = resolve_anomaly_incident_alert_schedule(
            "resolved",
            "resolved",
            &AlertConfig {
                alerting_enabled: false,
                alert_interval_minutes: DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES,
                notify_operators: true,
                notify_owner: true,
            },
            Some("2026-04-06T00:00:00Z"),
            OffsetDateTime::parse("2026-04-06T04:00:00Z", &Rfc3339).unwrap(),
        );
        assert!(!schedule.alert_due);
        assert!(schedule.next_alert_due_at.is_none());
    }

    #[test]
    fn anomaly_alert_delivery_profile_maps_critical_to_danger() {
        let profile = resolve_anomaly_alert_delivery_profile("critical");
        assert_eq!(profile.alert_level, 3);
        assert_eq!(profile.webhook_severity, "danger");
    }

    #[test]
    fn supported_sync_kind_recognizes_provider_routing_and_hotspot_tags() {
        assert!(matches!(
            determine_supported_sync_kind_from_tag(Some("provider-routing:balanced")),
            super::SupportedPolicySyncKind::ProviderRouting
        ));
        assert!(matches!(
            determine_supported_sync_kind_from_tag(Some("rate-limit-hotspot:balanced")),
            super::SupportedPolicySyncKind::RateLimitHotspot
        ));
        assert!(matches!(
            determine_supported_sync_kind_from_tag(Some("analysis-export:balanced")),
            super::SupportedPolicySyncKind::AnalysisExport
        ));
    }

    #[test]
    fn ad_hoc_request_filters_parse_tag_parts_and_explicit_overrides() {
        let filters = build_ad_hoc_request_audit_filters(
            None,
            &GatewayAnalysisAnomalyIncidentSyncInput {
                project_id: Some("project-1".to_string()),
                status: Some("failed".to_string()),
                limit: Some(25),
                ..GatewayAnalysisAnomalyIncidentSyncInput::default()
            },
            Some("provider-routing:balanced:provider:provider-a:endpoint:search"),
        );
        assert_eq!(filters.project_id.as_deref(), Some("project-1"));
        assert_eq!(filters.provider_account_id.as_deref(), Some("provider-a"));
        assert_eq!(filters.endpoint_kind.as_deref(), Some("search"));
        assert_eq!(filters.status.as_deref(), Some("failed"));
        assert_eq!(filters.limit, Some(25));
    }

    #[test]
    fn ad_hoc_analysis_export_defaults_to_active_status() {
        let filters = build_ad_hoc_analysis_export_filters(
            None,
            &GatewayAnalysisAnomalyIncidentSyncInput {
                label: Some("Nightly".to_string()),
                project_id: Some("project-1".to_string()),
                text_mode: Some("chat".to_string()),
                ..GatewayAnalysisAnomalyIncidentSyncInput::default()
            },
            Some("custom-export-tag"),
        );
        assert_eq!(filters.label.as_deref(), Some("Nightly"));
        assert_eq!(filters.project_id.as_deref(), Some("project-1"));
        assert_eq!(filters.tag.as_deref(), Some("custom-export-tag"));
        assert_eq!(filters.text_mode.as_deref(), Some("chat"));
        assert_eq!(filters.status.as_deref(), Some("active"));
    }

    #[test]
    fn ad_hoc_sync_kind_defaults_to_analysis_export_without_request_audit_hints() {
        assert!(matches!(
            determine_supported_ad_hoc_sync_kind(
                Some("custom-export-tag"),
                &GatewayAnalysisAnomalyIncidentSyncInput {
                    project_id: Some("project-1".to_string()),
                    ..GatewayAnalysisAnomalyIncidentSyncInput::default()
                }
            ),
            super::SupportedPolicySyncKind::AnalysisExport
        ));
        assert!(matches!(
            determine_supported_ad_hoc_sync_kind(
                Some("custom-export-tag"),
                &GatewayAnalysisAnomalyIncidentSyncInput {
                    provider_account_id: Some("provider-1".to_string()),
                    ..GatewayAnalysisAnomalyIncidentSyncInput::default()
                }
            ),
            super::SupportedPolicySyncKind::Unsupported(_)
        ));
    }

    #[test]
    fn analysis_anomaly_thresholds_match_balanced_defaults() {
        let thresholds = build_analysis_anomaly_threshold_config("balanced", None);
        assert_eq!(
            thresholds
                .get("failureRateWarningThreshold")
                .and_then(Value::as_f64),
            Some(0.15)
        );
        assert_eq!(
            thresholds
                .get("tokensPerSampleCriticalAbsoluteThreshold")
                .and_then(Value::as_f64),
            Some(2000.0)
        );
    }

    #[test]
    fn remediation_run_summary_counts_meaningful_changes() {
        let before = base_incident("provider_routing_score_drop");
        let mut after = before.clone();
        after.follow_up_status = "investigating".to_string();

        let route_before = base_route_policy();
        let mut route_after = route_before.clone();
        route_after.config.provider_max_concurrent_requests = Some(1);

        let summary =
            build_remediation_run_summary(&[GatewayAnalysisAnomalyIncidentRemediationRunView {
                id: "run-1".to_string(),
                incident_id: "incident-1".to_string(),
                policy_id: None,
                route_policy_id: Some("route-1".to_string()),
                action_key: "reduce-provider-concurrency".to_string(),
                title: "Reduce Provider Concurrency Cap".to_string(),
                execution_mode: "route_policy_patch".to_string(),
                status: "applied".to_string(),
                dry_run: false,
                actor_user_id: "operator-1".to_string(),
                note: None,
                input: None,
                result: None,
                before_incident: Some(before),
                after_incident: Some(after),
                before_route_policy: Some(route_before),
                after_route_policy: Some(route_after),
                error_summary: None,
                created_at: "2026-04-13T00:00:00Z".to_string(),
                completed_at: Some("2026-04-13T00:01:00Z".to_string()),
            }]);

        assert_eq!(summary.total_runs, 1);
        assert_eq!(summary.applied_runs, 1);
        assert_eq!(summary.incident_changed_runs, 1);
        assert_eq!(summary.route_policy_changed_runs, 1);
    }

    #[test]
    fn metric_delta_uses_four_decimal_precision() {
        let metric = build_metric_delta(Some(0.4), Some(0.51));
        assert_eq!(metric.delta_value, Some(0.11));
        assert_eq!(metric.delta_ratio, Some(0.275));
    }

    #[test]
    fn remediation_effectiveness_classifies_improved_and_unavailable_runs() {
        let run = GatewayAnalysisAnomalyIncidentRemediationRunView {
            id: "run-1".to_string(),
            incident_id: "incident-1".to_string(),
            policy_id: None,
            route_policy_id: Some("route-1".to_string()),
            action_key: "provider-isolation".to_string(),
            title: "Isolate Degraded Providers".to_string(),
            execution_mode: "route_policy_patch".to_string(),
            status: "applied".to_string(),
            dry_run: false,
            actor_user_id: "operator-1".to_string(),
            note: None,
            input: None,
            result: None,
            before_incident: Some(base_incident("provider_routing_score_drop")),
            after_incident: Some(base_incident("provider_routing_score_drop")),
            before_route_policy: Some(base_route_policy()),
            after_route_policy: Some(base_route_policy()),
            error_summary: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            completed_at: Some("2026-04-13T00:01:00Z".to_string()),
        };
        let impact = GatewayAnalysisAnomalyRemediationRunImpactView {
            generated_at: "2026-04-13T00:02:00Z".to_string(),
            run: run.clone(),
            incident: run.before_incident.clone(),
            project_id: Some("project-1".to_string()),
            route_policy_id: Some("route-1".to_string()),
            anchor_at: "2026-04-13T00:01:00Z".to_string(),
            window_minutes: 180,
            before_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
                started_at: "2026-04-12T21:01:00Z".to_string(),
                ended_at: "2026-04-13T00:01:00Z".to_string(),
                summary: empty_summary(),
            },
            after_window: GatewayAnalysisAnomalyRemediationImpactWindowView {
                started_at: "2026-04-13T00:01:00Z".to_string(),
                ended_at: "2026-04-13T03:01:00Z".to_string(),
                summary: empty_summary(),
            },
            metrics: GatewayAnalysisAnomalyRemediationRunImpactMetricsView {
                completion_rate: build_metric_delta(Some(0.4), Some(0.5)),
                failure_rate: build_metric_delta(Some(0.2), Some(0.1)),
                cancellation_rate: build_metric_delta(Some(0.0), Some(0.0)),
                stream_rate: build_metric_delta(Some(0.0), Some(0.0)),
                tool_request_rate: build_metric_delta(Some(0.0), Some(0.0)),
                tool_response_rate: build_metric_delta(Some(0.0), Some(0.0)),
                request_artifact_coverage: build_metric_delta(Some(0.1), Some(0.2)),
                response_artifact_coverage: build_metric_delta(Some(0.1), Some(0.2)),
                prompt_tokens_per_sample: build_metric_delta(Some(100.0), Some(80.0)),
                completion_tokens_per_sample: build_metric_delta(Some(40.0), Some(30.0)),
                total_tokens_per_sample: build_metric_delta(Some(200.0), Some(150.0)),
                request_text_chars_avg: build_metric_delta(Some(0.0), Some(0.0)),
                response_text_chars_avg: build_metric_delta(Some(0.0), Some(0.0)),
                first_token_latency_ms_avg: build_metric_delta(Some(400.0), Some(300.0)),
                stream_chunk_count_avg: build_metric_delta(Some(0.0), Some(0.0)),
            },
        };
        let summary = build_remediation_effectiveness_summary(
            OffsetDateTime::now_utc(),
            180,
            &[run.clone(), run],
            &[Some(impact), None],
        );

        assert_eq!(summary.total_runs, 2);
        assert_eq!(summary.impacted_runs, 1);
        assert_eq!(summary.unavailable_runs, 1);
        assert_eq!(summary.completion_rate.improved_runs, 1);
        assert_eq!(summary.completion_rate.unavailable_runs, 1);
        assert_eq!(summary.failure_rate.improved_runs, 1);
        assert_eq!(summary.actions[0].action_key, "provider-isolation");
    }

    #[test]
    fn remediation_effectiveness_anomaly_snapshot_filters_match_normalized_profile_key() {
        let snapshot = empty_anomaly_snapshot();
        let filters = GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
            profile_key: Some("BALANCED".to_string()),
            route_policy_id: Some("route-1".to_string()),
            action_key: Some("provider-isolation".to_string()),
            ..GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters::default()
        };
        assert!(matches_remediation_effectiveness_anomaly_snapshot_filters(
            &snapshot, &filters, None, None
        ));
    }

    #[test]
    fn remediation_effectiveness_anomaly_snapshot_filters_reject_unmatched_profile_key() {
        let snapshot = empty_anomaly_snapshot();
        let filters = GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
            profile_key: Some("aggressive".to_string()),
            ..GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters::default()
        };
        assert!(!matches_remediation_effectiveness_anomaly_snapshot_filters(
            &snapshot, &filters, None, None
        ));
    }

    #[test]
    fn remediation_schedule_requires_alert_before_first_apply() {
        let schedule = resolve_anomaly_remediation_schedule(
            &base_incident("provider_routing_score_drop"),
            &AutoRemediationConfig {
                auto_remediation_enabled: true,
                auto_remediation_interval_minutes: 180,
                auto_remediation_dry_run_first: false,
                auto_remediation_action_keys: Some(vec!["disable-prestream-fallback".to_string()]),
                auto_remediation_max_apply_runs_per_incident: Some(1),
                auto_remediation_require_alert_before_apply: true,
                auto_remediation_freeze_on_provider_health_degrade: true,
            },
            true,
            0,
            false,
            None,
            OffsetDateTime::parse("2026-04-13T03:00:00Z", &Rfc3339).unwrap(),
        );
        assert!(!schedule.remediation_due);
        assert_eq!(schedule.next_execution_status.as_deref(), Some("applied"));
        assert_eq!(schedule.blocked_reason.as_deref(), Some("alert_pending"));
    }

    #[test]
    fn remediation_schedule_promotes_dry_run_to_apply_after_interval() {
        let mut incident = base_incident("provider_routing_score_drop");
        incident.last_alerted_at = Some("2026-04-13T00:10:00Z".to_string());
        let latest_run = GatewayAnalysisAnomalyIncidentRemediationRunView {
            id: "run-1".to_string(),
            incident_id: incident.id.clone(),
            policy_id: None,
            route_policy_id: Some("route-1".to_string()),
            action_key: "provider-isolation".to_string(),
            title: "Isolate Degraded Providers".to_string(),
            execution_mode: "route_policy_patch".to_string(),
            status: "dry_run".to_string(),
            dry_run: true,
            actor_user_id: "operator-1".to_string(),
            note: None,
            input: None,
            result: None,
            before_incident: Some(incident.clone()),
            after_incident: Some(incident.clone()),
            before_route_policy: Some(base_route_policy()),
            after_route_policy: Some(base_route_policy()),
            error_summary: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            completed_at: Some("2026-04-13T00:05:00Z".to_string()),
        };
        let schedule = resolve_anomaly_remediation_schedule(
            &incident,
            &AutoRemediationConfig {
                auto_remediation_enabled: true,
                auto_remediation_interval_minutes: 60,
                auto_remediation_dry_run_first: true,
                auto_remediation_action_keys: Some(vec!["provider-isolation".to_string()]),
                auto_remediation_max_apply_runs_per_incident: Some(2),
                auto_remediation_require_alert_before_apply: true,
                auto_remediation_freeze_on_provider_health_degrade: true,
            },
            true,
            0,
            false,
            Some(&latest_run),
            OffsetDateTime::parse("2026-04-13T02:00:00Z", &Rfc3339).unwrap(),
        );
        assert!(schedule.remediation_due);
        assert_eq!(schedule.next_execution_status.as_deref(), Some("applied"));
        assert!(schedule.blocked_reason.is_none());
    }
}
