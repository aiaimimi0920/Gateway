//! OAuth state belongs to an authorization, not every unrelated provider setting.
use super::*;

impl RouteConfigSnapshot {
    pub(crate) fn oauth_refresh_targets(&self) -> Vec<(String, String)> {
        self.compiled()
            .providers
            .iter()
            .flat_map(|p| {
                p.credential_pool
                    .iter()
                    .filter(|c| c.enabled && c.refresh_config.is_some())
                    .map(|c| (p.id.clone(), c.id.clone()))
            })
            .collect()
    }

    pub(crate) fn oauth_refresh_target(
        &self,
        provider: &str,
        credential: &str,
    ) -> Option<(&CompiledProvider, &CompiledCredential)> {
        let provider = self
            .compiled()
            .providers
            .iter()
            .find(|p| p.id == provider)?;
        let credential = provider
            .credential_pool
            .iter()
            .find(|c| c.id == credential)?;
        Some((provider, credential))
    }
}

fn credential<'a>(
    document: &'a RouteConfigYaml,
    provider: &str,
    id: &str,
) -> Option<&'a ProviderCredentialYaml> {
    let provider = document.providers.iter().find(|p| p.id == provider)?;
    provider
        .credentials
        .iter()
        .enumerate()
        .find(|(index, c)| effective_credential_id(&provider.id, *index, c) == id)
        .map(|(_, c)| c)
}

pub(crate) fn same_authorization(a: &ProviderCredentialYaml, b: &ProviderCredentialYaml) -> bool {
    a.api_key == b.api_key
        && a.auth_token == b.auth_token
        && a.refresh_token == b.refresh_token
        && a.refresh_endpoint == b.refresh_endpoint
        && a.refresh_client_id == b.refresh_client_id
        && a.grok_oauth == b.grok_oauth
        && a.luma_oauth == b.luma_oauth
        && a.kiro_oauth == b.kiro_oauth
        && a.quick_oauth == b.quick_oauth
        && a.vertex_oauth == b.vertex_oauth
        && a.dim_oauth == b.dim_oauth
        && a.nous_portal == b.nous_portal
        && a.lobsterai == b.lobsterai
        && a.accio_sgcookie == b.accio_sgcookie
        && a.expires_at == b.expires_at
        && a.token_expires_in_secs == b.token_expires_in_secs
        && a.session_auth == b.session_auth
        && a.headers == b.headers
}

fn same_session_transport(a: Option<&SessionAuthConfig>, b: Option<&SessionAuthConfig>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.transport == b.transport
                && a.primary_cookie_name == b.primary_cookie_name
                && a.secondary_cookie_name == b.secondary_cookie_name
                && a.header_name == b.header_name
        }
        _ => false,
    }
}

pub(super) fn reuse_and_retire(
    current: &RouteConfigSnapshot,
    replacement: &mut RouteConfigInner,
    document: &RouteConfigYaml,
    persisted_rotation: bool,
) {
    let mut retained = HashSet::new();
    let mut retained_authorizations = HashSet::new();
    for provider in &mut replacement.providers {
        let Some(old_provider) = current
            .compiled()
            .providers
            .iter()
            .find(|p| p.id == provider.id)
        else {
            continue;
        };
        for target in &mut provider.credential_pool {
            let Some(old) = old_provider
                .credential_pool
                .iter()
                .find(|c| c.id == target.id)
            else {
                continue;
            };
            let (Some(a), Some(b), Some(refresh)) = (
                credential(current.document(), &provider.id, &target.id),
                credential(document, &provider.id, &target.id),
                old.refresh_config.as_ref(),
            ) else {
                continue;
            };
            // A document is the configuration anchor, not a replacement for newer runtime material.
            if target.refresh_config.is_some()
                && old.payload.adapter == target.payload.adapter
                && old.payload.auth_mode == target.payload.auth_mode
                && old.payload.auth_header_name == target.payload.auth_header_name
                && old.payload.auth_token == target.payload.auth_token
                && old.payload.headers == target.payload.headers
                && same_session_transport(
                    old.payload.session_auth.as_ref(),
                    target.payload.session_auth.as_ref(),
                )
                && ((same_authorization(a, b)
                    && old.payload.base_url == target.payload.base_url
                    && old.payload.api_key == target.payload.api_key
                    && old.payload.session_auth == target.payload.session_auth
                    && old.payload.expires_at == target.payload.expires_at)
                    || (persisted_rotation
                        && refresh.persisted_rotation_matches(
                            a,
                            b,
                            &target.payload.api_key,
                            &target.payload.base_url,
                            &old.payload.base_url,
                        )))
            {
                retained_authorizations.insert(refresh.authorization_id());
                if old.enabled == target.enabled
                    && old_provider.credential_pool_auth_policy
                        == provider.credential_pool_auth_policy
                {
                    target.refresh_config = Some(refresh.clone());
                    retained.insert(Arc::as_ptr(refresh) as usize);
                } else {
                    target.refresh_config = Some(Arc::new(refresh.routing_replacement()));
                }
            }
        }
    }
    for provider in &current.compiled().providers {
        for credential in &provider.credential_pool {
            if let Some(refresh) = &credential.refresh_config {
                if !retained.contains(&(Arc::as_ptr(refresh) as usize)) {
                    refresh.retire();
                }
                if !retained_authorizations.contains(&refresh.authorization_id()) {
                    refresh.revoke_authorization();
                }
            }
        }
    }
}
