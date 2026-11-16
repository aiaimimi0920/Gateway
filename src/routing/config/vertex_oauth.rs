//! Original declaration and final credential validation keep Full Vertex grants credential-local.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::vertex_oauth::{business, TOKEN_ENDPOINT};

pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    let typed = cfg.auth_mode.as_deref() == Some(business::AUTH_MODE)
        || cfg.credentials.iter().any(|c| {
            c.vertex_oauth.is_some() || c.auth_mode.as_deref() == Some(business::AUTH_MODE)
        });
    if !typed {
        return Ok(());
    }
    if cfg.auth_mode.as_deref() != Some(business::AUTH_MODE)
        || cfg.preset.as_deref().is_some_and(|p| {
            !matches!(
                p,
                "google-agent-platform"
                    | "google-agent-platform-official-api"
                    | "vertex-gemini"
                    | "vertex-official-api"
            )
        })
        || cfg.protocol_family.as_deref().is_some_and(|p| {
            crate::protocol::registry::canonicalize_protocol_family_key(p)
                != "gemini_generate_content"
        })
        || cfg.protocol_profile.as_deref().is_some_and(|p| {
            crate::protocol::registry::canonicalize_protocol_profile_key(p)
                != "google_agent_platform_official_api"
        })
        || cfg.codex_basispoints_enabled
    {
        return Err(business::invalid().into());
    }
    for credential in &cfg.credentials {
        let context = credential
            .vertex_oauth
            .as_ref()
            .ok_or_else(business::invalid)?;
        context.validate()?;
        if credential.grok_oauth.is_some()
            || credential.kiro_oauth.is_some()
            || credential.accio_sgcookie.is_some()
            || credential
                .refresh_endpoint
                .as_deref()
                .is_some_and(|s| s != TOKEN_ENDPOINT)
            || credential
                .refresh_client_id
                .as_deref()
                .is_some_and(|s| s != context.client_id())
            || credential
                .refresh_token
                .as_deref()
                .is_some_and(|s| !business::valid_token(s))
            || credential.discovery.is_some()
            || credential.discovery_job.is_some()
            || credential.codex_basispoints_enabled == Some(true)
            || credential
                .supported_models
                .iter()
                .any(|m| !crate::protocol::vertex_express::valid_model(m))
        {
            return Err(business::invalid().into());
        }
    }
    Ok(())
}
pub(super) fn configure(
    c: &ProviderCredentialYaml,
    payload: &mut ProviderAccountPayload,
) -> anyhow::Result<()> {
    let Some(context) = &c.vertex_oauth else {
        return Ok(());
    };
    // A derived quota header is metadata, never an arbitrary provider/caller override.
    if !payload.headers.is_empty() || c.api_key.is_none() && !payload.api_key.is_empty() {
        return Err(business::invalid().into());
    }
    if let Some(quota) = context.target().quota_project() {
        payload
            .headers
            .insert("x-goog-user-project".into(), quota.into());
    }
    business::validate_context(payload, context)?;
    crate::protocol::call_flow::prepare(payload)?;
    Ok(())
}
