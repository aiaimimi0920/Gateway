//! A probe may update only the exact identity that normal route candidates read.
use super::*;
use crate::{db::RecordCredentialModelSuccessInput, provider_discovery::CredentialDiscovery};

pub(super) fn effective_protocol_profile(
    provider: &CompiledProvider,
    discovery: Option<&CredentialDiscovery>,
) -> String {
    discovery
        .map(|discovery| match discovery.protocol {
            crate::provider_discovery::DiscoveredProtocol::Messages => "anthropic_messages".into(),
            _ => "openai_compatible_generic".into(),
        })
        .unwrap_or_else(|| provider.protocol_profile.clone())
}

impl RouteConfigSnapshot {
    pub(crate) fn model_probe_identity(
        &self,
        target: &CredentialProbeTarget,
        model: &str,
    ) -> Option<RecordCredentialModelSuccessInput> {
        let provider = self
            .compiled
            .providers
            .iter()
            .find(|provider| provider.id == target.provider_id)?;
        let (payload, discovery) = if provider.credential_pool.is_empty() {
            if target.credential_id != provider_default_account_id(&provider.id) {
                return None;
            }
            (provider.payload.clone(), None)
        } else {
            let credential = provider
                .credential_pool
                .iter()
                .find(|credential| credential.id.trim() == target.credential_id)?;
            if !credential.enabled {
                return None;
            }
            (
                apply_token_override(credential),
                credential.discovery.as_ref(),
            )
        };
        // OAuth refresh can change the runtime payload without changing the route revision.
        // Structural comparison avoids serializing secrets into a public digest or log.
        if !target.enabled
            || serde_json::to_value(&payload).ok()? != serde_json::to_value(&target.payload).ok()?
        {
            return None;
        }
        Some(RecordCredentialModelSuccessInput {
            provider_account_id: provider.id.clone(),
            provider_credential_id: None, // Console targets come from route config, not DB credentials.
            provider_credential_ref: payload.credential_id.clone(),
            protocol_profile: Some(effective_protocol_profile(provider, discovery)),
            model: model.into(),
        })
    }
}
