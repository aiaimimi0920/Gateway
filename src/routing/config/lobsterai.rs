//! Lobster personal sessions own their target and refresh metadata; expiry stays absent.
use super::{ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::lobsterai::{AUTH_MODE, CLIENT_ATTRIBUTION, TOKEN_ENDPOINT};
pub(crate) const BASE: &str = "https://lobsterai-server.youdao.com/api/proxy/v1";
pub(super) fn validate_declaration(p: &ProviderConfigYaml) -> anyhow::Result<()> {
    if p.auth_mode.as_deref() != Some(AUTH_MODE)
        && !p.credentials.iter().any(|c| {
            c.lobsterai.is_some()
                || c.lobsterai_catalog.is_some()
                || c.auth_mode.as_deref() == Some(AUTH_MODE)
        })
    {
        return Ok(());
    }
    if p.auth_mode.as_deref() != Some(AUTH_MODE)
        || !p.explicit_credentials_only
        || p.base_url != BASE
        || p.adapter.as_deref() != Some("openai_compatible")
        || p.preset.is_some()
        || !p.api_key.is_empty()
        || p.auth_token.is_some()
        || !p.headers.is_empty()
        || !p.extra_body.is_empty()
        || p.session_auth.is_some()
        || p.expires_at.is_some()
        || p.codex_basispoints_enabled
    {
        anyhow::bail!("lobsterai_invalid_configuration");
    }
    for c in &p.credentials {
        validate_credential(c)?;
    }
    Ok(())
}
fn validate_credential(c: &ProviderCredentialYaml) -> anyhow::Result<()> {
    if let Some(catalog) = &c.lobsterai_catalog {
        catalog.validate()?;
    }
    let context = c
        .lobsterai
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("lobsterai_context_required"))?;
    context.validate_binding(
        c.api_key.as_deref().unwrap_or_default(),
        c.refresh_token.as_deref().unwrap_or_default(),
    )?;
    if c.base_url.as_deref().is_some_and(|v| v != BASE)
        || c.auth_mode.as_deref().is_some_and(|v| v != AUTH_MODE)
        || c.refresh_endpoint
            .as_deref()
            .is_some_and(|v| v != TOKEN_ENDPOINT)
        || c.refresh_client_id
            .as_deref()
            .is_some_and(|v| v != CLIENT_ATTRIBUTION)
        || c.expires_at.is_some()
        || c.token_expires_in_secs.is_some()
        || c.session_auth.is_some()
        || c.auth_token.is_some()
        || !c.headers.is_empty()
        || !c.extra_body.is_empty()
        || c.grok_oauth.is_some()
        || c.kiro_oauth.is_some()
        || c.vertex_oauth.is_some()
        || c.dim_oauth.is_some()
        || c.nous_portal.is_some()
        || c.accio_sgcookie.is_some()
        || c.freebuff_session.is_some()
        || c.discovery.is_some()
        || c.discovery_job.is_some()
        || c.codex_basispoints_enabled == Some(true)
    {
        anyhow::bail!("lobsterai_invalid_configuration");
    }
    Ok(())
}

pub(super) fn configure(
    cred: &ProviderCredentialYaml,
    payload: &mut crate::routing::candidate::ProviderAccountPayload,
) -> anyhow::Result<()> {
    if let Some(context) = &cred.lobsterai {
        crate::protocol::lobsterai::metadata::configure(
            payload,
            context.clone(),
            cred.lobsterai_catalog.clone(),
        )?;
    }
    Ok(())
}
