use serde_json::{json, Value};

use super::{
    build_rate_limit_hotspot_anomaly_report, build_rate_limit_hotspot_anomaly_thresholds,
    build_rate_limit_hotspot_snapshot_inventory_summary,
    build_rate_limit_hotspot_snapshot_trend_point, build_rate_limit_hotspot_snapshot_trend_report,
    build_rate_limit_hotspot_summary, build_rate_limit_hotspot_trend_report,
    GatewayRateLimitHotspotAnomalyOverrides, GatewayRateLimitHotspotFilterView,
    GatewayRateLimitHotspotSnapshotFilterView, GatewayRateLimitHotspotSnapshotReportFilterView,
    GatewayRateLimitHotspotSnapshotView,
};
use crate::db::request_audits::{GatewayRequestAuditView, GatewaySummaryBucketKeyView};

fn base_audit() -> GatewayRequestAuditView {
    GatewayRequestAuditView {
        id: "audit-1".to_string(),
        project_id: "project-a".to_string(),
        api_key_id: Some("key-a".to_string()),
        user_credential_id: None,
        access_key_id: Some("access-key-a".to_string()),
        source_access_key_id: Some("access-key-a".to_string()),
        session_id: None,
        route_policy_id: Some("policy-1".to_string()),
        provider_account_id: None,
        protocol_family: "openai".to_string(),
        endpoint_kind: "chat.completions".to_string(),
        requested_model: Some("gpt-4".to_string()),
        resolved_model: Some("gpt-4".to_string()),
        model_alias: None,
        stream: false,
        status: "failed".to_string(),
        upstream_status: Some(429),
        duration_ms: Some(1000),
        prompt_tokens: Some(1),
        completion_tokens: Some(1),
        total_tokens: Some(2),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        client_has_cache_control: false,
        auto_cache_applied: false,
        error_summary: Some("rate limit".to_string()),
        route_trace: Some(json!({ "errorCode": "rate_limit_exceeded_project" })),
        analysis_profile: None,
        request_artifact_object_key: None,
        response_artifact_object_key: None,
        response_id: "resp-1".to_string(),
        previous_response_id: None,
        client_disconnected_at: None,
        created_at: "2026-04-07T11:10:00Z".to_string(),
        completed_at: Some("2026-04-07T11:10:01Z".to_string()),
        updated_at: "2026-04-07T11:10:01Z".to_string(),
    }
}

fn create_audit(overrides: Value) -> GatewayRequestAuditView {
    let mut row = base_audit();
    if let Some(id) = overrides.get("id").and_then(Value::as_str) {
        row.id = id.to_string();
    }
    if let Some(project_id) = overrides.get("projectId").and_then(Value::as_str) {
        row.project_id = project_id.to_string();
    }
    if let Some(route_policy_id) = overrides.get("routePolicyId").and_then(Value::as_str) {
        row.route_policy_id = Some(route_policy_id.to_string());
    }
    if overrides.get("routePolicyId").is_some_and(Value::is_null) {
        row.route_policy_id = None;
    }
    if let Some(api_key_id) = overrides.get("apiKeyId").and_then(Value::as_str) {
        row.api_key_id = Some(api_key_id.to_string());
    }
    if let Some(requested_model) = overrides.get("requestedModel").and_then(Value::as_str) {
        row.requested_model = Some(requested_model.to_string());
    }
    if let Some(resolved_model) = overrides.get("resolvedModel").and_then(Value::as_str) {
        row.resolved_model = Some(resolved_model.to_string());
    }
    if let Some(endpoint_kind) = overrides.get("endpointKind").and_then(Value::as_str) {
        row.endpoint_kind = endpoint_kind.to_string();
    }
    if let Some(created_at) = overrides.get("createdAt").and_then(Value::as_str) {
        row.created_at = created_at.to_string();
    }
    if let Some(route_trace) = overrides.get("routeTrace") {
        row.route_trace = Some(route_trace.clone());
    }
    row
}

fn create_snapshot(
    snapshot_id: &str,
    created_at: &str,
    label: Option<&str>,
    total_rate_limited_requests: usize,
    code: &str,
    project: &str,
    api_key: &str,
    endpoint_kind: &str,
) -> GatewayRateLimitHotspotSnapshotView {
    GatewayRateLimitHotspotSnapshotView {
        snapshot_id: snapshot_id.to_string(),
        label: label.map(|value| value.to_string()),
        created_at: created_at.to_string(),
        object_key: format!("ai-gateway/rate-limit-hotspot-snapshots/{snapshot_id}/snapshot.json"),
        filters: GatewayRateLimitHotspotSnapshotFilterView {
            project_id: Some(project.to_string()),
            route_policy_id: Some("policy-a".to_string()),
            provider_account_id: None,
            session_id: None,
            api_key_id: Some(api_key.to_string()),
            response_id: None,
            protocol_family: None,
            endpoint_kind: Some(endpoint_kind.to_string()),
            error_code: None,
            created_from: None,
            created_to: None,
            limit: 1000,
            lookback_hours: Some(24),
        },
        summary: super::GatewayRateLimitHotspotSummaryView {
            total_rate_limited_requests,
            by_code: vec![GatewaySummaryBucketKeyView {
                key: code.to_string(),
                count: total_rate_limited_requests,
            }],
            by_project: vec![GatewaySummaryBucketKeyView {
                key: project.to_string(),
                count: total_rate_limited_requests,
            }],
            by_route_policy_id: vec![GatewaySummaryBucketKeyView {
                key: "policy-a".to_string(),
                count: total_rate_limited_requests,
            }],
            by_api_key_id: vec![GatewaySummaryBucketKeyView {
                key: api_key.to_string(),
                count: total_rate_limited_requests,
            }],
            by_requested_model: vec![GatewaySummaryBucketKeyView {
                key: "gpt-4o".to_string(),
                count: total_rate_limited_requests,
            }],
            by_resolved_model: vec![GatewaySummaryBucketKeyView {
                key: "gpt-4o".to_string(),
                count: total_rate_limited_requests,
            }],
            by_endpoint_kind: vec![GatewaySummaryBucketKeyView {
                key: endpoint_kind.to_string(),
                count: total_rate_limited_requests,
            }],
        },
    }
}

#[test]
fn hotspot_summary_counts_only_rate_limit_rows() {
    let rows = vec![
        create_audit(
            json!({ "id": "audit-rl-1", "routeTrace": { "errorCode": "rate_limit_exceeded_project" } }),
        ),
        create_audit(
            json!({ "id": "audit-rl-2", "projectId": "project-b", "routePolicyId": "policy-2", "apiKeyId": "key-b", "endpointKind": "responses", "routeTrace": { "errorCode": "rate_limit_exceeded_endpoint" } }),
        ),
        create_audit(
            json!({ "id": "audit-legacy", "projectId": "project-b", "routePolicyId": "policy-2", "apiKeyId": "key-c", "endpointKind": "responses", "routeTrace": { "errorCode": "rate-limit-legacy" } }),
        ),
        create_audit(json!({ "id": "audit-nonrl", "routeTrace": { "errorCode": "timeout" } })),
    ];
    let summary = build_rate_limit_hotspot_summary(&rows);
    assert_eq!(summary.total_rate_limited_requests, 3);
    assert_eq!(
        summary
            .by_route_policy_id
            .first()
            .map(|bucket| bucket.key.as_str()),
        Some("policy-2")
    );
    assert_eq!(
        summary
            .by_endpoint_kind
            .first()
            .map(|bucket| bucket.key.as_str()),
        Some("responses")
    );
}

#[test]
fn hotspot_trend_reports_latest_vs_previous_bucket() {
    let rows = vec![
        create_audit(json!({ "id": "latest-1", "createdAt": "2026-04-07T11:05:00Z" })),
        create_audit(
            json!({ "id": "latest-2", "endpointKind": "responses", "routeTrace": { "errorCode": "rate_limit_exceeded_endpoint" }, "createdAt": "2026-04-07T11:30:00Z" }),
        ),
        create_audit(
            json!({ "id": "previous-1", "projectId": "project-b", "apiKeyId": "key-b", "requestedModel": "gpt-4o", "resolvedModel": "gpt-4o", "endpointKind": "responses", "createdAt": "2026-04-07T10:15:00Z" }),
        ),
    ];
    let report = build_rate_limit_hotspot_trend_report(
        "2026-04-07T12:00:00Z".to_string(),
        GatewayRateLimitHotspotFilterView {
            project_id: None,
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            response_id: None,
            protocol_family: None,
            endpoint_kind: None,
            error_code: None,
            created_from: None,
            created_to: None,
            limit: 1000,
            window_size: 3,
            bucket_size_minutes: 60,
        },
        &rows,
    );
    assert_eq!(report.matched_requests_count, 3);
    assert_eq!(report.points[0].bucket_start_at, "2026-04-07T11:00:00Z");
    assert_eq!(report.points[1].bucket_start_at, "2026-04-07T10:00:00Z");
    assert_eq!(
        report
            .summary
            .as_ref()
            .and_then(|summary| summary.total_rate_limited_requests.latest_value),
        Some(2.0)
    );
    assert_eq!(
        report
            .summary
            .as_ref()
            .and_then(|summary| summary.latest_top_api_key_key.as_deref()),
        Some("key-a")
    );
}

#[test]
fn hotspot_anomaly_report_detects_spike_and_key_hotspot() {
    let rows = vec![
        create_audit(json!({ "id": "latest-1", "createdAt": "2026-04-07T11:01:00Z" })),
        create_audit(json!({ "id": "latest-2", "createdAt": "2026-04-07T11:02:00Z" })),
        create_audit(json!({ "id": "latest-3", "createdAt": "2026-04-07T11:03:00Z" })),
        create_audit(json!({ "id": "latest-4", "createdAt": "2026-04-07T11:04:00Z" })),
        create_audit(json!({ "id": "latest-5", "createdAt": "2026-04-07T11:05:00Z" })),
        create_audit(
            json!({ "id": "previous-1", "apiKeyId": "key-b", "createdAt": "2026-04-07T10:10:00Z" }),
        ),
    ];
    let trend = build_rate_limit_hotspot_trend_report(
        "2026-04-07T12:00:00Z".to_string(),
        GatewayRateLimitHotspotFilterView {
            project_id: None,
            route_policy_id: None,
            provider_account_id: None,
            session_id: None,
            api_key_id: None,
            response_id: None,
            protocol_family: None,
            endpoint_kind: None,
            error_code: None,
            created_from: None,
            created_to: None,
            limit: 1000,
            window_size: 2,
            bucket_size_minutes: 60,
        },
        &rows,
    );
    let report = build_rate_limit_hotspot_anomaly_report(
        "2026-04-07T12:00:00Z".to_string(),
        trend,
        "aggressive".to_string(),
        build_rate_limit_hotspot_anomaly_thresholds(
            "aggressive",
            GatewayRateLimitHotspotAnomalyOverrides {
                total_rate_limited_requests_warning_threshold: Some(4.0),
                total_rate_limited_requests_critical_threshold: Some(5.0),
                total_rate_limited_requests_delta_ratio_threshold: Some(0.1),
                top_api_key_share_warning_threshold: Some(0.7),
                top_api_key_share_critical_threshold: Some(0.9),
                ..Default::default()
            },
        ),
    );
    assert!(report
        .anomalies
        .iter()
        .any(|item| item.code == "rate_limit_request_spike"));
    let api_key_hotspot = report
        .anomalies
        .iter()
        .find(|item| item.code == "rate_limit_api_key_hotspot");
    assert_eq!(
        api_key_hotspot.and_then(|item| item.entity_key.as_deref()),
        Some("key-a")
    );
    assert_eq!(
        api_key_hotspot.map(|item| item.severity.as_str()),
        Some("critical")
    );
}

#[test]
fn hotspot_snapshot_inventory_merges_snapshot_buckets() {
    let summary = build_rate_limit_hotspot_snapshot_inventory_summary(&[
        create_snapshot(
            "snapshot-1",
            "2026-04-07T12:00:00Z",
            Some("daily"),
            5,
            "rate_limit_exceeded_api_key",
            "project-a",
            "key-a",
            "responses",
        ),
        create_snapshot(
            "snapshot-2",
            "2026-04-08T12:00:00Z",
            Some("weekly"),
            2,
            "rate_limit_exceeded_model",
            "project-b",
            "key-b",
            "chat.completions",
        ),
    ]);
    assert_eq!(summary.total_snapshots, 2);
    assert_eq!(summary.total_rate_limited_requests, 7);
    assert_eq!(
        summary.by_label.first().map(|bucket| bucket.key.as_str()),
        Some("daily")
    );
    assert_eq!(
        summary
            .by_project
            .iter()
            .find(|bucket| bucket.key == "project-b")
            .map(|bucket| bucket.count),
        Some(2)
    );
}

#[test]
fn hotspot_snapshot_trend_report_summarizes_latest_vs_previous() {
    let snapshots = vec![
        create_snapshot(
            "snapshot-1",
            "2026-04-08T12:00:00Z",
            Some("daily"),
            10,
            "rate_limit_exceeded_api_key",
            "project-a",
            "key-a",
            "responses",
        ),
        create_snapshot(
            "snapshot-2",
            "2026-04-07T12:00:00Z",
            Some("daily"),
            4,
            "rate_limit_exceeded_model",
            "project-a",
            "key-a",
            "responses",
        ),
    ];
    let report = build_rate_limit_hotspot_snapshot_trend_report(
        "2026-04-08T13:00:00Z".to_string(),
        GatewayRateLimitHotspotSnapshotReportFilterView {
            label: Some("daily".to_string()),
            project_id: Some("project-a".to_string()),
            route_policy_id: Some("policy-a".to_string()),
            api_key_id: Some("key-a".to_string()),
            endpoint_kind: Some("responses".to_string()),
            created_from: None,
            created_to: None,
        },
        10,
        build_rate_limit_hotspot_snapshot_inventory_summary(&snapshots),
        snapshots
            .into_iter()
            .map(build_rate_limit_hotspot_snapshot_trend_point)
            .collect(),
    );
    assert_eq!(report.matched_snapshots_count, 2);
    assert_eq!(
        report
            .summary
            .as_ref()
            .and_then(|item| item.latest_snapshot_id.as_deref()),
        Some("snapshot-1")
    );
    assert_eq!(
        report
            .summary
            .as_ref()
            .and_then(|item| item.total_rate_limited_requests.latest_value),
        Some(10.0)
    );
    assert_eq!(
        report
            .summary
            .as_ref()
            .and_then(|item| item.total_rate_limited_requests.previous_value),
        Some(4.0)
    );
}
