use super::*;

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
