use crate::routing::candidate::ProviderExecutionMode;
use serde::{Deserialize, Serialize};
mod health;
mod lease_release;
mod leases;
mod nodes;
mod normalization;
mod slots;
mod storage;
mod timestamps;
pub use health::get_browser_executor_health;
#[cfg(test)]
use health::is_challenge_open;
#[cfg(test)]
use leases::choose_release_status;
pub use leases::{
    acquire_browser_capability_lease, list_browser_capability_leases,
    release_browser_capability_lease,
};
pub use nodes::{heartbeat_browser_executor_node, list_browser_executor_nodes};
pub use slots::{list_browser_capability_slots, upsert_browser_capability_slot};
#[cfg(test)]
mod tests;

const DEFAULT_NODE_TTL_SECONDS: u64 = 180;
const DEFAULT_SLOT_TTL_SECONDS: u64 = 600;
const DEFAULT_LEASE_TTL_SECONDS: u64 = 300;
const RELEASED_LEASE_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserExecutorNodeStatus {
    Active,
    Degraded,
    Draining,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserCapabilitySlotStatus {
    Warming,
    Warm,
    Hot,
    Busy,
    Cooling,
    Expired,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeView {
    pub node_id: String,
    pub status: BrowserExecutorNodeStatus,
    pub capabilities: Vec<String>,
    pub last_heartbeat_at: String,
    pub version: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotView {
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub adapter: String,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    pub status: BrowserCapabilitySlotStatus,
    pub runtime_state_object_key: Option<String>,
    pub account_name: Option<String>,
    pub last_warm_at: Option<String>,
    pub last_used_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub degradation_reasons: Option<Vec<String>>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseView {
    pub lease_id: String,
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub request_audit_id: Option<String>,
    pub project_id: Option<String>,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub released_at: Option<String>,
    pub release_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeHealthView {
    pub node_id: String,
    pub total_slots: usize,
    pub warm_slots: usize,
    pub busy_slots: usize,
    pub cooling_slots: usize,
    pub degraded_slots: usize,
    pub challenge_open_slots: usize,
    pub last_error_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorProviderHealthView {
    pub provider_account_id: String,
    pub total_slots: usize,
    pub warm_slots: usize,
    pub busy_slots: usize,
    pub cooling_slots: usize,
    pub degraded_slots: usize,
    pub challenge_open_slots: usize,
    pub last_error_summary: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorHealthView {
    pub generated_at: String,
    pub total_nodes: usize,
    pub total_slots: usize,
    pub total_warm_slots: usize,
    pub total_busy_slots: usize,
    pub total_cooling_slots: usize,
    pub total_degraded_slots: usize,
    pub total_challenge_open_slots: usize,
    pub nodes: Vec<BrowserExecutorNodeHealthView>,
    pub providers: Vec<BrowserExecutorProviderHealthView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorNodeHeartbeatInput {
    pub node_id: String,
    #[serde(default)]
    pub status: Option<BrowserExecutorNodeStatus>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotUpsertInput {
    pub slot_id: String,
    pub node_id: String,
    pub provider_account_id: String,
    pub adapter: String,
    pub endpoint_kind: String,
    pub execution_mode: ProviderExecutionMode,
    #[serde(default)]
    pub status: Option<BrowserCapabilitySlotStatus>,
    #[serde(default)]
    pub runtime_state_object_key: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub last_warm_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub last_failure_at: Option<String>,
    #[serde(default)]
    pub degradation_reasons: Option<Vec<String>>,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilitySlotFilters {
    pub node_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub execution_mode: Option<ProviderExecutionMode>,
    pub status: Option<BrowserCapabilitySlotStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseAcquireInput {
    pub provider_account_id: String,
    pub endpoint_kind: String,
    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,
    #[serde(default)]
    pub request_audit_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub lease_ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseReleaseInput {
    pub lease_id: String,
    #[serde(default)]
    pub release_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCapabilityLeaseFilters {
    pub provider_account_id: Option<String>,
    pub node_id: Option<String>,
    pub endpoint_kind: Option<String>,
    pub request_audit_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserExecutorHealthFilters {
    pub provider_account_id: Option<String>,
    pub node_id: Option<String>,
}
