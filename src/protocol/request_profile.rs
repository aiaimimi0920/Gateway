//! Operator-owned wire identity; independent of endpoint, authentication and model routing.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestClientProfile {
    #[default]
    Auto,
    CodexCli,
    ClaudeCli,
    Workbuddy,
    Cpa,
    CpaClaudeCode,
}

impl RequestClientProfile {
    pub(crate) fn is_cpa(self) -> bool {
        matches!(self, Self::Cpa | Self::CpaClaudeCode)
    }
    pub(crate) fn uses_claude(self, model: &str, requested_model: Option<&str>) -> bool {
        match self {
            Self::ClaudeCli | Self::CpaClaudeCode => true,
            Self::CodexCli | Self::Workbuddy => false,
            Self::Auto | Self::Cpa => is_claude(model) || requested_model.is_some_and(is_claude),
        }
    }
}

fn is_claude(model: &str) -> bool {
    model.split(['/', ':', '.']).any(|part| {
        part.eq_ignore_ascii_case("claude")
            || part
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("claude-"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_client_profiles_auto_and_explicit_selection() {
        for model in [
            "claude-sonnet-4-6",
            "anthropic/Claude-Opus",
            "anthropic.claude-sonnet:0",
        ] {
            assert!(RequestClientProfile::Auto.uses_claude(model, None));
        }
        for model in ["gpt-fixture", "o3", "gemini-fixture", "not-claude-proxy"] {
            assert!(!RequestClientProfile::Auto.uses_claude(model, None));
        }
        assert!(RequestClientProfile::Auto.uses_claude("deployment-1", Some("claude-sonnet")));
        assert!(!RequestClientProfile::CodexCli.uses_claude("claude-sonnet", None));
        assert!(RequestClientProfile::ClaudeCli.uses_claude("gpt-fixture", None));
        assert!(serde_json::from_str::<RequestClientProfile>("\"unknown\"").is_err());
    }
}
