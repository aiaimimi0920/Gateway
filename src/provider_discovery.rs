//! Explicit account onboarding/refresh only; inference never performs discovery I/O.
mod catalogue;
mod endpoint_candidates;
mod registry;
mod selection;
#[cfg(test)]
mod tests;
mod transport;
pub use registry::{ProbeAttempt, ProtocolProbe};
pub use selection::{select_protocol, select_protocol_with_override, ProtocolCapability};

use crate::routing::candidate::ProviderAccountPayload;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredProtocol {
    ChatCompletions,
    Responses,
    Messages,
    GeminiGenerateContent,
    GeminiInteractions,
    OllamaChat,
    OllamaGenerate,
    CohereChat,
    BedrockConverse,
    Completions,
    DashscopeText,
    DashscopeMultimodal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDiscovery {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<ProtocolProbe>,
    /// Empty only for documents written by the legacy single-protocol reader.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocols: Vec<ProtocolCapability>,
    pub source_url: String,
    pub api_base: String,
    pub protocol: DiscoveredProtocol,
    pub models: Vec<String>,
    /// Catalogue visibility is distinct from an actual generation check.
    pub verified_models: Vec<String>,
    pub checked_at: String,
    pub binding: String,
}

pub fn binding(url: &str, key: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{}\0{}", url.trim_end_matches('/'), key))
    )
}

impl DiscoveredProtocol {
    /// Discovery evidence must not enable an unimplemented runtime bridge.
    pub fn routing_ready(self) -> bool {
        matches!(
            self,
            Self::ChatCompletions
                | Self::Responses
                | Self::Messages
                | Self::DashscopeText
                | Self::DashscopeMultimodal
        )
    }
    pub fn family(self) -> &'static str {
        match self {
            DiscoveredProtocol::ChatCompletions => crate::protocol::registry::OPENAI_CHAT_FAMILY,
            DiscoveredProtocol::Responses => crate::protocol::registry::OPENAI_RESPONSES_FAMILY,
            DiscoveredProtocol::Messages => crate::protocol::registry::ANTHROPIC_MESSAGES_FAMILY,
            DiscoveredProtocol::GeminiGenerateContent => "gemini_generate_content",
            DiscoveredProtocol::GeminiInteractions => "gemini_interactions",
            DiscoveredProtocol::OllamaChat => "ollama_chat",
            DiscoveredProtocol::OllamaGenerate => "ollama_generate",
            DiscoveredProtocol::CohereChat => "cohere_chat",
            DiscoveredProtocol::BedrockConverse => "bedrock_converse",
            DiscoveredProtocol::Completions => "openai_legacy_completions",
            DiscoveredProtocol::DashscopeText => "dashscope_text",
            DiscoveredProtocol::DashscopeMultimodal => "dashscope_multimodal",
        }
    }
}

impl CredentialDiscovery {
    pub fn family(&self) -> &'static str {
        self.protocol.family()
    }

    pub fn apply(&self, payload: &mut ProviderAccountPayload) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.binding == binding(&payload.base_url, &payload.api_key),
            "Account address or key changed; refresh model and protocol discovery."
        );
        let source = transport::checked_url(&self.source_url)?;
        anyhow::ensure!(
            self.source_url.trim_end_matches('/') == payload.base_url.trim_end_matches('/'),
            "Discovery address no longer matches the account."
        );
        let api = transport::checked_url(&self.api_base)?;
        anyhow::ensure!(
            source.origin() == api.origin(),
            "Discovery cannot change credential origin."
        );
        anyhow::ensure!(
            registry::bases(&self.source_url, self.protocol)?.contains(&self.api_base),
            "Discovery API base is outside the configured address."
        );
        anyhow::ensure!(
            !self.models.is_empty() && self.models.len() <= 2048,
            "Discovery model catalogue is empty or too large."
        );
        anyhow::ensure!(
            self.models.iter().all(|m| !m.trim().is_empty()
                && m.len() <= 256
                && !m.chars().any(char::is_control))
                && !self.verified_models.is_empty()
                && self.verified_models.iter().all(|m| self.models.contains(m)),
            "Discovery requires valid model IDs and a verified catalogue model."
        );
        anyhow::ensure!(
            self.protocols.len() <= registry::ALL.len(),
            "Too many discovery protocols."
        );
        anyhow::ensure!(
            self.probes.len() <= registry::ALL.len(),
            "Too many probe reports."
        );
        if let Some(first) = self.protocols.first() {
            anyhow::ensure!(
                first.protocol == self.protocol
                    && first.api_base == self.api_base
                    && first.verified_models == self.verified_models,
                "Inconsistent legacy discovery projection."
            );
        }
        for (index, capability) in self.protocols.iter().enumerate() {
            anyhow::ensure!(
                !self.protocols[..index]
                    .iter()
                    .any(|c| c.protocol == capability.protocol)
                    && registry::bases(&self.source_url, capability.protocol)?
                        .contains(&capability.api_base)
                    && !capability.verified_models.is_empty()
                    && !capability
                        .verified_models
                        .iter()
                        .any(|m| capability.failed_models.contains(m))
                    && capability
                        .verified_models
                        .iter()
                        .chain(&capability.failed_models)
                        .all(|m| self.models.contains(m)),
                "Invalid protocol discovery evidence."
            );
        }
        payload.discovered_protocols = if self.protocols.is_empty() {
            vec![ProtocolCapability {
                protocol: self.protocol,
                api_base: self.api_base.clone(),
                verified_models: self.verified_models.clone(),
                failed_models: Vec::new(),
            }]
        } else {
            self.protocols.clone()
        };
        let selected = payload
            .discovered_protocols
            .iter()
            .find(|c| c.protocol.routing_ready())
            .unwrap_or(&payload.discovered_protocols[0])
            .clone();
        selected.apply(payload);
        Ok(())
    }
}

pub async fn discover(url: &str, key: &str) -> anyhow::Result<CredentialDiscovery> {
    anyhow::ensure!(
        !key.trim().is_empty() && key.len() <= 16_384 && !key.contains(['\r', '\n']),
        "A valid API key is required."
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(720),
        transport::discover(url, key),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Discovery timed out; previous settings were not changed."))?
}
