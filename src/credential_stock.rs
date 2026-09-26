use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;

mod evaluation;
mod policy;
mod quota_fields;
mod signal_store;
mod signals;
mod status;
mod validation;

use evaluation::{combined_needs_replenishment, combined_stock_severity};
pub use evaluation::{evaluate_credential_count, evaluate_token_window};
pub use policy::{list_credential_stock_policies, upsert_credential_stock_policy};
use policy::{policy_view_from_row, query_policy_rows};
use quota_fields::remaining_tokens_for_policy_window;
use signal_store::{insert_signal_event, mark_policy_signal_sent, mark_signal_event_published};
pub use signals::{
    build_signal_key, start_credential_stock_monitor_task, sweep_credential_stock_signals_once,
};
pub use status::get_credential_stock_status_report;
#[cfg(test)]
use status::metric_requires_quota_snapshot;
use validation::{
    normalize_credential_material_kind, normalize_optional_key, normalize_required_key,
    normalize_required_text, resolve_provider_supply_metric_kind, trim_nonempty,
};

const DEFAULT_SIGNAL_STREAM: &str = "gw:credential-stock:signals";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockSeverity {
    Healthy,
    Warning,
    Critical,
    Overstock,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountWatermark {
    pub min: Option<usize>,
    pub target: Option<usize>,
    pub max: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountStockEvaluation {
    pub usable_credential_count: usize,
    pub min_credential_count: Option<usize>,
    pub target_credential_count: Option<usize>,
    pub max_credential_count: Option<usize>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub deficit_to_min: usize,
    pub deficit_to_target: usize,
    pub suggested_credential_top_up_count: usize,
    pub excess_over_max: usize,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenWindowWatermark {
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub target_credential_count: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusFilters {
    pub stock_class_key: Option<String>,
    pub service_provider_key: Option<String>,
    pub implementation_line_key: Option<String>,
    pub provider_surface_key: Option<String>,
    pub credential_material_kind: Option<String>,
    pub provider_account_id: Option<String>,
    pub needs_replenishment: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCredentialStockPolicyInput {
    #[serde(default)]
    pub id: Option<String>,
    pub stock_class_key: String,
    pub display_name: String,
    pub service_provider_key: String,
    pub implementation_line_key: String,
    pub provider_surface_key: String,
    pub credential_material_kind: String,
    #[serde(default)]
    pub provider_account_id: Option<String>,
    #[serde(default)]
    pub provider_adapter: Option<String>,
    #[serde(default)]
    pub selector: Option<Value>,
    #[serde(default, rename = "providerSupplyMetricKind")]
    pub provider_supply_metric_kind: Option<String>,
    #[serde(default, rename = "metricKind")]
    pub legacy_metric_kind: Option<String>,
    #[serde(default)]
    pub token_window_key: Option<String>,
    #[serde(default)]
    pub token_window_seconds: Option<i64>,
    #[serde(default)]
    pub min_credential_count: Option<i32>,
    #[serde(default)]
    pub target_credential_count: Option<i32>,
    #[serde(default)]
    pub max_credential_count: Option<i32>,
    #[serde(default)]
    pub min_average_available_tokens: Option<i64>,
    #[serde(default)]
    pub target_average_available_tokens: Option<i64>,
    #[serde(default)]
    pub signal_enabled: Option<bool>,
    #[serde(default)]
    pub signal_stream: Option<String>,
    #[serde(default)]
    pub signal_cooldown_secs: Option<i64>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockPolicyView {
    pub id: String,
    pub stock_class_key: String,
    pub display_name: String,
    pub service_provider_key: String,
    pub implementation_line_key: String,
    pub provider_surface_key: String,
    pub credential_material_kind: String,
    pub provider_account_id: Option<String>,
    pub provider_adapter: Option<String>,
    pub selector: Value,
    #[serde(rename = "providerSupplyMetricKind")]
    pub provider_supply_metric_kind: String,
    #[serde(rename = "metricKind")]
    pub metric_kind: String,
    pub token_window_key: Option<String>,
    pub token_window_seconds: Option<i64>,
    pub min_credential_count: Option<i32>,
    pub target_credential_count: Option<i32>,
    pub max_credential_count: Option<i32>,
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub signal_enabled: bool,
    pub signal_stream: String,
    pub signal_cooldown_secs: i64,
    pub enabled: bool,
    pub last_signal_key: Option<String>,
    pub last_signal_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusView {
    pub policy: CredentialStockPolicyView,
    pub provider_supply_metric_kind: String,
    #[serde(rename = "metricKind")]
    pub metric_kind: String,
    pub token_window_key: Option<String>,
    pub token_window_seconds: Option<i64>,
    pub usable_credential_count: usize,
    pub active_credential_count: usize,
    pub cooling_credential_count: usize,
    pub disabled_credential_count: usize,
    pub archived_credential_count: usize,
    pub unknown_status_credential_count: usize,
    pub count_evaluation: CountStockEvaluation,
    pub token_evaluation: Option<TokenWindowStockEvaluation>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub suggested_credential_top_up_count: usize,
    pub signal_key: String,
    pub signal_payload: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockStatusReport {
    pub policies: Vec<CredentialStockStatusView>,
    pub summary: CredentialStockSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSummaryView {
    pub total_policy_count: usize,
    pub enabled_policy_count: usize,
    pub needs_replenishment_policy_count: usize,
    pub critical_policy_count: usize,
    pub warning_policy_count: usize,
    pub healthy_policy_count: usize,
    pub overstock_policy_count: usize,
    pub total_suggested_credential_top_up_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSignalSweepResult {
    pub scanned_policy_count: usize,
    pub replenishment_policy_count: usize,
    pub emitted_signal_count: usize,
    pub suppressed_signal_count: usize,
    pub redis_error_count: usize,
    pub signals: Vec<CredentialStockSignalEventView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStockSignalEventView {
    pub id: String,
    pub policy_id: String,
    pub stock_class_key: String,
    pub signal_key: String,
    pub severity: StockSeverity,
    pub stream: String,
    pub payload: Value,
    pub published_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockPolicyRow {
    id: String,
    stock_class_key: String,
    display_name: String,
    service_provider_key: String,
    implementation_line_key: String,
    provider_surface_key: String,
    credential_material_kind: String,
    provider_account_id: Option<String>,
    provider_adapter: Option<String>,
    selector: sqlx::types::Json<Value>,
    metric_kind: String,
    token_window_key: Option<String>,
    token_window_seconds: Option<i64>,
    min_credential_count: Option<i32>,
    target_credential_count: Option<i32>,
    max_credential_count: Option<i32>,
    min_average_available_tokens: Option<i64>,
    target_average_available_tokens: Option<i64>,
    signal_enabled: bool,
    signal_stream: String,
    signal_cooldown_secs: i64,
    enabled: bool,
    last_signal_key: Option<String>,
    last_signal_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockCredentialRow {
    provider_credential_id: String,
    status: String,
    archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, FromRow)]
struct CredentialStockSignalEventRow {
    id: String,
    policy_id: String,
    stock_class_key: String,
    signal_key: String,
    severity: String,
    stream: String,
    payload: sqlx::types::Json<Value>,
    published_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenWindowStockEvaluation {
    pub known_token_credential_count: usize,
    pub unknown_token_credential_count: usize,
    pub average_available_tokens: Option<i64>,
    pub min_average_available_tokens: Option<i64>,
    pub target_average_available_tokens: Option<i64>,
    pub target_credential_count: Option<usize>,
    pub needs_replenishment: bool,
    pub severity: StockSeverity,
    pub deficit_to_min_average_tokens: i64,
    pub deficit_to_target_average_tokens: i64,
    pub suggested_credential_top_up_count: usize,
}

#[cfg(test)]
mod tests;
