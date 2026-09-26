use super::*;

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides {
    pub impacted_run_rate_warning_threshold: Option<f64>,
    pub impacted_run_rate_critical_threshold: Option<f64>,
    pub unavailable_run_rate_warning_threshold: Option<f64>,
    pub unavailable_run_rate_critical_threshold: Option<f64>,
    pub completion_rate_regressed_warning_threshold: Option<f64>,
    pub completion_rate_regressed_critical_threshold: Option<f64>,
    pub failure_rate_regressed_warning_threshold: Option<f64>,
    pub failure_rate_regressed_critical_threshold: Option<f64>,
    pub request_artifact_regressed_warning_threshold: Option<f64>,
    pub request_artifact_regressed_critical_threshold: Option<f64>,
    pub response_artifact_regressed_warning_threshold: Option<f64>,
    pub response_artifact_regressed_critical_threshold: Option<f64>,
    pub first_token_latency_regressed_warning_threshold: Option<f64>,
    pub first_token_latency_regressed_critical_threshold: Option<f64>,
    pub total_tokens_regressed_warning_threshold: Option<f64>,
    pub total_tokens_regressed_critical_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
    pub impacted_run_rate_warning_threshold: f64,
    pub impacted_run_rate_critical_threshold: f64,
    pub unavailable_run_rate_warning_threshold: f64,
    pub unavailable_run_rate_critical_threshold: f64,
    pub completion_rate_regressed_warning_threshold: f64,
    pub completion_rate_regressed_critical_threshold: f64,
    pub failure_rate_regressed_warning_threshold: f64,
    pub failure_rate_regressed_critical_threshold: f64,
    pub request_artifact_regressed_warning_threshold: f64,
    pub request_artifact_regressed_critical_threshold: f64,
    pub response_artifact_regressed_warning_threshold: f64,
    pub response_artifact_regressed_critical_threshold: f64,
    pub first_token_latency_regressed_warning_threshold: f64,
    pub first_token_latency_regressed_critical_threshold: f64,
    pub total_tokens_regressed_warning_threshold: f64,
    pub total_tokens_regressed_critical_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub latest_snapshot_id: Option<String>,
    pub previous_snapshot_id: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    pub profile_key: String,
    pub thresholds: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
    pub latest_snapshot: Option<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>,
    pub previous_snapshot: Option<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>,
    pub trend_summary: Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView>,
    pub anomalies: Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub profile_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView {
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
    pub lookback_hours: Option<i32>,
    pub profile_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView,
    pub report: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView,
}
