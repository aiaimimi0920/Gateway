//! Credential-owned DIM authority is validated before refresh scheduling, never inherited from a parent.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::dim_oauth::{business, Site};

pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    let typed = cfg.auth_mode.as_deref() == Some(business::AUTH_MODE)
        || business::owns_base(&cfg.base_url)
        || cfg.credentials.iter().any(|c| {
            c.dim_oauth.is_some()
                || c.dim_catalog.is_some()
                || c.auth_mode.as_deref() == Some(business::AUTH_MODE)
                || c.base_url.as_deref().is_some_and(business::owns_base)
        });
    if !typed {
        return Ok(());
    }
    let site = [Site::China, Site::Global]
        .into_iter()
        .find(|site| cfg.base_url.trim_end_matches('/') == business::base(*site))
        .ok_or_else(business::invalid)?;
    let preset = match site {
        Site::China => "dim-cn-oauth",
        Site::Global => "dim-global-oauth",
    };
    if cfg.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || !cfg.explicit_credentials_only
        || cfg.preset.as_deref().is_some_and(|p| p != preset)
        || cfg
            .adapter
            .as_deref()
            .is_some_and(|a| a != "openai_compatible")
    {
        return Err(business::invalid().into());
    }
    for c in &cfg.credentials {
        let context = c.dim_oauth.as_ref().ok_or_else(business::invalid)?;
        context.validate()?;
        if context.site() != site
            || c.grok_oauth.is_some()
            || c.kiro_oauth.is_some()
            || c.vertex_oauth.is_some()
            || c.accio_sgcookie.is_some()
            || c.freebuff_session.is_some()
            || c.refresh_endpoint
                .as_deref()
                .is_some_and(|s| s != context.token_endpoint())
            || c.refresh_client_id
                .as_deref()
                .is_some_and(|s| s != context.client_id())
            || c.refresh_token
                .as_deref()
                .is_none_or(|s| !business::valid_token(s))
            || c.api_key
                .as_deref()
                .is_none_or(|s| !business::valid_token(s))
            || c.discovery.is_some()
            || c.discovery_job.is_some()
            || c.codex_basispoints_enabled == Some(true)
            || cfg.codex_basispoints_enabled
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
    if let Some(context) = &c.dim_oauth {
        business::validate_context(payload, context)?;
        if let Some(catalog) = &c.dim_catalog {
            catalog.validate(context)?;
        }
    }
    Ok(())
}
