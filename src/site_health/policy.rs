//! Stable detection-plan identities and explicit local-group/remote-classification separation.
use super::{HealthMode, UnknownPolicy};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthAdapter {
    #[default]
    Auto,
    NewApi,
    Sub2api,
    OneApi,
    CompatibleGateway,
    // Legacy wire values stay pinned to one reader; never silently broaden secret destinations.
    NewApiPerfSummary,
    ModelStatusAll,
    ChannelHealth,
    ModelStatusEmbed,
    ModelHealthSlot,
    Sub2apiV2,
    OpenaiStatus,
    AnthropicStatus,
    DeepseekStatus,
    NvidiaStatus,
    GeminiStatus,
    OpenrouterStatus,
    CohereStatus,
    GroqStatus,
    PoeStatus,
    PerplexityStatus,
    LinkupStatus,
    ExaStatus,
    JinaStatus,
    TogetherStatus,
    MistralStatus,
    XaiStatus,
    TavilyStatus,
    YouStatus,
    AzureStatus,
    QwenStatus,
}
impl HealthAdapter {
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::NewApi => "new-api",
            Self::Sub2api => "sub2api",
            Self::OneApi => "one-api",
            Self::CompatibleGateway => "compatible-gateway",
            Self::NewApiPerfSummary => "new-api-perf-summary",
            Self::ModelStatusAll => "model-status-all",
            Self::ChannelHealth => "channel-health",
            Self::ModelStatusEmbed => "model-status-embed",
            Self::ModelHealthSlot => "model-health-slot",
            Self::Sub2apiV2 => "sub2api-v2",
            Self::OpenaiStatus => "openai-status",
            Self::AnthropicStatus => "anthropic-status",
            Self::DeepseekStatus => "deepseek-status",
            Self::NvidiaStatus => "nvidia-status",
            Self::GeminiStatus => "gemini-status",
            Self::OpenrouterStatus => "openrouter-status",
            Self::CohereStatus => "cohere-status",
            Self::GroqStatus => "groq-status",
            Self::PoeStatus => "poe-status",
            Self::PerplexityStatus => "perplexity-status",
            Self::LinkupStatus => "linkup-status",
            Self::ExaStatus => "exa-status",
            Self::JinaStatus => "jina-status",
            Self::TogetherStatus => "together-status",
            Self::MistralStatus => "mistral-status",
            Self::XaiStatus => "xai-status",
            Self::TavilyStatus => "tavily-status",
            Self::YouStatus => "you-status",
            Self::AzureStatus => "azure-status",
            Self::QwenStatus => "qwen-status",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitoringAuthKind {
    #[default]
    None,
    Bearer,
    ApiKey,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonitoringAuth {
    pub kind: MonitoringAuthKind,
    pub token: Option<String>,
    pub header: Option<String>,
}
impl std::fmt::Debug for MonitoringAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MonitoringAuth")
            .field("kind", &self.kind)
            .field("token", &"<redacted>")
            .finish()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProbePolicy {
    /// Preserve absence for immutable revision digests; absent still means enabled.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_enabled"
    )]
    pub enabled: Option<bool>,
    pub mode: HealthMode,
    pub site_url: Option<String>,
    pub refresh_interval_secs: u64,
    pub max_age_secs: u64,
    pub minimum_success_rate: f64,
    pub unknown_policy: UnknownPolicy,
    pub monitor_groups: Vec<String>,
    pub model_map: HashMap<String, String>,
    pub allow_private_network: bool,
    pub adapter: HealthAdapter,
    pub auth: MonitoringAuth,
}
impl Default for ProbePolicy {
    fn default() -> Self {
        Self {
            enabled: None,
            mode: HealthMode::Advisory,
            site_url: None,
            refresh_interval_secs: 300,
            max_age_secs: 900,
            minimum_success_rate: 95.0,
            unknown_policy: UnknownPolicy::Allow,
            monitor_groups: Vec::new(),
            model_map: HashMap::new(),
            allow_private_network: false,
            adapter: HealthAdapter::Auto,
            auth: MonitoringAuth::default(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthProfile {
    pub id: String,
    pub name: String,
    /// Local credential-group IDs, never remote monitor group names. Empty means all groups.
    #[serde(default)]
    pub account_group_ids: Vec<String>,
    pub policy: ProbePolicy,
}
// A present activation field must remain a boolean, not null or a string.
fn deserialize_enabled<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}

impl ProbePolicy {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn validate(&self, base: &str) -> anyhow::Result<()> {
        if let Some(url) = self.adapter.official_url() {
            anyhow::ensure!(self.site_url.as_deref().is_none_or(|v| v == url)
                && self.auth.kind == MonitoringAuthKind::None && self.auth.token.is_none()
                && !self.allow_private_network && self.monitor_groups.is_empty()
                && self.model_map.is_empty(),
                "Official health adapters require their fixed public URL, no credentials or custom mappings");
        }
        super::transport::checked_url(
            self.adapter
                .official_url()
                .or(self.site_url.as_deref())
                .unwrap_or(base),
        )?;
        anyhow::ensure!(
            (30..=86400).contains(&self.refresh_interval_secs)
                && (30..=604800).contains(&self.max_age_secs)
                && self.minimum_success_rate.is_finite()
                && (0.0..=100.0).contains(&self.minimum_success_rate)
                && self.monitor_groups.len() <= 16
                && self.model_map.len() <= 4096
                && self.monitor_groups.iter().all(|s| !s.trim().is_empty()
                    && s.len() <= 128
                    && !s.chars().any(char::is_control))
                && self.model_map.iter().all(|(a, b)| !a.is_empty()
                    && !b.is_empty()
                    && a.len() <= 256
                    && b.len() <= 256),
            "Invalid site health limits, threshold or model/group mapping"
        );
        anyhow::ensure!(
            self.auth
                .token
                .as_ref()
                .is_none_or(|s| s.len() <= 8192 && !s.chars().any(char::is_control)),
            "Invalid monitoring credential"
        );
        anyhow::ensure!(
            self.auth.kind != MonitoringAuthKind::ApiKey
                || self.auth.header.as_deref().is_some_and(|h| matches!(
                    h.to_ascii_lowercase().as_str(),
                    "x-api-key" | "api-key"
                )),
            "Unsupported monitoring API key header"
        );
        // Automatic public discovery never broadcasts credentials to every candidate adapter.
        anyhow::ensure!(
            self.adapter != HealthAdapter::Auto || self.auth.kind == MonitoringAuthKind::None,
            "Authenticated monitoring requires an explicitly selected adapter"
        );
        anyhow::ensure!(
            !matches!(
                self.adapter,
                HealthAdapter::Sub2api | HealthAdapter::Sub2apiV2
            ) || self.monitor_groups.iter().all(|g| g
                .parse::<i64>()
                .is_ok_and(|id| id > 0 && id.to_string() == *g)),
            "sub2api monitor groups must be positive numeric group IDs"
        );
        Ok(())
    }
}
pub(super) fn validate_profiles(profiles: &[HealthProfile], base: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        profiles.len() <= 16,
        "At most 16 site health plans per provider"
    );
    let mut ids = HashSet::new();
    let mut assigned = HashSet::new();
    for p in profiles {
        anyhow::ensure!(
            !p.id.is_empty()
                && p.id.len() <= 64
                && p.id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
                && ids.insert(&p.id),
            "Invalid or duplicate site health plan ID"
        );
        anyhow::ensure!(
            !p.name.trim().is_empty() && p.name.len() <= 128 && p.account_group_ids.len() <= 64,
            "Invalid site health plan name or groups"
        );
        anyhow::ensure!(
            p.account_group_ids
                .iter()
                .all(|id| !id.is_empty() && id.len() <= 256 && assigned.insert(id)),
            "A local group may belong to only one detection plan"
        );
        anyhow::ensure!(
            !p.account_group_ids.is_empty() || profiles.len() == 1,
            "An all-groups plan cannot overlap another plan"
        );
        p.policy.validate(base)?;
    }
    Ok(())
}
