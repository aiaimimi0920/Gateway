use super::{
    build_adhoc_incident_fingerprint, build_provider_routing_incident_tag,
    resolve_analysis_export_auto_escalation, resolve_provider_routing_auto_escalation,
    GatewayAnalysisExportAutoEscalationConfig, GatewayProviderRoutingAnalysisAnomalyReportView,
};
use crate::db::request_audits::{
    GatewayAnalysisMetricDistributionView, GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
    GatewayProviderRoutingAnalysisFilterView, GatewayProviderRoutingAnalysisSummaryView,
};

#[test]
fn provider_routing_incident_tag_matches_ts_shape() {
    let tag =
        build_provider_routing_incident_tag(&GatewayProviderRoutingAnalysisAnomalyReportView {
            generated_at: "2026-04-13T00:00:00Z".to_string(),
            filters: GatewayProviderRoutingAnalysisFilterView {
                project_id: Some("proj-1".to_string()),
                route_policy_id: Some("rp-1".to_string()),
                provider_account_id: Some("provider-1".to_string()),
                session_id: Some("session-1".to_string()),
                api_key_id: Some("key-1".to_string()),
                response_id: Some("resp-1".to_string()),
                protocol_family: Some("OpenAI".to_string()),
                endpoint_kind: Some("Chat".to_string()),
                status: Some("FAILED".to_string()),
                created_from: None,
                created_to: None,
                limit: 50,
            },
            profile_key: "balanced".to_string(),
            thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
                routing_score_warning_threshold: 0.65,
                routing_score_critical_threshold: 0.4,
                degraded_route_warning_threshold: 0.15,
                degraded_route_critical_threshold: 0.35,
                saturated_route_warning_threshold: 0.1,
                saturated_route_critical_threshold: 0.25,
                breaker_open_route_warning_threshold: 0.05,
                breaker_open_route_critical_threshold: 0.15,
            },
            summary: GatewayProviderRoutingAnalysisSummaryView {
                total_samples: 0,
                selected_provider_samples: 0,
                degraded_selected_provider_samples: 0,
                saturated_selected_provider_samples: 0,
                breaker_open_selected_provider_samples: 0,
                routing_score: GatewayAnalysisMetricDistributionView {
                    avg: None,
                    p50: None,
                    p95: None,
                },
                health_weight: GatewayAnalysisMetricDistributionView {
                    avg: None,
                    p50: None,
                    p95: None,
                },
                capacity_weight: GatewayAnalysisMetricDistributionView {
                    avg: None,
                    p50: None,
                    p95: None,
                },
                by_selected_provider: Vec::new(),
                by_degradation_reason: Vec::new(),
            },
            anomalies: Vec::new(),
            by_severity: Vec::new(),
            by_code: Vec::new(),
        });

    assert_eq!(
        tag,
        "provider-routing:balanced:provider:provider-1:protocol:openai:endpoint:chat:api-key:key-1:session:session-1:response:resp-1:status:failed"
    );
}

#[test]
fn provider_routing_adhoc_fingerprint_matches_ts_shape() {
    let fingerprint = build_adhoc_incident_fingerprint(
        None,
        Some("project-1"),
        Some("route-1"),
        Some("provider-routing:balanced"),
        None,
        "routing_score_low",
    );
    assert_eq!(
        fingerprint,
        "adhoc:project:project-1:routePolicy:route-1:tag:provider-routing:balanced:textMode:*:code:routing_score_low"
    );
}

#[test]
fn analysis_export_policy_fingerprint_matches_ts_shape() {
    let fingerprint = build_adhoc_incident_fingerprint(
        Some("policy-1"),
        Some("project-1"),
        Some("route-1"),
        Some("custom-export-tag"),
        Some("chat"),
        "failure_rate_spike",
    );
    assert_eq!(fingerprint, "policy:policy-1:code:failure_rate_spike");
}

#[test]
fn analysis_export_adhoc_fingerprint_keeps_text_mode() {
    let fingerprint = build_adhoc_incident_fingerprint(
        None,
        Some("project-1"),
        None,
        Some("custom-export-tag"),
        Some("chat"),
        "failure_rate_spike",
    );
    assert_eq!(
        fingerprint,
        "adhoc:project:project-1:routePolicy:*:tag:custom-export-tag:textMode:chat:code:failure_rate_spike"
    );
}

#[test]
fn provider_routing_auto_escalation_matches_ts_rules() {
    let warning_before_threshold = resolve_provider_routing_auto_escalation("warning", 2);
    assert!(!warning_before_threshold.should_escalate);

    let warning_after_threshold = resolve_provider_routing_auto_escalation("warning", 3);
    assert!(warning_after_threshold.should_escalate);
    assert_eq!(
        warning_after_threshold.reason.as_deref(),
        Some("Provider routing auto escalated after 3 warning sync hit(s).")
    );
    assert_eq!(
        warning_after_threshold.follow_up_status.as_deref(),
        Some("monitoring")
    );

    let critical = resolve_provider_routing_auto_escalation("critical", 1);
    assert!(critical.should_escalate);
    assert_eq!(
        critical.reason.as_deref(),
        Some(
            "Provider routing auto escalated immediately at severity critical after 1 sync hit(s)."
        )
    );
    assert_eq!(critical.follow_up_status.as_deref(), Some("investigating"));
}

#[test]
fn analysis_export_auto_escalation_matches_ts_rules() {
    let disabled = resolve_analysis_export_auto_escalation(
        &GatewayAnalysisExportAutoEscalationConfig::default(),
        "critical",
        4,
    );
    assert!(!disabled.should_escalate);

    let warning_before_threshold = resolve_analysis_export_auto_escalation(
        &GatewayAnalysisExportAutoEscalationConfig {
            enabled: true,
            severity_threshold: Some("warning".to_string()),
            after_sync_count: Some(3),
            owner_user_id: Some("owner-1".to_string()),
            follow_up_status: Some("monitoring".to_string()),
        },
        "warning",
        2,
    );
    assert!(!warning_before_threshold.should_escalate);

    let warning = resolve_analysis_export_auto_escalation(
        &GatewayAnalysisExportAutoEscalationConfig {
            enabled: true,
            severity_threshold: Some("warning".to_string()),
            after_sync_count: Some(3),
            owner_user_id: Some("owner-1".to_string()),
            follow_up_status: Some("monitoring".to_string()),
        },
        "warning",
        3,
    );
    assert!(warning.should_escalate);
    assert_eq!(
        warning.reason.as_deref(),
        Some("Auto escalated after 3 sync hit(s) at severity warning.")
    );
    assert_eq!(warning.owner_user_id.as_deref(), Some("owner-1"));
    assert_eq!(warning.follow_up_status.as_deref(), Some("monitoring"));

    let critical = resolve_analysis_export_auto_escalation(
        &GatewayAnalysisExportAutoEscalationConfig {
            enabled: true,
            severity_threshold: None,
            after_sync_count: None,
            owner_user_id: None,
            follow_up_status: Some("investigating".to_string()),
        },
        "critical",
        3,
    );
    assert!(critical.should_escalate);
    assert_eq!(
        critical.reason.as_deref(),
        Some("Auto escalated after 3 sync hit(s) at severity critical.")
    );
    assert_eq!(critical.follow_up_status.as_deref(), Some("investigating"));
}
