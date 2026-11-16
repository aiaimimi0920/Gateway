//! The pool's Account ID is the token audience; credentials may not silently override it.
use super::*;
use crate::protocol::cloudflare_workers_ai as cf;

pub(super) fn validate_final(
    cfg: &ProviderConfigYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if cfg.preset.as_deref() != Some(cf::PRESET) && !cf::owns_payload(payload) {
        return Ok(());
    }
    let parent = cf::Endpoint::parse(&cfg.base_url).ok_or_else(cf::invalid)?;
    let account = cf::Endpoint::parse(&payload.base_url).ok_or_else(cf::invalid)?;
    anyhow::ensure!(
        parent.base_url == account.base_url,
        "Cloudflare credential Account ID must match its pool."
    );
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
