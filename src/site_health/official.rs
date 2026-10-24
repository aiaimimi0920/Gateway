//! Official observations retain categorical status; never fabricate success percentages.
mod aliyun;
mod azure;
#[cfg(test)]
mod catalog_tests;
mod gemini;
mod parsing;
#[cfg(test)]
mod remaining_tests;
mod statuspage;
#[cfg(test)]
mod tests;
mod together;
mod xai;
use super::{HealthAdapter, MonitoringAuth, ProbePolicy, ProbeResult, SiteSnapshot, UnknownPolicy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub name: String,
    pub status: String,
    /// Only explicit model identities may participate in model admission.
    pub model: Option<String>,
}

impl HealthAdapter {
    pub fn official_url(self) -> Option<&'static str> {
        Some(match self {
            Self::OpenaiStatus => "https://status.openai.com/",
            Self::AnthropicStatus => "https://status.claude.com/",
            Self::DeepseekStatus => "https://status.deepseek.com/",
            Self::NvidiaStatus => "https://status.ngc.nvidia.com/",
            Self::GeminiStatus => "https://aistudio.google.com/status",
            Self::OpenrouterStatus => "https://status.openrouter.ai/",
            Self::CohereStatus => "https://status.cohere.com/",
            Self::GroqStatus => "https://groqstatus.com/",
            Self::PoeStatus => "https://status.poe.com/",
            Self::PerplexityStatus => "https://status.perplexity.com/",
            Self::LinkupStatus => "https://status.linkup.so/",
            Self::ExaStatus => "https://status.exa.ai/",
            Self::JinaStatus => "https://status.jina.ai/",
            Self::TogetherStatus => "https://status.together.ai/",
            Self::MistralStatus => "https://status.mistral.ai/",
            Self::XaiStatus => "https://status.x.ai/",
            Self::TavilyStatus => "https://status.tavily.com/",
            Self::YouStatus => "https://status.you.com/",
            Self::AzureStatus => "https://azure.status.microsoft/en-us/status",
            Self::QwenStatus => "https://status.aliyun.com/",
            _ => return None,
        })
    }
}

pub(super) fn allows(
    config: &ProbePolicy,
    snapshot: Option<&SiteSnapshot>,
    binding: &str,
    model: &str,
    now: u64,
) -> bool {
    let observations = snapshot
        .filter(|s| {
            s.binding == binding && now >= s.checked_at && now - s.checked_at < config.max_age_secs
        })
        .into_iter()
        .flat_map(|s| &s.probes)
        .filter(|p| p.template == config.adapter.id() && p.status == "supported")
        .flat_map(|p| &p.observations)
        .filter(|o| {
            o.model.as_deref() == Some(model)
                || (config.adapter == HealthAdapter::OpenaiStatus && o.name == "Codex API")
        });
    let mut found = false;
    let mut unknown = false;
    for observation in observations {
        found = true;
        match observation.status.as_str() {
            "operational" => (),
            "degraded_performance" | "partial_outage" | "major_outage" | "under_maintenance" => {
                return false
            }
            _ => unknown = true,
        }
    }
    if !found || unknown {
        config.unknown_policy == UnknownPolicy::Allow
    } else {
        true
    }
}

pub(super) async fn discover(
    id: &str,
    base: &str,
    config: &ProbePolicy,
) -> anyhow::Result<SiteSnapshot> {
    let origin = super::transport::checked_url(config.adapter.official_url().unwrap())?;
    let client = if config.adapter == HealthAdapter::AzureStatus {
        super::transport::regional_client(&origin).await?
    } else {
        super::transport::client(&origin, false).await?
    };
    let path = match config.adapter {
        HealthAdapter::DeepseekStatus => "api/status-page/6410630422455/summary/active",
        HealthAdapter::OpenrouterStatus => "config.json",
        HealthAdapter::JinaStatus => "api/v2/summary.json",
        HealthAdapter::LinkupStatus => "api/status/summary.json",
        HealthAdapter::ExaStatus => "v3/components.json",
        HealthAdapter::TogetherStatus => "index.json",
        HealthAdapter::XaiStatus => "api/status",
        HealthAdapter::TavilyStatus | HealthAdapter::YouStatus => "api/v2/summary.json",
        HealthAdapter::AzureStatus => "en-us/status",
        HealthAdapter::QwenStatus => "api/status/listEventInProgress",
        _ => "api/v2/components.json",
    };
    let endpoint = origin.join(path)?.to_string();
    let mut probe = ProbeResult {
        template: config.adapter.id().into(),
        endpoint: endpoint.clone(),
        status: "invalid_response".into(),
        http_status: None,
        model_count: 0,
        models: Default::default(),
        coverage: None,
        observations: Vec::new(),
    };
    let response = if config.adapter == HealthAdapter::GeminiStatus {
        probe.endpoint = "https://aistudio.google.com/status".into();
        gemini::fetch(&client)
            .await
            .map(|body| parsing::parse(config.adapter, &body))
    } else if config.adapter == HealthAdapter::AzureStatus {
        azure::fetch(&client, &endpoint).await
    } else {
        super::transport::json(&client, &endpoint, None, &MonitoringAuth::default())
            .await
            .map(|body| parsing::parse(config.adapter, &body))
    };
    match response {
        Ok(observations) => {
            probe.http_status = Some(200);
            if let Some(observations) = observations {
                probe.status = if observations.is_empty() {
                    "no_samples"
                } else {
                    "supported"
                }
                .into();
                probe.model_count = observations.iter().filter(|o| o.model.is_some()).count();
                probe.observations = observations;
            }
        }
        Err((status, code)) => {
            probe.status = status;
            probe.http_status = code;
        }
    }
    Ok(SiteSnapshot {
        provider_id: id.into(),
        profile_id: None,
        checked_at: super::now(),
        binding: super::configuration::binding(config, base, None),
        selected_template: (probe.status == "supported").then(|| config.adapter.id().into()),
        models: Default::default(),
        probes: vec![probe],
    })
}
