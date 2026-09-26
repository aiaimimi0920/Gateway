use super::*;
use serde_json::json;

fn sample_with_selected(route_trace: Value) -> GatewayAnalysisSampleView {
    GatewayAnalysisSampleView {
        request_audit_id: "req-1".to_string(),
        response_id: "resp-1".to_string(),
        project_id: "proj-1".to_string(),
        route_policy_id: Some("policy-1".to_string()),
        session_id: None,
        provider_account_id: Some("provider-a".to_string()),
        protocol_family: "openai".to_string(),
        endpoint_kind: "chat_completions".to_string(),
        requested_model: Some("gpt-4o".to_string()),
        resolved_model: Some("gpt-4o".to_string()),
        status: "completed".to_string(),
        stream: false,
        created_at: "2026-04-13T00:00:00Z".to_string(),
        completed_at: Some("2026-04-13T00:00:01Z".to_string()),
        prompt_tokens: Some(10),
        completion_tokens: Some(5),
        total_tokens: Some(15),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        analysis_profile: None,
        request_artifact_object_key: None,
        response_artifact_object_key: None,
        route_trace: Some(route_trace),
    }
}

#[test]
fn provider_routing_summary_counts_selected_candidates() {
    let rows = vec![
        sample_with_selected(json!({
            "selectedCandidate": {
                "providerAccountId": "provider-a",
                "routingScore": 0.61,
                "healthWeight": 0.7,
                "capacityWeight": 0.4,
                "degraded": true,
                "breakerOpen": false,
                "degradationReasons": ["concurrency_pressure", "failure_count_elevated"]
            }
        })),
        sample_with_selected(json!({
            "selectedCandidate": {
                "providerAccountId": "provider-b",
                "routingScore": 0.92,
                "healthWeight": 1.0,
                "capacityWeight": 1.0,
                "degraded": false,
                "breakerOpen": true,
                "degradationReasons": ["breaker_open"]
            }
        })),
        sample_with_selected(json!({
            "other": "ignored"
        })),
    ];

    let summary = build_provider_routing_summary(&rows);
    assert_eq!(summary.total_samples, 3);
    assert_eq!(summary.selected_provider_samples, 2);
    assert_eq!(summary.degraded_selected_provider_samples, 1);
    assert_eq!(summary.saturated_selected_provider_samples, 0);
    assert_eq!(summary.breaker_open_selected_provider_samples, 1);
    assert_eq!(summary.by_selected_provider[0].key, "provider-a");
    assert_eq!(summary.by_selected_provider[1].key, "provider-b");
    assert_eq!(summary.by_degradation_reason[0].count, 1);
}

#[test]
fn provider_routing_distribution_matches_ts_rank_logic() {
    let values = vec![Some(0.1), Some(0.2), Some(0.3), Some(0.4)];
    let distribution = build_ts_distribution(&values);
    assert_eq!(distribution.avg, Some(0.25));
    assert_eq!(distribution.p50, Some(0.2));
    assert_eq!(distribution.p95, Some(0.4));
}

#[test]
fn provider_routing_thresholds_match_balanced_defaults() {
    let thresholds = build_provider_routing_anomaly_thresholds(
        "balanced",
        GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
    );
    assert_eq!(thresholds.routing_score_warning_threshold, 0.55);
    assert_eq!(thresholds.routing_score_critical_threshold, 0.35);
    assert_eq!(thresholds.breaker_open_route_warning_threshold, 0.02);
    assert_eq!(thresholds.breaker_open_route_critical_threshold, 0.08);
}

#[test]
fn provider_routing_anomaly_report_flags_low_score_and_breaker_usage() {
    let filters = RequestAuditFilters {
        project_id: Some("proj-1".to_string()),
        limit: Some(1000),
        ..RequestAuditFilters::default()
    };
    let summary = GatewayProviderRoutingAnalysisSummaryView {
        total_samples: 4,
        selected_provider_samples: 4,
        degraded_selected_provider_samples: 2,
        saturated_selected_provider_samples: 1,
        breaker_open_selected_provider_samples: 1,
        routing_score: GatewayAnalysisMetricDistributionView {
            avg: Some(0.30),
            p50: Some(0.30),
            p95: Some(0.60),
        },
        health_weight: GatewayAnalysisMetricDistributionView {
            avg: Some(0.5),
            p50: Some(0.5),
            p95: Some(0.8),
        },
        capacity_weight: GatewayAnalysisMetricDistributionView {
            avg: Some(0.2),
            p50: Some(0.2),
            p95: Some(0.7),
        },
        by_selected_provider: vec![GatewaySummaryBucketKeyView {
            key: "provider-a".to_string(),
            count: 4,
        }],
        by_degradation_reason: vec![GatewaySummaryBucketKeyView {
            key: "breaker_open".to_string(),
            count: 1,
        }],
    };

    let report = build_provider_routing_anomaly_report(
        &filters,
        "balanced",
        build_provider_routing_anomaly_thresholds(
            "balanced",
            GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
        ),
        summary,
    );

    assert_eq!(report.profile_key, "balanced");
    assert_eq!(report.anomalies.len(), 4);
    assert!(report
        .anomalies
        .iter()
        .any(|anomaly| anomaly.code == "provider_routing_score_drop"));
    assert!(report
        .anomalies
        .iter()
        .any(|anomaly| anomaly.code == "breaker_open_provider_route_detected"));
}

#[test]
fn prompt_cache_summary_tracks_client_and_auto_applied_counts() {
    let summary = build_prompt_cache_summary_view(10, 4, 3, 2, 5, 1200, 800, 3.0);
    assert_eq!(summary.total_requests, 10);
    assert_eq!(summary.client_marked_requests, 2);
    assert_eq!(summary.auto_applied_requests, 5);
    assert_eq!(summary.cache_control_coverage_requests, 7);
    assert_eq!(summary.cache_hit_rate, 0.4);
    assert_eq!(summary.cache_control_coverage_rate, 0.7);
}
