//! Independent site observations. Never writes credential tests, breakers, quota or tariffs.
mod configuration;
pub mod discovery;
pub mod official;
mod parsing;
mod policy;
pub(crate) mod runtime;
mod transport;
pub use configuration::SiteHealthConfig;
pub(crate) use configuration::{CompiledPlan, CompiledSiteHealth};
pub use policy::{HealthAdapter, HealthProfile, MonitoringAuth, MonitoringAuthKind, ProbePolicy};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<official::Observation>,
    pub template: String,
    pub endpoint: String,
    pub status: String,
    pub http_status: Option<u16>,
    pub model_count: usize,
    pub models: BTreeMap<String, Option<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<ProbeCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeCoverage {
    pub complete: Option<bool>,
    pub aggregation_lag_secs: Option<u64>,
    pub computed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSnapshot {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    pub checked_at: u64,
    pub binding: String,
    pub probes: Vec<ProbeResult>,
    pub selected_template: Option<String>,
    pub models: BTreeMap<String, Option<f64>>,
}

pub fn allows(
    config: &ProbePolicy,
    snapshot: Option<&SiteSnapshot>,
    binding: &str,
    model: &str,
    now: u64,
) -> bool {
    if !config.is_enabled() || config.mode == HealthMode::Advisory {
        return true;
    }
    if config.adapter.official_url().is_some() {
        return official::allows(config, snapshot, binding, model, now);
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
