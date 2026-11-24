//! Explicit credential-owned Amazon Quick OAuth declaration and payload configuration.
//! Rejects provider-level token inheritance, spoofing of internal `__gateway_quick_oauth` metadata,
//! and mixing Quick OAuth context with unrelated OAuth/session mechanisms.

use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::quick_oauth::{
    metadata::{self, METADATA_KEY},
    valid_token, AUTH_MODE, PORTAL_BASE, PRESET_ID,
};

pub(super) fn owns_provider(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some(PRESET_ID)
        || cfg.auth_mode.as_deref() == Some(AUTH_MODE)
        || cfg
            .credentials
            .iter()
            .any(|c| c.quick_oauth.is_some() || c.auth_mode.as_deref() == Some(AUTH_MODE))
}

pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if cfg.extra_body.contains_key(METADATA_KEY)
        || cfg
            .credentials
            .iter()
            .any(|c| c.extra_body.contains_key(METADATA_KEY))
    {
        anyhow::bail!("Internal Amazon Quick metadata cannot be supplied in extra_body.");
    }
    if !owns_provider(cfg) {
        return Ok(());
    }
    if cfg.preset.as_deref() != Some(PRESET_ID)
        || !cfg.explicit_credentials_only
        || !cfg.api_key.is_empty()
        || cfg.auth_token.is_some()
        || cfg.base_url.trim_end_matches('/') != PORTAL_BASE
    {
        anyhow::bail!("Invalid Amazon Quick OAuth provider declaration.");
    }
    for cred in &cfg.credentials {
        let Some(context) = &cred.quick_oauth else {
            anyhow::bail!("Amazon Quick provider credentials require quick_oauth context.");
        };
        context.validate_metadata()?;
        if context.client_secret().is_some() {
            context.validate()?;
        }
        if cred
            .auth_mode
            .as_deref()
            .is_some_and(|mode| mode != AUTH_MODE)
            || cred
                .base_url
                .as_deref()
                .is_some_and(|base| base.trim_end_matches('/') != PORTAL_BASE)
            || cred.discovery.is_some()
            || cred.discovery_job.is_some()
            || cred.copilot_github_token.is_some()
            || cred.grok_oauth.is_some()
            || cred.luma_oauth.is_some()
            || cred.kiro_oauth.is_some()
            || cred.vertex_oauth.is_some()
            || cred.dim_oauth.is_some()
            || cred.nous_portal.is_some()
            || cred.lobsterai.is_some()
            || cred.freebuff_session.is_some()
            || cred.accio_sgcookie.is_some()
            || cred
                .refresh_endpoint
                .as_deref()
                .is_some_and(|ep| ep != context.token_endpoint())
            || cred
                .refresh_client_id
                .as_deref()
                .is_some_and(|cid| cid != context.client_id())
            || cred
                .refresh_token
                .as_deref()
                .is_some_and(|rt| !valid_token(rt))
        {
            anyhow::bail!("Invalid Amazon Quick OAuth credential configuration.");
        }
    }
    Ok(())
}

pub(super) fn configure(
    credential: &ProviderCredentialYaml,
    payload: &mut ProviderAccountPayload,
) -> anyhow::Result<()> {
    let Some(context) = &credential.quick_oauth else {
        if crate::protocol::quick_oauth::owns_payload(payload) {
            anyhow::bail!("Amazon Quick runtime requires quick_oauth context.");
        }
        return Ok(());
    };
    if credential.api_key.is_none() && !payload.api_key.is_empty() {
        anyhow::bail!("Amazon Quick credential cannot inherit provider api_key.");
    }
    metadata::attach(payload, context);
    Ok(())
}
