//! Original declaration anchors normalized Web material even if later merges change its audience.
use super::{ProviderConfigYaml, ProviderCredentialYaml};
use crate::{
    protocol::perplexity_web::material as product, routing::candidate::ProviderAccountPayload,
};

pub(crate) fn owns(
    provider: &ProviderConfigYaml,
    credential: Option<&ProviderCredentialYaml>,
) -> bool {
    provider.preset.as_deref() == Some(product::PRESET_ID)
        || provider.adapter.as_deref() == Some(product::ADAPTER)
        || credential
            .and_then(|c| c.auth_mode.as_deref())
            .or(provider.auth_mode.as_deref())
            == Some(product::AUTH_MODE)
        || product::owns_base(
            credential
                .and_then(|c| c.base_url.as_deref())
                .unwrap_or(&provider.base_url),
        )
        || credential
            .and_then(|c| c.session_auth.as_ref())
            .or(provider.session_auth.as_ref())
            .is_some_and(|s| s.primary_cookie_name.as_deref() == Some(product::SESSION_COOKIE))
}
fn claimed(provider: &ProviderConfigYaml) -> bool {
    owns(provider, None) || provider.credentials.iter().any(|c| owns(provider, Some(c)))
}
pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !claimed(cfg) {
        return Ok(());
    }
    // Credentials, UA and absolute expiry belong to one account; no shared inherited identity.
    if !cfg.explicit_credentials_only
        || !cfg.api_key.is_empty()
        || cfg.auth_token.is_some()
        || !cfg.headers.is_empty()
        || !cfg.extra_body.is_empty()
        || cfg.expires_at.is_some()
        || cfg.session_auth.is_some()
        || cfg.keepalive.is_some()
        || cfg.codex_basispoints_enabled
        || cfg
            .protocol_profile
            .as_deref()
            .is_some_and(|p| p != "perplexity_web")
        || cfg
            .protocol_family
            .as_deref()
            .is_some_and(|f| f != crate::protocol::registry::OPENAI_CHAT_FAMILY)
        || cfg.credentials.iter().any(|c| {
            c.grok_oauth.is_some()
                || c.refresh_token.is_some()
                || c.refresh_endpoint.is_some()
                || c.refresh_client_id.is_some()
                || c.token_expires_in_secs.is_some()
                || c.discovery.is_some()
                || c.discovery_job.is_some()
                || c.codex_basispoints_enabled == Some(true)
        })
    {
        return Err(product::invalid().into());
    }
    Ok(())
}
pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if claimed(cfg) {
        if !product::owns_payload(payload) {
            return Err(product::invalid().into());
        }
        product::validate(payload, payload.canonical_adapter())?;
    }
    Ok(())
}
