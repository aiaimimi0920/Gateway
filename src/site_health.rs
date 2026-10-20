//! Independent site observations. Never writes credential tests, breakers, quota or tariffs.
pub mod discovery;
mod parsing;
pub(crate) mod runtime;
mod transport;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthMode {
    #[default]
    Advisory,
    Restrict,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownPolicy {
    #[default]
    Allow,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SiteHealthConfig {
    pub mode: HealthMode,
    pub site_url: Option<String>,
    pub refresh_interval_secs: u64,
    pub max_age_secs: u64,
    pub minimum_success_rate: f64,
    pub unknown_policy: UnknownPolicy,
    pub monitor_groups: Vec<String>,
    pub model_map: HashMap<String, String>,
    pub allow_private_network: bool,
}

impl Default for SiteHealthConfig {
    fn default() -> Self {
        Self {
            mode: HealthMode::Advisory,
            site_url: None,
            refresh_interval_secs: 300,
            max_age_secs: 900,
            minimum_success_rate: 95.0,
            unknown_policy: UnknownPolicy::Allow,
            monitor_groups: Vec::new(),
            model_map: HashMap::new(),
            allow_private_network: false,
        }
    }
}

impl SiteHealthConfig {
    pub fn validate(&self, base: &str) -> anyhow::Result<()> {
        transport::checked_url(self.site_url.as_deref().unwrap_or(base))?;
        anyhow::ensure!(
            (30..=86400).contains(&self.refresh_interval_secs)
                && (30..=604800).contains(&self.max_age_secs)
                && self.minimum_success_rate.is_finite()
                && (0.0..=100.0).contains(&self.minimum_success_rate)
                && self.monitor_groups.len() <= 16
                && self.model_map.len() <= 4096
                && self
                    .monitor_groups
                    .iter()
                    .all(|g| !g.trim().is_empty() && g.len() <= 128)
                && self.model_map.iter().all(|(a, b)| !a.is_empty()
                    && !b.is_empty()
                    && a.len() <= 256
                    && b.len() <= 256),
            "Invalid site health limits, threshold or model/group mapping."
        );
        Ok(())
    }
    pub(crate) fn binding(&self, base: &str) -> String {
        use sha2::{Digest, Sha256};
        // Sorted maps make bindings stable across deserialization and config snapshots.
        let mut value = serde_json::to_value(self).expect("finite validated health config");
        value["provider_base"] = serde_json::json!(base);
        hex::encode(Sha256::digest(value.to_string().as_bytes()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub template: String,
    pub endpoint: String,
    pub status: String,
    pub http_status: Option<u16>,
    pub model_count: usize,
    pub models: BTreeMap<String, Option<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSnapshot {
    pub provider_id: String,
    pub checked_at: u64,
    pub binding: String,
    pub probes: Vec<ProbeResult>,
    pub selected_template: Option<String>,
    pub models: BTreeMap<String, Option<f64>>,
}

pub fn allows(
    config: &SiteHealthConfig,
    snapshot: Option<&SiteSnapshot>,
    binding: &str,
    model: &str,
    now: u64,
) -> bool {
    if config.mode == HealthMode::Advisory {
        return true;
    }
    let rate = snapshot
        .filter(|s| {
            s.binding == binding && now >= s.checked_at && now - s.checked_at < config.max_age_secs
        })
        .and_then(|s| {
            s.models.get(
                config
                    .model_map
                    .get(model)
                    .map(String::as_str)
                    .unwrap_or(model),
            )
        })
        .copied()
        .flatten();
    match rate {
        Some(rate) if rate.is_finite() && (0.0..=100.0).contains(&rate) => {
            rate >= config.minimum_success_rate
        }
        _ => config.unknown_policy == UnknownPolicy::Allow,
    }
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
