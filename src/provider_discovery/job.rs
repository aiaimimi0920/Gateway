//! Persisted, non-secret discovery intent and stale-result fences.
use super::CredentialDiscovery;
use crate::routing::config::{ModelRoute, ProviderConfigYaml, RouteConfigYaml};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryJob {
    pub id: String,
    pub status: DiscoveryJobStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryJobStatus {
    Pending,
    Complete,
    Unconfirmed,
}

pub(crate) fn supported(provider: &ProviderConfigYaml) -> bool {
    provider.preset.is_none()
        && matches!(
            provider.adapter.as_deref(),
            Some(
                "openai_compatible"
                    | "anthropic_compatible"
                    | "dashscope_compatible"
                    | "dashscope_multimodal_compatible"
            )
        )
}

pub(crate) fn requested(document: &RouteConfigYaml) -> bool {
    document.providers.iter().any(|p| {
        p.credentials.iter().any(|c| {
            c.discovery_job
                .as_ref()
                .is_some_and(|j| j.status == DiscoveryJobStatus::Pending)
        })
    })
}

pub(crate) fn new_request(previous: &RouteConfigYaml, document: &RouteConfigYaml) -> bool {
    document.providers.iter().any(|p| {
        p.credentials.iter().any(|c| {
            let Some(job) = c
                .discovery_job
                .as_ref()
                .filter(|j| j.status == DiscoveryJobStatus::Pending)
            else {
                return false;
            };
            let old_provider = previous.providers.iter().find(|old| old.id == p.id);
            let Some(old_provider) = old_provider else {
                return true;
            };
            let Some(old) = old_provider.credentials.iter().find(|old| old.id == c.id) else {
                return true;
            };
            old.discovery_job.as_ref() != Some(job)
                || old.base_url.as_ref().unwrap_or(&old_provider.base_url)
                    != c.base_url.as_ref().unwrap_or(&p.base_url)
                || old.api_key.as_ref().unwrap_or(&old_provider.api_key)
                    != c.api_key.as_ref().unwrap_or(&p.api_key)
        })
    })
}

pub(crate) fn validate(document: &RouteConfigYaml) -> anyhow::Result<()> {
    for provider in &document.providers {
        for credential in &provider.credentials {
            if let Some(job) = &credential.discovery_job {
                anyhow::ensure!(
                    supported(provider),
                    "Discovery requires a generic API provider."
                );
                anyhow::ensure!(
                    uuid::Uuid::parse_str(&job.id).is_ok(),
                    "Invalid discovery job ID."
                );
                anyhow::ensure!(
                    credential.id.as_ref().is_some_and(|id| !id.is_empty()),
                    "Discovery requires an explicit account ID."
                );
            }
        }
    }
    Ok(())
}

/// The job token fences replacement/recreation; the binding fences address/key edits.
/// Only merge into the latest document, never into the snapshot used for network I/O.
pub(crate) fn merge(
    document: &mut RouteConfigYaml,
    provider_id: &str,
    credential_id: &str,
    job: &DiscoveryJob,
    discovery: Option<&CredentialDiscovery>,
) -> bool {
    let Some(provider) = document.providers.iter_mut().find(|p| p.id == provider_id) else {
        return false;
    };
    if !supported(provider) {
        return false;
    }
    let Some(credential) = provider
        .credentials
        .iter_mut()
        .find(|c| c.id.as_deref() == Some(credential_id))
    else {
        return false;
    };
    if credential.discovery_job.as_ref() != Some(job) {
        return false;
    }
    credential.discovery_job.as_mut().unwrap().status = if discovery.is_some() {
        DiscoveryJobStatus::Complete
    } else {
        DiscoveryJobStatus::Unconfirmed
    };
    let Some(discovery) = discovery else {
        return true;
    };
    credential.discovery = Some(discovery.clone());
    credential.supported_models = discovery.models.clone();
    // Preserve inherited/manual models for the other accounts and route policy fields.
    for model in &discovery.models {
        if !provider.supported_models.contains(model) {
            provider.supported_models.push(model.clone());
        }
        if let Some(route) = document
            .model_routes
            .iter_mut()
            .find(|r| r.pattern == *model)
        {
            if !route.provider_ids.contains(&provider.id) {
                route.provider_ids.push(provider.id.clone());
            }
        } else {
            document.model_routes.push(ModelRoute {
                pattern: model.clone(),
                provider_ids: vec![provider.id.clone()],
                priority: 10,
                enabled: true,
            });
        }
    }
    true
}
