//! Three independent selections. Legacy documents omit this block entirely.
mod preparation;
#[cfg(test)]
mod tests;

use super::request_profile::RequestClientProfile;
pub use preparation::{prepare, PreparedCallFlow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ProviderCallFlow {
    #[serde(default)]
    pub call_adapter: CallAdapter,
    #[serde(default)]
    pub fingerprint_adapter: FingerprintAdapter,
    #[serde(default)]
    pub special_adapter: SpecialAdapter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallAdapter {
    #[default]
    Auto,
    Openai,
    Anthropic,
    Gemini,
    Cohere,
}

impl CallAdapter {
    pub fn adapter(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Openai => Some("openai_compatible"),
            Self::Anthropic => Some("anthropic_compatible"),
            Self::Gemini => Some("gemini_api_compatible"),
            Self::Cohere => Some("cohere_compatible"),
        }
    }

    /// Discovery keeps its verified/negative evidence; a fixed format only narrows it.
    pub fn allows(self, protocol: crate::provider_discovery::DiscoveredProtocol) -> bool {
        use crate::provider_discovery::DiscoveredProtocol as P;
        match self {
            Self::Auto => true,
            Self::Openai => matches!(protocol, P::ChatCompletions | P::Responses),
            Self::Anthropic => protocol == P::Messages,
            // Discovery-only Gemini/Cohere capabilities are not made routing-ready here.
            Self::Gemini | Self::Cohere => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintAdapter {
    #[default]
    Auto,
    CodexCli,
    ClaudeCli,
    Workbuddy,
    Cpa,
    CpaClaudeCode,
    Session,
}

impl FingerprintAdapter {
    pub fn profile(self) -> Option<RequestClientProfile> {
        Some(match self {
            Self::Auto => RequestClientProfile::Auto,
            Self::CodexCli => RequestClientProfile::CodexCli,
            Self::ClaudeCli => RequestClientProfile::ClaudeCli,
            Self::Workbuddy => RequestClientProfile::Workbuddy,
            Self::Cpa => RequestClientProfile::Cpa,
            Self::CpaClaudeCode => RequestClientProfile::CpaClaudeCode,
            Self::Session => return None,
        })
    }
}

impl From<RequestClientProfile> for FingerprintAdapter {
    fn from(value: RequestClientProfile) -> Self {
        match value {
            RequestClientProfile::Auto => Self::Auto,
            RequestClientProfile::CodexCli => Self::CodexCli,
            RequestClientProfile::ClaudeCli => Self::ClaudeCli,
            RequestClientProfile::Workbuddy => Self::Workbuddy,
            RequestClientProfile::Cpa => Self::Cpa,
            RequestClientProfile::CpaClaudeCode => Self::CpaClaudeCode,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialAdapter {
    /// Retain the preset's existing execution owner, including non-text products.
    #[default]
    Auto,
    None,
    Basispoints,
    GeminiWeb,
    GeminiCanvas,
    ChatgptWeb,
    AistudioWeb,
}

impl SpecialAdapter {
    pub fn adapter(self) -> Option<&'static str> {
        match self {
            Self::Auto | Self::None => None,
            Self::Basispoints => Some(crate::protocol::chatgpt::basispoints::ADAPTER),
            Self::GeminiWeb => Some("gemini_web_compatible"),
            Self::GeminiCanvas => Some("gemini_canvas_web_reverse_compatible"),
            Self::ChatgptWeb => Some("chatgpt_web_reverse_compatible"),
            Self::AistudioWeb => Some("aistudio_web_reverse_compatible"),
        }
    }
}
