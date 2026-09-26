use std::collections::{BTreeMap, HashMap, HashSet};

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

mod action_models;
mod alerts;
mod auto_remediation;
mod effectiveness;
mod effectiveness_anomalies;
mod effectiveness_anomaly_models;
mod effectiveness_anomaly_snapshots;
mod effectiveness_models;
mod effectiveness_snapshots;
mod effectiveness_trends;
mod execution;
mod impact;
mod plan;
mod plan_context;
mod policies;
mod policy_models;
mod policy_parameters;
mod policy_sweep;
mod policy_sync;
mod policy_sync_filters;
mod policy_write;
mod queue;
mod route_policy_patch;
mod run_models;
mod runs;
mod sweep;

pub use action_models::{
    ExecuteGatewayAnalysisAnomalyIncidentRemediationInput,
    GatewayAnalysisAnomalyIncidentAlertQueueItemView, GatewayAnalysisAnomalyIncidentAlertQueueView,
    GatewayAnalysisAnomalyIncidentRemediationActionView,
    GatewayAnalysisAnomalyIncidentRemediationPlanView,
    GatewayAnalysisAnomalyIncidentRemediationQueueItemView,
    GatewayAnalysisAnomalyIncidentRemediationQueueView,
    GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput,
    GatewayAnalysisAnomalyRemediationQueueFilters, GatewayAnalysisAnomalyRemediationSweepItemView,
    GatewayAnalysisAnomalyRemediationSweepView,
};
pub use alerts::list_anomaly_incident_alert_queue;
pub use effectiveness::get_anomaly_remediation_effectiveness;
pub use effectiveness_anomalies::get_anomaly_remediation_effectiveness_snapshot_anomaly_report;
pub use effectiveness_anomaly_models::{
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView,
};
pub use effectiveness_anomaly_snapshots::{
    get_anomaly_remediation_effectiveness_anomaly_snapshot,
    list_anomaly_remediation_effectiveness_anomaly_snapshots,
    persist_anomaly_remediation_effectiveness_anomaly_snapshot,
};
pub use effectiveness_models::{
    GatewayAnalysisAnomalyRemediationActionEffectivenessView,
    GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    GatewayAnalysisAnomalyRemediationEffectivenessSummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView,
    GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView,
};
pub use effectiveness_snapshots::{
    get_anomaly_remediation_effectiveness_snapshot,
    list_anomaly_remediation_effectiveness_snapshots,
    persist_anomaly_remediation_effectiveness_snapshot,
    summarize_anomaly_remediation_effectiveness_snapshots,
};
pub use effectiveness_trends::get_anomaly_remediation_effectiveness_trend_report;
pub use execution::execute_anomaly_incident_remediation;
pub use impact::{
    capture_anomaly_incident_remediation_run_impact, get_anomaly_incident_remediation_run_impact,
};
pub use plan_context::get_anomaly_incident_remediation_plan;
pub use policies::{list_anomaly_policies, summarize_anomaly_policies};
pub use policy_models::{
    GatewayAnalysisAnomalyIncidentSyncInput, GatewayAnalysisAnomalyIncidentSyncView,
    GatewayAnalysisAnomalyPolicyFilters, GatewayAnalysisAnomalyPolicySummaryView,
    GatewayAnalysisAnomalyPolicySweepItemView, GatewayAnalysisAnomalyPolicySweepView,
    GatewayAnalysisAnomalyPolicySyncView, GatewayAnalysisAnomalyPolicyView,
    UpsertGatewayAnalysisAnomalyPolicyInput,
};
pub use policy_sweep::sweep_anomaly_policies;
pub use policy_sync::{sync_anomaly_incidents, sync_anomaly_policy};
pub use policy_write::save_anomaly_policy;
pub use queue::list_anomaly_remediation_queue;
pub use run_models::{
    GatewayAnalysisAnomalyIncidentRemediationRunView,
    GatewayAnalysisAnomalyRemediationImpactCaptureView,
    GatewayAnalysisAnomalyRemediationImpactMetricView,
    GatewayAnalysisAnomalyRemediationImpactWindowView, GatewayAnalysisAnomalyRemediationRunFilters,
    GatewayAnalysisAnomalyRemediationRunImpactMetricsView,
    GatewayAnalysisAnomalyRemediationRunImpactView,
    GatewayAnalysisAnomalyRemediationRunSummaryView,
};
pub use runs::{
    list_anomaly_incident_remediation_runs, summarize_anomaly_incident_remediation_runs,
};
pub use sweep::sweep_anomaly_remediations;

#[cfg(test)]
use alerts::{resolve_anomaly_alert_delivery_profile, resolve_anomaly_incident_alert_schedule};
#[cfg(test)]
use auto_remediation::resolve_rate_limit_hotspot_auto_remediation_config;
#[cfg(test)]
use effectiveness::build_remediation_effectiveness_summary;
#[cfg(test)]
use effectiveness_anomaly_snapshots::matches_remediation_effectiveness_anomaly_snapshot_filters;
#[cfg(test)]
use impact::build_metric_delta;
#[cfg(test)]
use plan::build_incident_remediation_plan;
#[cfg(test)]
use policies::resolve_anomaly_policy_schedule;
#[cfg(test)]
use policy_parameters::build_analysis_anomaly_threshold_config;
#[cfg(test)]
use policy_sync::{determine_supported_ad_hoc_sync_kind, determine_supported_sync_kind_from_tag};
#[cfg(test)]
use policy_sync_filters::{
    build_ad_hoc_analysis_export_filters, build_ad_hoc_request_audit_filters,
};
#[cfg(test)]
use queue::resolve_anomaly_remediation_schedule;
#[cfg(test)]
use route_policy_patch::build_route_policy_patch;
#[cfg(test)]
use runs::build_remediation_run_summary;

const DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES: i32 = 180;

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

#[derive(Debug, Clone)]
struct RoutePolicyPatchSpec {
    next: GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput,
    changed_fields: Vec<String>,
    summary: String,
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

#[cfg(test)]
mod tests;
