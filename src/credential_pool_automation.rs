use crate::routing::config::ProviderCredentialYaml;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
mod actions;
mod archive;
mod driver;
mod inventory;
mod reconciliation;
mod refill;
mod registry;
mod runtime;
#[cfg(test)]
use crate::routing::config::ProviderConfigYaml;
pub use actions::{
    run_provider_credential_pool_automation, run_provider_credential_pool_prune,
    start_credential_pool_automation_task, sweep_credential_pool_automation_once,
};
#[cfg(test)]
use archive::{
    archive_pruned_credentials_in_directory, archived_credential_count_in_directory,
    purge_credential_archive_directory, safe_archive_path_segment,
};
pub use archive::{
    archived_provider_credential_count, provider_credential_archive_path,
    provider_credential_storage_path, provider_credential_storage_root_path,
    purge_provider_credential_archive,
};
#[cfg(test)]
use driver::execute_driver;
#[cfg(test)]
use inventory::{identity_category_requested_count, prune_credentials};
pub use refill::collect_refill_credentials_from_driver;
#[cfg(test)]
use registry::validate_driver;
#[cfg(test)]
use std::{collections::HashSet, fs};
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct CredentialPoolAutomationConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    pub driver_config_path: Option<PathBuf>,
    pub script_root: Option<PathBuf>,
    pub default_timeout_secs: u64,
    pub refill_queue_enabled: bool,
    pub refill_notification_interval_secs: u64,
    pub refill_task_ttl_secs: u64,
    pub refill_default_lease_secs: u64,
    pub refill_max_lease_secs: u64,
    pub refill_stream_max_len: usize,
}

impl Default for CredentialPoolAutomationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: 60,
            driver_config_path: None,
            script_root: None,
            default_timeout_secs: 60,
            refill_queue_enabled: false,
            refill_notification_interval_secs: 30,
            refill_task_ttl_secs: 7 * 24 * 60 * 60,
            refill_default_lease_secs: 300,
            refill_max_lease_secs: 3_600,
            refill_stream_max_len: 10_000,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverRegistryDocument {
    #[serde(default)]
    drivers: Vec<CredentialAutomationDriver>,
}

#[derive(Clone, Debug, Deserialize)]
struct CredentialAutomationDriver {
    id: String,
    #[serde(default)]
    provider_ids: Vec<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(flatten)]
    transport: CredentialAutomationDriverTransport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum CredentialAutomationDriverTransport {
    Script {
        script: String,
    },
    Http {
        endpoint: String,
        #[serde(default)]
        secret_env: Option<String>,
    },
}

impl CredentialAutomationDriver {
    fn mode(&self) -> &'static str {
        match self.transport {
            CredentialAutomationDriverTransport::Script { .. } => "script",
            CredentialAutomationDriverTransport::Http { .. } => "http",
        }
    }

    fn supports_provider(&self, provider_id: &str) -> bool {
        self.provider_ids.iter().any(|entry| entry == provider_id)
    }
}

#[derive(Clone, Debug, Default)]
struct DriverRegistry {
    drivers: BTreeMap<String, CredentialAutomationDriver>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPoolDriverView {
    pub id: String,
    pub mode: String,
    pub provider_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialPoolAutomationView {
    pub provider_id: String,
    pub provider_label: String,
    pub target_size: usize,
    pub credential_count: usize,
    pub active_credential_count: usize,
    pub auto_refill_enabled: bool,
    pub auto_prune_enabled: bool,
    pub permanent_delete_enabled: bool,
    pub driver_id: Option<String>,
    pub driver_mode: Option<String>,
    pub driver_configured: bool,
    pub state: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_action: Option<String>,
    pub created_count: usize,
    pub pruned_count: usize,
    pub message: Option<String>,
    pub revision_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderRunState {
    state: String,
    last_run_at: Option<String>,
    next_run_at: Option<String>,
    last_action: Option<String>,
    created_count: usize,
    pruned_count: usize,
    message: Option<String>,
    revision_id: Option<String>,
}

pub struct CredentialPoolAutomationRuntime {
    config: CredentialPoolAutomationConfig,
    registry: DriverRegistry,
    states: RwLock<BTreeMap<String, ProviderRunState>>,
    provider_locks: DashMap<String, Arc<Mutex<()>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverRequest {
    run_id: String,
    action: &'static str,
    provider: DriverProviderContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    refill: Option<DriverRefillContext>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverRefillContext {
    task_id: String,
    requested_count: usize,
    artifact_reference: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverProviderContext {
    id: String,
    label: String,
    target_size: usize,
    credential_count: usize,
    active_credential_count: usize,
    requested_count: usize,
    auto_refill_enabled: bool,
    auto_prune_enabled: bool,
    identity_categories: Vec<serde_json::Value>,
    credentials: Vec<DriverCredentialContext>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverCredentialContext {
    id: String,
    account_name: Option<String>,
    enabled: bool,
    identity_category_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverResponse {
    #[serde(default)]
    credentials: Vec<ProviderCredentialYaml>,
    #[serde(default)]
    prune: Vec<DriverPruneDecision>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DriverPruneDecision {
    credential_id: String,
    classification: PermanentPruneClassification,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PermanentPruneClassification {
    PermanentAuthFailure,
    AccountDeleted,
    PermanentUpstreamRejection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialPoolAutomationAction {
    Reconcile,
    Prune,
}

impl CredentialPoolAutomationAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reconcile => "reconcile",
            Self::Prune => "prune",
        }
    }
}

#[derive(Debug)]
struct ReconcileOutcome {
    created_count: usize,
    pruned_count: usize,
    message: Option<String>,
    revision_id: Option<String>,
}
