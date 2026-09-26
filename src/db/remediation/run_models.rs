use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunFilters {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub dry_run: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationRunView {
    pub id: String,
    pub incident_id: String,
    pub policy_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub action_key: String,
    pub title: String,
    pub execution_mode: String,
    pub status: String,
    pub dry_run: bool,
    pub actor_user_id: String,
    pub note: Option<String>,
    pub input: Option<Value>,
    pub result: Option<Value>,
    pub before_incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub after_incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub before_route_policy: Option<GatewayRoutePolicyView>,
    pub after_route_policy: Option<GatewayRoutePolicyView>,
    pub error_summary: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunSummaryView {
    pub total_runs: usize,
    pub dry_run_runs: usize,
    pub applied_runs: usize,
    pub failed_runs: usize,
    pub distinct_incident_count: usize,
    pub route_policy_changed_runs: usize,
    pub incident_changed_runs: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_execution_mode: Vec<GatewaySummaryBucketKeyView>,
    pub by_action_key: Vec<GatewaySummaryBucketKeyView>,
    pub by_policy_id: Vec<GatewaySummaryBucketKeyView>,
    pub by_route_policy_id: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactMetricView {
    pub before_value: Option<f64>,
    pub after_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunImpactView {
    pub generated_at: String,
    pub run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    pub incident: Option<GatewayAnalysisAnomalyIncidentView>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub anchor_at: String,
    pub window_minutes: i32,
    pub before_window: GatewayAnalysisAnomalyRemediationImpactWindowView,
    pub after_window: GatewayAnalysisAnomalyRemediationImpactWindowView,
    pub metrics: GatewayAnalysisAnomalyRemediationRunImpactMetricsView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactWindowView {
    pub started_at: String,
    pub ended_at: String,
    pub summary: GatewayAnalysisSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationRunImpactMetricsView {
    pub completion_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub failure_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub cancellation_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub stream_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub tool_request_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub tool_response_rate: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub request_artifact_coverage: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub response_artifact_coverage: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub prompt_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub completion_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub total_tokens_per_sample: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub request_text_chars_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub response_text_chars_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub first_token_latency_ms_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
    pub stream_chunk_count_avg: GatewayAnalysisAnomalyRemediationImpactMetricView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationImpactCaptureView {
    pub run: GatewayAnalysisAnomalyIncidentRemediationRunView,
    pub impact: GatewayAnalysisAnomalyRemediationRunImpactView,
}
