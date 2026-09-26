use super::{
    build_ad_hoc_analysis_export_filters, build_ad_hoc_request_audit_filters,
    build_analysis_anomaly_threshold_config, build_incident_remediation_plan, build_metric_delta,
    build_remediation_effectiveness_summary, build_remediation_run_summary,
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

use super::SupportedPolicySyncKind;

mod actions;
mod effectiveness;
mod scheduling;
mod sync;

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

fn empty_anomaly_snapshot() -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
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
