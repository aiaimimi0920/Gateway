//! Only an explicit credential registration may drive Kiro refresh; legacy accounts remain unchanged.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::kiro_oauth::business;

pub(super) fn validate_declaration(cfg: &ProviderConfigYaml) -> anyhow::Result<()> {
    if cfg.auth_mode.as_deref() == Some(crate::protocol::kiro_oauth::runtime::AUTH_MODE)
        && (cfg.preset.as_deref() != Some("kiro")
            || !cfg.explicit_credentials_only
            || !cfg.api_key.is_empty()
            || cfg.auth_token.is_some()
            || cfg.credentials.iter().any(|c| {
                c.kiro_oauth.is_none()
                    || c.auth_mode
                        .as_deref()
                        .is_some_and(|mode| mode != crate::protocol::kiro_oauth::runtime::AUTH_MODE)
                    || c.base_url.as_deref().is_some_and(|base| {
                        base.trim_end_matches('/') != cfg.base_url.trim_end_matches('/')
                    })
                    || c.discovery.is_some()
                    || c.discovery_job.is_some()
            }))
    {
        anyhow::bail!("Invalid Kiro registered OAuth runtime declaration.");
    }
    for credential in &cfg.credentials {
        let Some(context) = &credential.kiro_oauth else {
            continue;
        };
        context.validate_metadata()?;
        if context.client_secret().is_some() {
            context.validate()?;
        }
        if credential.grok_oauth.is_some()
            || credential.accio_sgcookie.is_some()
            || credential
                .refresh_endpoint
                .as_deref()
                .is_some_and(|endpoint| endpoint != context.token_endpoint())
            || credential
                .refresh_client_id
                .as_deref()
                .is_some_and(|client| client != context.client_id())
            || credential
                .refresh_token
                .as_deref()
                .is_some_and(|token| !business::valid_token(token))
        {
            anyhow::bail!("Invalid Kiro registered authorization context.");
        }
    }
    Ok(())
}

pub(super) fn configure(
    credential: &ProviderCredentialYaml,
    payload: &mut ProviderAccountPayload,
) -> anyhow::Result<()> {
    let Some(context) = &credential.kiro_oauth else {
        if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
            anyhow::bail!("Kiro runtime requires registered OAuth context.");
        }
        return Ok(());
    };
    // Explicit empty material remains clearable; never inherit the provider's access token.
    if credential.api_key.is_none() && !payload.api_key.is_empty() {
        anyhow::bail!("Kiro authorization cannot inherit provider access material.");
    }
    business::configure(payload, context)?;
    Ok(())
}
