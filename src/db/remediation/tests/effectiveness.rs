use super::*;

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
