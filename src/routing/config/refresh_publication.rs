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

fn unchanged(a: &ProviderCredentialYaml, b: &ProviderCredentialYaml) -> bool {
    a.api_key == b.api_key
        && a.auth_token == b.auth_token
        && a.refresh_token == b.refresh_token
        && a.refresh_endpoint == b.refresh_endpoint
        && a.refresh_client_id == b.refresh_client_id
        && a.expires_at == b.expires_at
        && a.headers == b.headers
}

pub(super) fn reuse_and_retire(
    current: &RouteConfigSnapshot,
    replacement: &mut RouteConfigInner,
    document: &RouteConfigYaml,
) {
    let mut retained = HashSet::new();
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
            // Preserve routing revocation boundaries, but not unrelated model/label/probe changes.
            if target.refresh_config.is_some()
                && unchanged(a, b)
                && old.payload.api_key == target.payload.api_key
                && old.payload.auth_token == target.payload.auth_token
                && old.payload.headers == target.payload.headers
            {
                // A policy change retires routing ownership, not an in-flight token exchange.
                // Share its mutex so a replacement owner cannot consume the same refresh token.
                if let Some(fresh) = target.refresh_config.as_mut().and_then(Arc::get_mut) {
                    fresh.attempt_lock = refresh.attempt_lock.clone();
                }
                if old.enabled == target.enabled
                    && old_provider.credential_pool_auth_policy
                        == provider.credential_pool_auth_policy
                {
                    target.refresh_config = Some(refresh.clone());
                    retained.insert(Arc::as_ptr(refresh) as usize);
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
            }
        }
    }
}
