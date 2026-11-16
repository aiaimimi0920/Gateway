//! Original-preset anchoring prevents overrides from erasing the token's Build audience.
use super::{ProviderAccountPayload, ProviderConfigYaml};
use crate::protocol::{grok_oauth::business, registry};

fn owns(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some(business::PRESET_ID)
        || cfg.auth_mode.as_deref() == Some(business::AUTH_MODE)
        || business::owns_base(&cfg.base_url)
        || cfg.credentials.iter().any(|c| {
            c.grok_oauth.is_some()
                || c.auth_mode.as_deref() == Some(business::AUTH_MODE)
                || c.base_url.as_deref().is_some_and(business::owns_base)
        })
}
pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if cfg
        .protocol_profile
        .as_deref()
        .is_some_and(|p| registry::canonicalize_protocol_profile_key(p) != business::PROFILE)
        || cfg.protocol_family.as_deref().is_some_and(|f| {
            registry::canonicalize_protocol_family_key(f) != registry::OPENAI_RESPONSES_FAMILY
        })
        || cfg.codex_basispoints_enabled
        || cfg.credentials.iter().any(|c| {
            c.discovery.is_some()
                || c.discovery_job.is_some()
                || c.codex_basispoints_enabled == Some(true)
                || invalid_refresh(c)
        })
    {
        anyhow::bail!("Invalid Grok Build OAuth configuration or refresh context.");
    }
    Ok(())
}
fn invalid_refresh(c: &super::ProviderCredentialYaml) -> bool {
    let legacy = c.refresh_token.is_some()
        || c.refresh_endpoint.is_some()
        || c.refresh_client_id.is_some()
        || c.token_expires_in_secs.is_some();
    let Some(context) = &c.grok_oauth else {
        return legacy;
    };
    context.validate().is_err()
        || c.refresh_endpoint
            .as_deref()
            .is_some_and(|s| s != crate::protocol::grok_oauth::TOKEN_ENDPOINT)
        || c.refresh_client_id
            .as_deref()
            .is_some_and(|s| s != context.client_id())
        || c.refresh_token.as_deref().is_some_and(|s| {
            s.is_empty() || s.len() > 32_768 || !s.bytes().all(|b| b.is_ascii_graphic())
        })
        || c.token_expires_in_secs
            .is_some_and(|s| s == 0 || s > 100 * 365 * 86400)
}
pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if !owns(cfg) {
        return Ok(());
    }
    if !business::owns_payload(payload) {
        return Err(business::invalid().into());
    }
    business::validate(payload, payload.canonical_adapter())?;
    crate::protocol::call_flow::prepare(payload)?;
    // Empty access material remains clearable/storable, but validate_plan cannot send it.
    if !payload.api_key.is_empty() {
        business::validate_material(payload)?;
    }
    Ok(())
}
