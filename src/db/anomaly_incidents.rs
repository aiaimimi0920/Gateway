use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::rate_limit_hotspots::GatewayRateLimitHotspotAnomalyView;
use crate::error::GatewayError;

use super::analysis_exports::{
    GatewayAnalysisExportAnomalyReportView, GatewayAnalysisExportAnomalyView,
};
use super::request_audits::{
    GatewayProviderRoutingAnalysisAnomalyView, GatewaySummaryBucketKeyView,
};
use super::{
    format_timestamp, get_provider_routing_anomaly_report, get_rate_limit_hotspot_anomaly_snapshot,
    map_db_error, GatewayProviderRoutingAnalysisAnomalyOverrides,
    GatewayProviderRoutingAnalysisAnomalyReportView, GatewayRateLimitHotspotAnomalySnapshotView,
    RequestAuditFilters,
};

mod alert_dispatch;
mod escalation;
mod export_metadata;
mod export_persistence;
mod follow_up;
mod history;
mod hotspot_persistence;
mod normalization;
mod provider_persistence;
mod queries;
mod summary;
mod synchronization;

pub use alert_dispatch::record_anomaly_incident_alert_dispatch;
pub use follow_up::{
    acknowledge_anomaly_incident, resolve_anomaly_incident, update_anomaly_incident_follow_up,
};
pub use history::list_anomaly_incident_history;
pub use queries::list_anomaly_incidents;
pub use summary::summarize_anomaly_incidents;
pub use synchronization::{
    sync_analysis_export_anomaly_incidents, sync_provider_routing_anomaly_incidents,
    sync_rate_limit_hotspot_anomaly_incidents,
};

use escalation::{
    resolve_analysis_export_auto_escalation, resolve_provider_routing_auto_escalation,
    resolve_rate_limit_hotspot_auto_escalation,
};
use export_persistence::{
    create_analysis_export_incident, resolve_analysis_export_incident,
    update_analysis_export_incident,
};
use follow_up::build_incident_snapshot_metadata_from_view;
use history::append_anomaly_incident_history;
use hotspot_persistence::{
    create_rate_limit_hotspot_incident, resolve_rate_limit_hotspot_incident,
    update_rate_limit_hotspot_incident,
};
use normalization::{
    normalize_incident_escalation_status, normalize_incident_follow_up_status,
    normalize_incident_status, require_incident_id, trimmed_owned, trimmed_owned_ref,
};
use provider_persistence::{
    create_provider_routing_incident, resolve_provider_routing_incident,
    update_provider_routing_incident,
};
use queries::{fetch_scope_incidents, get_anomaly_incident, get_anomaly_incident_row};
#[cfg(test)]
use synchronization::{build_adhoc_incident_fingerprint, build_provider_routing_incident_tag};

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisAnomalyIncidentFilters {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentView {
    pub id: String,
    pub policy_id: Option<String>,
    pub fingerprint: String,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub code: String,
    pub severity: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub follow_up_status: String,
    pub sync_hit_count: i32,
    pub escalation_status: String,
    pub escalated_at: Option<String>,
    pub escalation_reason: Option<String>,
    pub latest_note: Option<String>,
    pub resolution_note: Option<String>,
    pub last_action_at: Option<String>,
    pub last_alert_attempt_at: Option<String>,
    pub last_alerted_at: Option<String>,
    pub last_alert_severity: Option<String>,
    pub alert_delivery_count: i32,
    pub summary: String,
    pub latest_export_id: Option<String>,
    pub previous_export_id: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub acknowledged_at: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSummaryView {
    pub total_incidents: usize,
    pub open_incidents: usize,
    pub acknowledged_incidents: usize,
    pub resolved_incidents: usize,
    pub escalated_incidents: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
    pub by_follow_up_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_escalation_status: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentFollowUpInput {
    pub owner_user_id: Option<String>,
    pub follow_up_status: Option<String>,
    pub note: Option<String>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput {
    pub alerted_at: Option<String>,
    pub alert_severity: Option<String>,
    pub alert_level: Option<i32>,
    pub note: Option<String>,
    pub mailbox_recipient_count: Option<i32>,
    pub webhook_dispatched: Option<bool>,
    pub webhook_skipped_reason: Option<String>,
    pub remediation_action_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentHistoryView {
    pub id: String,
    pub incident_id: String,
    pub event_type: String,
    pub actor_user_id: Option<String>,
    pub note: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult {
    pub report: GatewayProviderRoutingAnalysisAnomalyReportView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncRateLimitHotspotAnomalyIncidentsResult {
    pub snapshot: GatewayRateLimitHotspotAnomalySnapshotView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewaySyncAnalysisExportAnomalyIncidentsInput {
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub auto_escalation: GatewayAnalysisExportAutoEscalationConfig,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisExportAutoEscalationConfig {
    pub enabled: bool,
    pub severity_threshold: Option<String>,
    pub after_sync_count: Option<i32>,
    pub owner_user_id: Option<String>,
    pub follow_up_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncAnalysisExportAnomalyIncidentsResult {
    pub report: GatewayAnalysisExportAnomalyReportView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentRow {
    id: String,
    policy_id: Option<String>,
    fingerprint: String,
    project_id: Option<String>,
    route_policy_id: Option<String>,
    tag: Option<String>,
    text_mode: Option<String>,
    code: String,
    severity: String,
    status: String,
    owner_user_id: Option<String>,
    follow_up_status: String,
    sync_hit_count: i32,
    escalation_status: String,
    escalated_at: Option<OffsetDateTime>,
    escalation_reason: Option<String>,
    latest_note: Option<String>,
    resolution_note: Option<String>,
    last_action_at: Option<OffsetDateTime>,
    last_alert_attempt_at: Option<OffsetDateTime>,
    last_alerted_at: Option<OffsetDateTime>,
    last_alert_severity: Option<String>,
    alert_delivery_count: i32,
    summary: String,
    latest_export_id: Option<String>,
    previous_export_id: Option<String>,
    latest_value: Option<f64>,
    previous_value: Option<f64>,
    delta_value: Option<f64>,
    delta_ratio: Option<f64>,
    threshold_value: Option<f64>,
    first_seen_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    acknowledged_at: Option<OffsetDateTime>,
    resolved_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentHistoryRow {
    id: String,
    incident_id: String,
    event_type: String,
    actor_user_id: Option<String>,
    note: Option<String>,
    metadata: Option<sqlx::types::Json<Value>>,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
struct ProviderRoutingEscalationDecision {
    should_escalate: bool,
    reason: Option<String>,
    owner_user_id: Option<String>,
    follow_up_status: Option<String>,
}

#[cfg(test)]
mod tests;
