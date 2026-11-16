//! Product recognition for ordinary Qwen material, not API-Key validity or quota inference.
use crate::{
    protocol::qwen::workspace_api::AUTH_MODE,
    provider_discovery::model_catalog::qwen_endpoint::QwenEndpoint,
    routing::config::{ProviderConfigYaml, ProviderCredentialYaml},
};

pub(super) fn owns(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    // Retain the original preset identity even when later overrides change the effective endpoint.
    provider.preset.as_deref() == Some("qwen-official-api")
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(AUTH_MODE)
        || QwenEndpoint::parse(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        .is_some()
}
