use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
    pub improved_runs: usize,
    pub regressed_runs: usize,
    pub neutral_runs: usize,
    pub unavailable_runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationActionEffectivenessView {
    pub action_key: String,
    pub run_count: usize,
    pub impacted_run_count: usize,
    pub unavailable_run_count: usize,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
    pub generated_at: String,
    pub window_minutes: i32,
    pub total_runs: usize,
    pub impacted_runs: usize,
    pub unavailable_runs: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    pub actions: Vec<GatewayAnalysisAnomalyRemediationActionEffectivenessView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
    pub snapshot_id: Option<String>,
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub dry_run: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: usize,
    pub lookback_hours: Option<i32>,
    pub window_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub created_at: String,
    pub object_key: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView,
    pub summary: GatewayAnalysisAnomalyRemediationEffectivenessSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
    pub total_snapshots: usize,
    pub total_runs: usize,
    pub total_impacted_runs: usize,
    pub total_unavailable_runs: usize,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_label: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView {
    pub label: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
    pub improved_rate: Option<f64>,
    pub regressed_rate: Option<f64>,
    pub neutral_rate: Option<f64>,
    pub unavailable_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
    pub snapshot: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    pub total_runs: usize,
    pub impacted_run_rate: Option<f64>,
    pub unavailable_run_rate: Option<f64>,
    pub completion_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub request_artifact_coverage:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub response_artifact_coverage:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub first_token_latency_ms_avg:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView {
    pub latest_snapshot_id: Option<String>,
    pub previous_snapshot_id: Option<String>,
    pub total_runs: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub impacted_run_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub unavailable_run_rate: GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub completion_rate_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub failure_rate_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub request_artifact_coverage_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub response_artifact_coverage_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub first_token_latency_ms_avg_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
    pub total_tokens_per_sample_regressed:
        GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView {
    pub generated_at: String,
    pub filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView,
    pub matched_snapshots_count: usize,
    pub window_size: usize,
    pub inventory_summary:
        GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView,
    pub points: Vec<GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView>,
    pub summary: Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView>,
}
