use super::inventory::{active_credential_count, normalized_target_size};
use super::{
    CredentialPoolAutomationConfig, CredentialPoolAutomationRuntime, CredentialPoolDriverView,
    DriverRegistry, ProviderCredentialPoolAutomationView, ProviderRunState,
};
use crate::routing::config::ProviderConfigYaml;
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::{Mutex, RwLock};
impl CredentialPoolAutomationRuntime {
    pub fn load(config: &CredentialPoolAutomationConfig) -> anyhow::Result<Self> {
        let registry = DriverRegistry::load(config)?;
        Ok(Self {
            config: config.clone(),
            registry,
            states: RwLock::new(BTreeMap::new()),
            provider_locks: DashMap::new(),
        })
    }

    pub fn disabled() -> Self {
        Self {
            config: CredentialPoolAutomationConfig::default(),
            registry: DriverRegistry::default(),
            states: RwLock::new(BTreeMap::new()),
            provider_locks: DashMap::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.config.interval_secs.max(5))
    }

    pub fn refill_queue_enabled(&self) -> bool {
        self.config.refill_queue_enabled
    }

    pub fn refill_notification_interval(&self) -> Duration {
        Duration::from_secs(self.config.refill_notification_interval_secs.max(5))
    }

    pub fn refill_task_ttl_seconds(&self) -> u64 {
        self.config.refill_task_ttl_secs.max(300)
    }

    pub fn refill_default_lease_seconds(&self) -> u64 {
        self.config.refill_default_lease_secs.clamp(30, 3_600)
    }

    pub fn refill_max_lease_seconds(&self) -> u64 {
        self.config
            .refill_max_lease_secs
            .clamp(self.refill_default_lease_seconds(), 86_400)
    }

    pub fn refill_stream_max_len(&self) -> usize {
        self.config.refill_stream_max_len.clamp(100, 1_000_000)
    }

    pub fn has_driver_for_provider(&self, provider: &ProviderConfigYaml) -> bool {
        self.registry.resolve(provider).ok().flatten().is_some()
    }

    pub fn drivers(&self) -> Vec<CredentialPoolDriverView> {
        self.registry
            .drivers
            .values()
            .map(|driver| CredentialPoolDriverView {
                id: driver.id.clone(),
                mode: driver.mode().to_string(),
                provider_ids: driver.provider_ids.clone(),
            })
            .collect()
    }

    pub async fn provider_views(
        &self,
        providers: &[ProviderConfigYaml],
    ) -> Vec<ProviderCredentialPoolAutomationView> {
        let states = self.states.read().await;
        providers
            .iter()
            .map(|provider| {
                let resolved_driver = self.registry.resolve(provider);
                let driver = resolved_driver.as_ref().ok().and_then(|value| *value);
                let run_state = states.get(&provider.id);
                let resolution_error = resolved_driver.err();
                let default_state = if !self.config.enabled {
                    "disabled"
                } else if driver.is_none() {
                    "not_configured"
                } else {
                    "idle"
                };
                let default_message = resolution_error.or_else(|| {
                    if self.config.enabled && driver.is_none() {
                        Some("该渠道尚未配置自动补号驱动器".to_string())
                    } else {
                        None
                    }
                });
                ProviderCredentialPoolAutomationView {
                    provider_id: provider.id.clone(),
                    provider_label: provider
                        .label
                        .clone()
                        .unwrap_or_else(|| provider.id.clone()),
                    target_size: normalized_target_size(provider.pool_target_size),
                    credential_count: provider.credentials.len(),
                    active_credential_count: active_credential_count(provider),
                    auto_refill_enabled: provider.auto_refill_enabled,
                    auto_prune_enabled: provider.auto_prune_enabled,
                    permanent_delete_enabled: provider.credential_permanent_delete_enabled,
                    driver_id: driver
                        .map(|entry| entry.id.clone())
                        .or_else(|| provider.credential_automation_driver_id.clone()),
                    driver_mode: driver.map(|entry| entry.mode().to_string()),
                    driver_configured: driver.is_some(),
                    state: run_state
                        .map(|state| state.state.clone())
                        .unwrap_or_else(|| default_state.to_string()),
                    last_run_at: run_state.and_then(|state| state.last_run_at.clone()),
                    next_run_at: run_state.and_then(|state| state.next_run_at.clone()),
                    last_action: run_state.and_then(|state| state.last_action.clone()),
                    created_count: run_state.map(|state| state.created_count).unwrap_or(0),
                    pruned_count: run_state.map(|state| state.pruned_count).unwrap_or(0),
                    message: run_state
                        .and_then(|state| state.message.clone())
                        .or(default_message),
                    revision_id: run_state.and_then(|state| state.revision_id.clone()),
                }
            })
            .collect()
    }

    pub(super) fn lock_for_provider(&self, provider_id: &str) -> Arc<Mutex<()>> {
        self.provider_locks
            .entry(provider_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub(super) async fn set_state(&self, provider_id: &str, state: ProviderRunState) {
        self.states
            .write()
            .await
            .insert(provider_id.to_string(), state);
    }
}

pub(super) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

pub(super) fn next_run_at(interval: Duration) -> String {
    let seconds = i64::try_from(interval.as_secs()).unwrap_or(i64::MAX);
    OffsetDateTime::now_utc()
        .saturating_add(time::Duration::seconds(seconds))
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}
