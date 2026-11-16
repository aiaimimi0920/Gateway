//! Explicit AWS material participates in the shared keep-audience protection.
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};

pub(super) fn owns(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    crate::protocol::bedrock_converse::explicit_auth_mode(
        credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref()),
    )
}
