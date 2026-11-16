//! One finite credential projection is shared by typed admission and durable recovery.
use super::persistence::credential;
use crate::routing::config::{effective_credential_id, ProviderCredentialYaml, RouteConfigYaml};

pub(super) fn project(
    before: &RouteConfigYaml,
    provider: &str,
    id: &str,
    material: &ProviderCredentialYaml,
) -> Option<RouteConfigYaml> {
    let previous = credential(before, provider, id)?;
    if previous.luma_oauth.is_some() || material.luma_oauth.is_some() {
        return crate::console::luma_oauth::refresh_publication::project(
            before, provider, id, material,
        );
    }
    let access = material
        .api_key
        .as_deref()
        .filter(|value| !value.is_empty())?;
    material
        .refresh_token
        .as_deref()
        .filter(|value| !value.is_empty())?;
    match (&previous.lobsterai, &material.lobsterai) {
        (Some(old), Some(next))
            if old.permits_rotation(next)
                && next
                    .validate_binding(access, material.refresh_token.as_deref()?)
                    .is_ok()
                && material.expires_at.is_none()
                && material.token_expires_in_secs.is_none() => {}
        (None, None) => {
            material.token_expires_in_secs.filter(|value| *value > 0)?;
            time::OffsetDateTime::parse(
                material.expires_at.as_deref()?,
                &time::format_description::well_known::Rfc3339,
            )
            .ok()?;
        }
        _ => return None,
    }
    if previous.kiro_oauth != material.kiro_oauth {
        return None;
    }
    let expiry = material.expires_at.as_deref();
    match (&previous.nous_portal, &material.nous_portal) {
        (Some(old), Some(next))
            if old.permits_rotation(next)
                && expiry.is_some_and(|expiry| next.matches_expiry(expiry))
                && next
                    .validate_binding(
                        access,
                        &crate::protocol::nous_portal::OAuthClient::new(next.client_id()).ok()?,
                    )
                    .is_ok() => {}
        (None, None) => {}
        _ => return None,
    }
    match (&previous.dim_oauth, &material.dim_oauth) {
        (Some(old), Some(next)) if old.permits_rotation(next) => {}
        (None, None) => {}
        _ => return None,
    }
    match (&previous.vertex_oauth, &material.vertex_oauth) {
        (Some(old), Some(next)) if old.permits_rotation(next) => {}
        (None, None) => {}
        _ => return None,
    }
    match (&previous.grok_oauth, &material.grok_oauth) {
        (Some(old), Some(next)) if old.permits_rotation(next, access) => {}
        (None, None) => {}
        _ => return None,
    }
    if previous.accio_sgcookie != material.accio_sgcookie {
        if material.refresh_endpoint.as_deref()
            != Some(crate::protocol::accio_oauth::REFRESH_ENDPOINT)
            || material.grok_oauth.is_some()
            || material.accio_sgcookie.is_none()
            || crate::protocol::accio_oauth::RefreshGrant::new(
                access,
                material.refresh_token.as_deref()?,
                material.accio_sgcookie.as_deref(),
            )
            .is_err()
        {
            return None;
        }
    }
    let mut document = before.clone();
    let target_provider = document
        .providers
        .iter_mut()
        .find(|entry| entry.id == provider)?;
    let index = target_provider
        .credentials
        .iter()
        .enumerate()
        .position(|(index, entry)| effective_credential_id(provider, index, entry) == id)?;
    let target = &mut target_provider.credentials[index];
    if previous
        .nous_portal
        .as_ref()
        .zip(material.nous_portal.as_ref())
        .is_some_and(|(old, next)| old.inference_target() != next.inference_target())
    {
        target.base_url = Some(
            material
                .nous_portal
                .as_ref()?
                .inference_target()
                .base_url()
                .into(),
        );
    }
    target.api_key = material.api_key.clone();
    target.refresh_token = material.refresh_token.clone();
    target.token_expires_in_secs = material.token_expires_in_secs;
    target.expires_at = material.expires_at.clone();
    target.grok_oauth = material.grok_oauth.clone();
    target.vertex_oauth = material.vertex_oauth.clone();
    target.dim_oauth = material.dim_oauth.clone();
    target.nous_portal = material.nous_portal.clone();
    target.lobsterai = material.lobsterai.clone();
    target.accio_sgcookie = material.accio_sgcookie.clone();
    if let Some(session) = &mut target.session_auth {
        session.expires_at = material.expires_at.clone();
    }
    Some(document)
}

pub(crate) fn is_refresh_projection(
    before: &RouteConfigYaml,
    after: &RouteConfigYaml,
    provider: &str,
    id: &str,
) -> bool {
    credential(after, provider, id)
        .and_then(|material| project(before, provider, id, material))
        .and_then(|expected| serde_json::to_value(expected).ok())
        .zip(serde_json::to_value(after).ok())
        .is_some_and(|(expected, actual)| expected == actual)
}
