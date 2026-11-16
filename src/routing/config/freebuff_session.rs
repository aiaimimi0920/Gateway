//! Strict product binding only for new typed CLI receipts and management login. Legacy adapter stays generic.
use super::{ProviderAccountPayload, ProviderConfigYaml, ProviderCredentialYaml};
use crate::protocol::{
    call_flow::{CallAdapter, SpecialAdapter},
    freebuff_login::LoginError,
};

pub(crate) fn owns_base(value: &str) -> bool {
    value == value.trim()
        && url::Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("freebuff.com")
                && url.port_or_known_default() == Some(443)
                && url.path() == "/"
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        })
}
pub(crate) fn path_key(key: &str) -> bool {
    let key = normalized_key(key);
    matches!(
        key.as_str(),
        "freebuffagentrunspath"
            | "freebuffstartrunpath"
            | "freebufffinishrunpath"
            | "freebuffsessionpath"
    )
}
fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn auth_copy_key(key: &str) -> bool {
    matches!(
        normalized_key(key).as_str(),
        "authtoken"
            | "apikey"
            | "token"
            | "refreshtoken"
            | "fingerprintid"
            | "fingerprinthash"
            | "freebuffsession"
            | "user"
    )
}
pub(super) fn validate_declaration(p: &ProviderConfigYaml) -> anyhow::Result<()> {
    use crate::protocol::freebuff::business_target;
    if p.extra_body
        .keys()
        .any(|key| business_target::is_config_key(key))
    {
        return Err(business_target::invalid().into());
    }
    for c in &p.credentials {
        if business_target::read(Some(&c.extra_body))?.is_some() {
            validate_owner(p, Some(c))?;
        }
        if let Some(context) = &c.freebuff_session {
            context.validate()?;
            if context.fingerprint_id().is_none()
                && c.api_key.as_deref().is_some_and(|s| !s.is_empty())
            {
                return Err(LoginError::InvalidInput.into());
            }
            validate_owner(p, Some(c))?;
        }
    }
    Ok(())
}
pub(super) fn validate_final(
    p: &ProviderConfigYaml,
    c: &ProviderCredentialYaml,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if c.freebuff_session.is_some()
        || crate::protocol::freebuff::business_target::read(Some(&c.extra_body))?.is_some()
    {
        validate_target(p, Some(c), payload)?;
    }
    Ok(())
}
fn validate_owner(
    p: &ProviderConfigYaml,
    c: Option<&ProviderCredentialYaml>,
) -> Result<(), LoginError> {
    if p.preset.as_deref() != Some("freebuff")
        || !p.api_key.is_empty()
        || p.auth_token.is_some()
        || p.codex_basispoints_enabled
        || c.is_some_and(|c| {
            c.grok_oauth.is_some()
                || c.kiro_oauth.is_some()
                || c.vertex_oauth.is_some()
                || c.accio_sgcookie.is_some()
                || c.refresh_token.is_some()
                || c.refresh_endpoint.is_some()
                || c.refresh_client_id.is_some()
                || c.token_expires_in_secs.is_some()
                || c.discovery.is_some()
                || c.discovery_job.is_some()
                || c.codex_basispoints_enabled == Some(true)
        })
    {
        return Err(LoginError::InvalidInput);
    }
    Ok(())
}
pub(crate) fn validate_target(
    p: &ProviderConfigYaml,
    c: Option<&ProviderCredentialYaml>,
    payload: &ProviderAccountPayload,
) -> Result<(), LoginError> {
    validate_owner(p, c)?;
    if !owns_base(&payload.base_url)
        || payload.canonical_adapter() != "freebuff_compatible"
        || payload.auth_token.is_some()
        || payload.auth_mode.is_some()
        || !payload.headers.is_empty()
        || payload.session_auth.is_some()
        || payload.keepalive.is_some()
        || payload.runtime_state_object_key.is_some()
        || payload
            .request_client_profile
            .is_some_and(|p| p != crate::protocol::request_profile::RequestClientProfile::Auto)
        || payload
            .execution_mode
            .is_some_and(|m| m != crate::routing::candidate::ProviderExecutionMode::DirectHttp)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|m| !m.is_empty())
        || payload
            .chat_completions_path
            .as_deref()
            .is_some_and(|p| p != crate::protocol::freebuff::FREEBUFF_DEFAULT_CHAT_COMPLETIONS_PATH)
        || payload
            .extra_body
            .as_ref()
            .is_some_and(|m| m.keys().any(|k| path_key(k) || auth_copy_key(k)))
        || payload.call_flow.is_some_and(|flow| {
            flow.call_adapter != CallAdapter::Auto
                || !matches!(
                    flow.special_adapter,
                    SpecialAdapter::Auto | SpecialAdapter::Freebuff
                )
        })
    {
        return Err(LoginError::InvalidInput);
    }
    if !payload.api_key.is_empty()
        && !crate::protocol::freebuff_login::atom(&payload.api_key, 32_768)
    {
        return Err(LoginError::InvalidInput);
    }
    crate::protocol::call_flow::prepare(payload).map_err(|_| LoginError::InvalidInput)?;
    Ok(())
}
