//! Preserve original Express identity before preset/account/discovery overrides are merged.
use super::{ProviderAccountPayload, ProviderConfigYaml};
use crate::protocol::vertex_express as key;

fn owns(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some(key::PRESET_ID)
        || cfg.auth_mode.as_deref() == Some(key::AUTH_MODE)
        || key::owns_base(&cfg.base_url)
        || cfg.credentials.iter().any(|c| {
            c.auth_mode.as_deref() == Some(key::AUTH_MODE)
                || c.base_url.as_deref().is_some_and(key::owns_base)
        })
}
pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if cfg.preset.as_deref().is_some_and(|p| p != key::PRESET_ID)
        || cfg.protocol_profile.as_deref().is_some_and(|p| {
            crate::protocol::registry::canonicalize_protocol_profile_key(p) != key::PROFILE
        })
        || cfg.protocol_family.as_deref().is_some_and(|p| {
            crate::protocol::registry::canonicalize_protocol_family_key(p)
                != "gemini_generate_content"
        })
        || cfg.codex_basispoints_enabled
        || cfg.supported_models.iter().any(|m| !key::valid_model(m))
        || cfg.credentials.iter().any(|c| {
            c.refresh_token.is_some()
                || c.refresh_endpoint.is_some()
                || c.refresh_client_id.is_some()
                || c.token_expires_in_secs.is_some()
                || c.kiro_oauth.is_some()
                || c.grok_oauth.is_some()
                || c.accio_sgcookie.is_some()
                || c.discovery.is_some()
                || c.discovery_job.is_some()
                || c.model_catalog.is_some()
                || c.codex_basispoints_enabled == Some(true)
                || c.supported_models.iter().any(|m| !key::valid_model(m))
        })
    {
        return Err(key::invalid().into());
    }
    Ok(())
}
pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if !key::owns_payload(payload) {
        return Err(key::invalid().into());
    }
    key::validate(payload, payload.canonical_adapter())?;
    crate::protocol::call_flow::prepare(payload)?;
    Ok(())
}
