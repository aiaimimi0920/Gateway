//! Management request routes with domain-owned query and report implementations.

mod analysis_exports;
mod analysis_reports;
mod anomaly_incidents;
mod anomaly_policies;
mod audits;
mod rate_limit_hotspots;
mod remediation;
mod remediation_reports;
mod runtime_analysis;

#[cfg(test)]
mod query_contract_tests;

pub use audits::{
    get_prompt_cache_trend_report, get_request_artifacts_by_id,
    get_request_artifacts_by_response_id, get_request_audit_by_id,
    get_request_audit_by_response_id, list_analysis_samples, list_request_audits,
    summarize_analysis, summarize_prompt_cache, summarize_request_audits, PromptCacheQuery,
    RequestAuditQuery,
};

pub use runtime_analysis::{
    get_cost_overview, get_model_association_matrix, get_provider_routing_anomaly_report,
    get_runtime_pressure, summarize_provider_routing_analysis,
    sync_provider_routing_anomaly_incidents, RuntimePressureQuery,
};

pub use analysis_exports::{
    cleanup_expired_analysis_exports, export_analysis_rows, get_persisted_analysis_export,
    list_persisted_analysis_exports, persist_analysis_export, summarize_persisted_analysis_exports,
    update_persisted_analysis_export_metadata, AnalysisExportQuery, PersistAnalysisExportInput,
    PersistedAnalysisExportQuery,
};

pub use analysis_reports::{
    get_analysis_export_anomaly_report, get_analysis_export_baseline_report,
    get_analysis_export_timeline_report, get_analysis_export_trend_report,
    get_persisted_analysis_export_diff, AnalysisExportAnomalyQuery, AnalysisExportDiffQuery,
};

pub use rate_limit_hotspots::{
    get_rate_limit_hotspot_anomaly_report, get_rate_limit_hotspot_anomaly_snapshot,
    get_rate_limit_hotspot_snapshot, get_rate_limit_hotspot_snapshot_trend_report,
    get_rate_limit_hotspot_trend_report, list_rate_limit_hotspot_anomaly_snapshots,
    list_rate_limit_hotspot_snapshots, persist_rate_limit_hotspot_anomaly_snapshot,
    persist_rate_limit_hotspot_snapshot, summarize_rate_limit_hotspot_snapshot_inventory,
    summarize_rate_limit_hotspots, sync_rate_limit_hotspot_anomaly_incidents,
    RateLimitHotspotQuery,
};

pub use anomaly_policies::{
    list_anomaly_policies, save_anomaly_policy, summarize_anomaly_policies, sweep_anomaly_policies,
    sync_anomaly_policy, AnomalyPolicyQuery,
};

pub use anomaly_incidents::{
    acknowledge_anomaly_incident, list_anomaly_incident_alert_queue, list_anomaly_incident_history,
    list_anomaly_incidents, record_anomaly_incident_alert_dispatch, resolve_anomaly_incident,
    summarize_anomaly_incidents, sync_anomaly_incidents, update_anomaly_incident_follow_up,
    AlertDispatchInput, AnomalyIncidentQuery, AnomalyIncidentSyncInput,
};

pub use remediation::{
    capture_remediation_run_impact, execute_incident_remediation_run,
    get_anomaly_incident_remediation_plan, get_remediation_run_impact,
    list_incident_remediation_runs, list_remediation_queue, list_remediation_runs,
    summarize_remediation_runs, sweep_remediation_runs, RemediationQueueQuery, RemediationRunQuery,
};

pub use remediation_reports::{
    get_remediation_effectiveness_anomaly_snapshot, get_remediation_effectiveness_snapshot,
    get_remediation_effectiveness_snapshot_anomaly_report,
    get_remediation_effectiveness_trend_report, list_remediation_effectiveness_anomaly_snapshots,
    list_remediation_effectiveness_snapshots, persist_remediation_effectiveness_anomaly_snapshot,
    persist_remediation_effectiveness_snapshot, summarize_remediation_effectiveness,
    summarize_remediation_effectiveness_snapshots, RemediationSnapshotQuery,
};

use crate::error::GatewayError;
use crate::state::AppState;
use axum::http::HeaderMap as AxumHeaderMap;

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

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
