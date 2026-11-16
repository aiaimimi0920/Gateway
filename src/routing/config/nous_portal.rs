//! Nous authority is credential-owned; expired access may compile for refresh, never receive a new TTL.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::nous_portal::{business, InferenceTarget, OAuthClient, TOKEN_ENDPOINT};
pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    let typed = cfg.auth_mode.as_deref() == Some(business::AUTH_MODE)
        || cfg.credentials.iter().any(|c| {
            c.nous_portal.is_some() || c.auth_mode.as_deref() == Some(business::AUTH_MODE)
        });
    if !typed {
        return Ok(());
    }
    if cfg.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || !cfg.explicit_credentials_only
        || InferenceTarget::from_base_url(&cfg.base_url).is_err()
        || cfg
            .preset
            .as_deref()
            .is_some_and(|p| p != "nous-portal-oauth")
        || cfg
            .adapter
            .as_deref()
            .is_some_and(|a| a != "openai_compatible")
        || cfg.codex_basispoints_enabled
    {
        return Err(business::invalid().into());
    }
    for c in &cfg.credentials {
        let context = c.nous_portal.as_ref().ok_or_else(business::invalid)?;
        let client = OAuthClient::new(context.client_id())?;
        context.validate_binding(c.api_key.as_deref().ok_or_else(business::invalid)?, &client)?;
        if c.grok_oauth.is_some()
            || c.kiro_oauth.is_some()
            || c.vertex_oauth.is_some()
            || c.dim_oauth.is_some()
            || c.accio_sgcookie.is_some()
            || c.freebuff_session.is_some()
            || c.refresh_endpoint
                .as_deref()
                .is_some_and(|s| s != TOKEN_ENDPOINT)
            || c.refresh_client_id
                .as_deref()
                .is_some_and(|s| s != context.client_id())
            || c.refresh_token
                .as_deref()
                .is_none_or(|s| !crate::protocol::nous_portal::atom(s, 32768))
            || !c
                .expires_at
                .as_deref()
                .is_some_and(|s| context.matches_expiry(s))
            || c.discovery.is_some()
            || c.discovery_job.is_some()
            || c.codex_basispoints_enabled == Some(true)
        {
            return Err(business::invalid().into());
        }
    }
    Ok(())
}
pub(super) fn configure(
    c: &ProviderCredentialYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if let Some(context) = &c.nous_portal {
        business::validate_context(payload, context)?;
    }
    Ok(())
}
