//! A Cloudflare pool can defer the token audience to each explicit account.
use super::*;
use crate::protocol::cloudflare_workers_ai as cf;

pub(super) fn is_template(cfg: &ProviderConfigYaml) -> bool {
    cfg.preset.as_deref() == Some(cf::PRESET)
        && cfg.base_url.is_empty()
        && cfg.explicit_credentials_only
}

pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if cfg.preset.as_deref() != Some(cf::PRESET) && !cf::owns_payload(payload) {
        return Ok(());
    }
    cf::validate(payload, payload.canonical_adapter())?;
    if cfg.preset.as_deref() == Some(cf::PRESET) {
        anyhow::ensure!(
            payload.auth_mode.as_deref() == Some(cf::AUTH_MODE),
            "Cloudflare token identity cannot be overridden."
        );
    }
    anyhow::ensure!(
        payload.api_key.is_empty() || cf::valid_key(&payload.api_key),
        "Invalid Cloudflare API token."
    );
    Ok(())
}

pub(super) fn account_key<'a>(
    cfg: &ProviderConfigYaml,
    credential: &'a ProviderCredentialYaml,
) -> Option<&'a str> {
    // A legacy pool key may only be inherited at its original destination.
    let different_account = |base: &str| {
        (cfg.preset.as_deref() == Some(cf::PRESET) || cf::Endpoint::parse(base).is_some())
            && cf::Endpoint::parse(base).map(|e| e.base_url)
                != cf::Endpoint::parse(&cfg.base_url).map(|e| e.base_url)
    };
    credential.api_key.as_deref().or_else(|| {
        credential
            .base_url
            .as_deref()
            .filter(|base| different_account(base))
            .map(|_| "")
    })
}

pub(super) fn validate_account(
    cfg: &ProviderConfigYaml,
    credential: &ProviderCredentialYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    validate_final(cfg, payload)?;
    if cfg.preset.as_deref() == Some(cf::PRESET) || cf::owns_payload(payload) {
        let declared = credential.base_url.as_deref().unwrap_or(&cfg.base_url);
        let declared = cf::Endpoint::parse(declared).ok_or_else(cf::invalid)?;
        let effective = cf::Endpoint::parse(&payload.base_url).ok_or_else(cf::invalid)?;
        anyhow::ensure!(
            declared.base_url == effective.base_url && credential.refresh_token.is_none(),
            "Cloudflare account destination and token cannot be overridden by discovery or refresh."
        );
    }
    Ok(())
}
