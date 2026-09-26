use super::{
    format_timestamp, trimmed_owned_ref, GatewayAnalysisAnomalyIncidentRow,
    GatewayAnalysisExportAnomalyView,
};
use serde_json::{json, Value};
use time::OffsetDateTime;

pub(super) fn build_analysis_export_incident_snapshot_metadata(
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    status: &str,
    owner_user_id: Option<&str>,
    follow_up_status: Option<&str>,
    sync_hit_count: i32,
    escalation_status: Option<&str>,
    escalated_at: Option<&OffsetDateTime>,
    escalation_reason: Option<&str>,
) -> Value {
    json!({
        "policyId": policy_id.and_then(trimmed_owned_ref),
        "projectId": project_id.and_then(trimmed_owned_ref),
        "routePolicyId": route_policy_id.and_then(trimmed_owned_ref),
        "tag": tag.and_then(trimmed_owned_ref),
        "textMode": text_mode.and_then(trimmed_owned_ref),
        "code": anomaly.code,
        "severity": anomaly.severity,
        "status": status,
        "ownerUserId": owner_user_id.and_then(trimmed_owned_ref),
        "followUpStatus": follow_up_status.and_then(trimmed_owned_ref),
        "syncHitCount": sync_hit_count,
        "escalationStatus": escalation_status.and_then(trimmed_owned_ref),
        "escalatedAt": escalated_at.map(|value| format_timestamp(*value)),
        "escalationReason": escalation_reason.and_then(trimmed_owned_ref),
        "lastAlertAttemptAt": Value::Null,
        "lastAlertedAt": Value::Null,
        "lastAlertSeverity": Value::Null,
        "alertDeliveryCount": Value::Null,
        "latestExportId": anomaly.latest_export_id.clone(),
        "previousExportId": anomaly.previous_export_id.clone(),
        "latestValue": anomaly.latest_value,
        "previousValue": anomaly.previous_value,
        "deltaValue": anomaly.delta_value,
        "deltaRatio": anomaly.delta_ratio,
        "thresholdValue": anomaly.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}

pub(super) fn build_row_analysis_export_anomaly_view(
    row: &GatewayAnalysisAnomalyIncidentRow,
) -> GatewayAnalysisExportAnomalyView {
    GatewayAnalysisExportAnomalyView {
        code: row.code.clone(),
        severity: row.severity.clone(),
        message: row.summary.clone(),
        latest_export_id: row.latest_export_id.clone(),
        previous_export_id: row.previous_export_id.clone(),
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value,
    }
}
