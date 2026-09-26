use crate::routing::candidate::ProviderAccountPayload;
use serde::{Deserialize, Serialize};
use serde_json::Value;
mod accio;
mod accio_http;
mod aggregation;
mod cache;
mod codex;
mod generic_balance;
mod http_contract;
mod payload_fields;
mod refresh;
mod refresh_clock;
mod refresh_lock;
#[cfg(test)]
use accio_http::build_accio_control_plane_headers;
pub use aggregation::{aggregate_provider_quota_snapshots, quota_to_balance_status};
pub use cache::{read_cached_provider_quota_snapshot, read_cached_runtime_quota_snapshot};
#[cfg(test)]
use codex::codex_window_to_view;
#[cfg(test)]
use payload_fields::{find_numeric_field, round_percent};
pub(crate) use refresh::get_or_refresh_runtime_quota_snapshot_with_cache;
pub use refresh::{
    get_or_refresh_provider_quota_snapshot, get_or_refresh_runtime_quota_snapshot,
    refresh_provider_quota_snapshot, refresh_runtime_quota_snapshot,
};
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_error_contract;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderQuotaWindowView {
    pub key: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub remaining_ratio: Option<f64>,
    pub limit_window_seconds: Option<i64>,
    pub reset_at: Option<String>,
    pub reset_after_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderQuotaView {
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub provider_type: String,
    pub source: String,
    pub status: String,
    pub ready: bool,
    pub checked_at: String,
    pub next_check_at: String,
    pub next_reset_at: Option<String>,
    pub plan_type: Option<String>,
    pub representative_claim: Option<String>,
    pub windows: Vec<GatewayProviderQuotaWindowView>,
    pub error: Option<String>,
    pub raw_data: Value,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    #[serde(default)]
    plan_type: Option<String>,
    #[serde(default)]
    rate_limit: Option<CodexRateLimitInfo>,
    #[serde(default)]
    code_review_rate_limit: Option<CodexRateLimitInfo>,
    #[serde(default)]
    spend_control: Option<CodexSpendControl>,
    #[serde(default)]
    rate_limit_reached_type: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CodexSpendControl {
    #[serde(default)]
    reached: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimitInfo {
    #[serde(default)]
    allowed: Option<bool>,
    #[serde(default)]
    limit_reached: Option<bool>,
    #[serde(default)]
    primary_window: Option<CodexUsageWindow>,
    #[serde(default)]
    secondary_window: Option<CodexUsageWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexUsageWindow {
    #[serde(default)]
    used_percent: Option<f64>,
    #[serde(default)]
    reset_at: Option<i64>,
    #[serde(default)]
    reset_after_seconds: Option<i64>,
    #[serde(default)]
    limit_window_seconds: Option<i64>,
}

pub fn provider_supports_quota(payload: &ProviderAccountPayload) -> bool {
    crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload)
        || payload.balance_path.is_some()
        || payload.canonical_adapter() == "accio_compatible"
}
