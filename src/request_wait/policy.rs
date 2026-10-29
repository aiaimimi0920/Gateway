//! Trusted provider settings; never infer long waiting from model names, URLs or OpenAI adapters.
use crate::{error::GatewayError, routing::config::RouteConfigSnapshot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestWaitMode {
    Disabled,
    #[default]
    Bounded,
    ClientConnected,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestWaitPolicy {
    #[serde(default)]
    pub mode: RequestWaitMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_timeout_seconds: Option<u32>,
}

impl RequestWaitPolicy {
    pub(crate) fn validate(&self) -> Result<(), GatewayError> {
        if self
            .total_timeout_seconds
            .is_some_and(|v| !(1..=900).contains(&v))
        {
            return Err(GatewayError::bad_request(
                "request_wait.total_timeout_seconds must be between 1 and 900",
            )
            .with_code("invalid_request_wait_policy"));
        }
        Ok(())
    }
    pub(crate) fn for_provider(snapshot: &RouteConfigSnapshot, id: &str) -> Self {
        let Some(provider) = snapshot.document().providers.iter().find(|p| p.id == id) else {
            return Self {
                mode: RequestWaitMode::Disabled,
                total_timeout_seconds: None,
            };
        };
        provider.request_wait.clone().unwrap_or_else(|| Self {
            mode: if provider.preset.as_deref().is_some_and(|preset| {
                matches!(
                    crate::protocol::registry::default_protocol_profile_for_preset(preset),
                    "chatgpt_official_api" | "chatgpt_codex_oauth_official_api"
                )
            }) {
                RequestWaitMode::ClientConnected
            } else {
                RequestWaitMode::Bounded
            },
            total_timeout_seconds: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::config::RouteConfigStore;
    #[test]
    fn only_declared_official_templates_default_to_long_wait() {
        for (preset, expected) in [
            ("openai", RequestWaitMode::ClientConnected),
            (
                "chatgpt-codex-oauth-official-api",
                RequestWaitMode::ClientConnected,
            ),
            ("nvidia-openai", RequestWaitMode::Bounded),
            ("groq-openai", RequestWaitMode::Bounded),
        ] {
            let document = format!("providers:\n  - id: fixture\n    preset: {preset}\n    base_url: https://example.invalid\n    api_key: fixture\n");
            let store =
                RouteConfigStore::from_document(serde_yaml::from_str(&document).unwrap()).unwrap();
            assert_eq!(
                RequestWaitPolicy::for_provider(&store.snapshot(), "fixture").mode,
                expected
            );
        }
    }
}
