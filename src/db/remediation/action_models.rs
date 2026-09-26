use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationQueueFilters {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub status: Option<String>,
    pub follow_up_status: Option<String>,
    pub escalation_status: Option<String>,
    pub code: Option<String>,
    pub severity: Option<String>,
    pub action_key: Option<String>,
    pub execution_mode: Option<String>,
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationActionView {
    pub action_key: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub priority: String,
    pub route_policy_id: Option<String>,
    pub executable: bool,
    pub execution_mode: String,
    pub default_execution_input: Option<Value>,
    pub recommended_changes: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput {
    pub provider_max_concurrent_requests: Option<i32>,
    pub pre_stream_fallback_enabled: Option<bool>,
    pub allowed_provider_account_ids: Option<Vec<String>>,
    pub project_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub api_key_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub model_rate_limit_key: Option<String>,
    pub model_rate_limit: Option<super::GatewayRateLimitDefinition>,
    pub endpoint_rate_limit_key: Option<String>,
    pub endpoint_rate_limit: Option<super::GatewayRateLimitDefinition>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteGatewayAnalysisAnomalyIncidentRemediationInput {
    pub action_key: String,
    pub dry_run: Option<bool>,
    pub note: Option<String>,
    pub incident_follow_up: Option<GatewayAnalysisAnomalyIncidentFollowUpInput>,
    pub route_policy_patch: Option<GatewayAnalysisAnomalyIncidentRemediationRoutePolicyPatchInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationPlanView {
    pub generated_at: String,
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub overview: String,
    pub actions: Vec<GatewayAnalysisAnomalyIncidentRemediationActionView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationQueueItemView {
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub action: GatewayAnalysisAnomalyIncidentRemediationActionView,
    pub remediation_due: bool,
    pub next_execution_status: Option<String>,
    pub next_run_due_at: Option<String>,
    pub blocked_reason: Option<String>,
    pub latest_run: Option<GatewayAnalysisAnomalyIncidentRemediationRunView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentRemediationQueueView {
    pub generated_at: String,
    pub limit: usize,
    pub due_only: bool,
    pub item_count: usize,
    pub due_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyIncidentRemediationQueueItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentAlertQueueItemView {
    pub incident: GatewayAnalysisAnomalyIncidentView,
    pub policy: Option<Value>,
    pub route_policy: Option<GatewayRoutePolicyView>,
    pub alert_interval_minutes: i32,
    pub alert_due: bool,
    pub next_alert_due_at: Option<String>,
    pub notify_operators: bool,
    pub notify_owner: bool,
    pub alert_level: i32,
    pub webhook_severity: String,
    pub remediation_action_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentAlertQueueView {
    pub generated_at: String,
    pub limit: usize,
    pub due_only: bool,
    pub incident_count: usize,
    pub due_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyIncidentAlertQueueItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationSweepItemView {
    pub incident_id: String,
    pub action_key: String,
    pub status: String,
    pub execution_status: Option<String>,
    pub run_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyRemediationSweepView {
    pub started_at: String,
    pub completed_at: String,
    pub limit: usize,
    pub attempted_count: usize,
    pub dry_run_count: usize,
    pub applied_count: usize,
    pub error_count: usize,
    pub skipped_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyRemediationSweepItemView>,
}
