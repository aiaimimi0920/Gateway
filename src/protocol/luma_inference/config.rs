//! Preserve declared product identity before account/preset overrides can erase it.
use crate::routing::config::ProviderConfigYaml;
pub(crate) fn owns(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some(super::PRESET)
        || cfg.protocol_profile.as_deref() == Some(super::PROFILE)
        || cfg.auth_mode.as_deref() == Some(super::AUTH_MODE)
        || super::owns_base(&cfg.base_url)
        || cfg.credentials.iter().any(|c| {
            c.auth_mode.as_deref() == Some(super::AUTH_MODE)
                || c.base_url.as_deref().is_some_and(super::owns_base)
        })
}
pub(crate) fn validate(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    crate::implementation_lines::ensure_protocol_profile_compiled(super::PROFILE)?;
    anyhow::ensure!(
        cfg.preset.as_deref() == Some(super::PRESET)
            && cfg.base_url.trim_end_matches('/') == super::BASE
            && cfg
                .protocol_profile
                .as_deref()
                .is_none_or(|v| v == super::PROFILE)
            && cfg
                .auth_mode
                .as_deref()
                .is_none_or(|v| v == super::AUTH_MODE)
            && cfg.credentials.iter().all(|c| c
                .base_url
                .as_deref()
                .is_none_or(|v| v.trim_end_matches('/') == super::BASE)
                && c.auth_mode.as_deref().is_none_or(|v| v == super::AUTH_MODE)
                && c.refresh_token.is_none()
                && c.refresh_endpoint.is_none()
                && c.refresh_client_id.is_none()
                && c.token_expires_in_secs.is_none()
                && c.luma_oauth.is_none()
                && c.grok_oauth.is_none()
                && c.kiro_oauth.is_none()
                && c.vertex_oauth.is_none()
                && c.freebuff_session.is_none()
                && c.dim_oauth.is_none()
                && c.nous_portal.is_none()
                && c.discovery.is_none()
                && c.discovery_job.is_none()
                && c.model_catalog.is_none()),
        "Luma inference must retain its independent static Key product and target."
    );
    Ok(())
}
