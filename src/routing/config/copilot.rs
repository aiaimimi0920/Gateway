//! SDK identities use existing credential storage without projecting tokens into HTTP payloads.
use super::ProviderConfigYaml;

pub(crate) const AUTH_MODE: &str = "copilot_sdk";

pub(super) fn credential_enabled(credential: &super::ProviderCredentialYaml) -> bool {
    cfg!(feature = "copilot-sdk-runtime")
        && credential.enabled == Some(true)
        && credential
            .copilot_github_token
            .as_deref()
            .is_some_and(valid_token)
}

pub(crate) fn is_provider(provider: &ProviderConfigYaml) -> bool {
    provider.auth_mode.as_deref() == Some(AUTH_MODE)
}

pub(crate) fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 16384 && token.bytes().all(|b| b.is_ascii_graphic())
}

pub(super) fn validate_declaration(p: &ProviderConfigYaml) -> anyhow::Result<()> {
    let sdk = is_provider(p);
    anyhow::ensure!(
        sdk || p
            .credentials
            .iter()
            .all(|c| c.copilot_github_token.is_none()),
        "Copilot identity requires a dedicated SDK provider"
    );
    if !sdk {
        return Ok(());
    }
    // Empty endpoint is intentional. Even an out-of-band generic probe has no HTTP target.
    anyhow::ensure!(
        p.explicit_credentials_only
            && p.preset.is_none()
            && p.adapter.is_none()
            && p.base_url.is_empty()
            && p.api_key.is_empty()
            && p.auth_token.is_none()
            && p.headers.is_empty()
            && p.extra_body.is_empty()
            && p.session_auth.is_none()
            && p.keepalive.is_none()
            && p.site_health.is_none()
            && !p.scheduled_probe_enabled,
        "Copilot SDK provider cannot contain HTTP transport or authentication configuration"
    );
    for c in &p.credentials {
        anyhow::ensure!(
            c.auth_mode.is_none()
                && c.base_url.is_none()
                && c.api_key.is_none()
                && c.auth_token.is_none()
                && c.headers.is_empty()
                && c.extra_body.is_empty()
                && c.session_auth.is_none()
                && c.keepalive.is_none()
                && c.refresh_token.is_none()
                && c.refresh_endpoint.is_none()
                && c.refresh_client_id.is_none()
                && c.runtime_state_object_key.is_none()
                && c.discovery.is_none()
                && c.discovery_job.is_none()
                && c.model_catalog.is_none()
                && c.grok_oauth.is_none()
                && c.kiro_oauth.is_none()
                && c.quick_oauth.is_none()
                && c.vertex_oauth.is_none()
                && c.dim_oauth.is_none()
                && c.nous_portal.is_none()
                && c.lobsterai.is_none()
                && c.freebuff_session.is_none()
                && c.accio_sgcookie.is_none()
                && !c.scheduled_probe_enabled,
            "Copilot SDK credential cannot contain another transport or authentication mechanism"
        );
        anyhow::ensure!(
            c.copilot_github_token.as_deref().is_none_or(valid_token),
            "Invalid Copilot SDK identity"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
