use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicyFilters {
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub auto_sync_enabled: Option<bool>,
    pub auto_escalate_enabled: Option<bool>,
    pub auto_remediation_enabled: Option<bool>,
    pub alerting_enabled: Option<bool>,
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicyView {
    pub id: String,
    pub name: String,
    pub status: String,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub profile_key: String,
    pub thresholds: Value,
    pub auto_sync_enabled: bool,
    pub auto_sync_interval_minutes: Option<i32>,
    pub last_synced_at: Option<String>,
    pub last_sync_status: Option<String>,
    pub last_sync_error: Option<String>,
    pub next_sync_due_at: Option<String>,
    pub sync_due: bool,
    pub auto_escalate_enabled: bool,
    pub escalate_severity_threshold: Option<String>,
    pub escalate_after_sync_count: Option<i32>,
    pub auto_escalate_owner_user_id: Option<String>,
    pub auto_escalate_follow_up_status: Option<String>,
    pub auto_remediation_enabled: bool,
    pub auto_remediation_interval_minutes: Option<i32>,
    pub auto_remediation_dry_run_first: bool,
    pub auto_remediation_action_keys: Option<Vec<String>>,
    pub auto_remediation_max_apply_runs_per_incident: Option<i32>,
    pub auto_remediation_require_alert_before_apply: bool,
    pub auto_remediation_freeze_on_provider_health_degrade: bool,
    pub alerting_enabled: bool,
    pub alert_interval_minutes: Option<i32>,
    pub notify_operators_on_escalation: bool,
    pub notify_owner_on_escalation: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySummaryView {
    pub total_policies: usize,
    pub enabled_policies: usize,
    pub disabled_policies: usize,
    pub auto_sync_enabled_policies: usize,
    pub auto_escalate_enabled_policies: usize,
    pub auto_remediation_enabled_policies: usize,
    pub alerting_enabled_policies: usize,
    pub due_policies: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_sync_status: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySyncView {
    pub policy: GatewayAnalysisAnomalyPolicyView,
    pub sync_kind: String,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
    pub sync: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSyncInput {
    pub policy_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub profile_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSyncView {
    pub policy: Option<GatewayAnalysisAnomalyPolicyView>,
    pub sync_kind: String,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
    pub sync: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySweepItemView {
    pub policy_id: String,
    pub policy_name: String,
    pub status: String,
    pub error: Option<String>,
    pub last_synced_at: Option<String>,
    pub next_sync_due_at: Option<String>,
    pub sync_due: bool,
    pub anomaly_count: usize,
    pub opened_incident_count: usize,
    pub updated_incident_count: usize,
    pub resolved_incident_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyPolicySweepView {
    pub started_at: String,
    pub completed_at: String,
    pub limit: usize,
    pub attempted_count: usize,
    pub ok_count: usize,
    pub error_count: usize,
    pub skipped_count: usize,
    pub items: Vec<GatewayAnalysisAnomalyPolicySweepItemView>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertGatewayAnalysisAnomalyPolicyInput {
    pub id: Option<String>,
    pub name: String,
    pub status: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub profile_key: Option<String>,
    pub thresholds: Option<Value>,
    pub auto_sync_enabled: Option<bool>,
    pub auto_sync_interval_minutes: Option<i32>,
    pub auto_escalate_enabled: Option<bool>,
    pub escalate_severity_threshold: Option<String>,
    pub escalate_after_sync_count: Option<i32>,
    pub auto_escalate_owner_user_id: Option<String>,
    pub auto_escalate_follow_up_status: Option<String>,
    pub auto_remediation_enabled: Option<bool>,
    pub auto_remediation_interval_minutes: Option<i32>,
    pub auto_remediation_dry_run_first: Option<bool>,
    pub auto_remediation_action_keys: Option<Vec<String>>,
    pub auto_remediation_max_apply_runs_per_incident: Option<i32>,
    pub auto_remediation_require_alert_before_apply: Option<bool>,
    pub auto_remediation_freeze_on_provider_health_degrade: Option<bool>,
    pub alerting_enabled: Option<bool>,
    pub alert_interval_minutes: Option<i32>,
    pub notify_operators_on_escalation: Option<bool>,
    pub notify_owner_on_escalation: Option<bool>,
}
