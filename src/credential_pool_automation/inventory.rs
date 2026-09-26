use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use std::collections::HashSet;
const DEFAULT_TARGET_SIZE: usize = 1;

const MAX_TARGET_SIZE: usize = 10_000;

pub(super) fn normalized_target_size(value: Option<usize>) -> usize {
    value
        .unwrap_or(DEFAULT_TARGET_SIZE)
        .clamp(1, MAX_TARGET_SIZE)
}

pub(super) fn active_credential_count(provider: &ProviderConfigYaml) -> usize {
    if provider.credentials.is_empty() {
        usize::from(!provider.api_key.trim().is_empty() || provider.auth_token.is_some())
    } else {
        provider
            .credentials
            .iter()
            .filter(|credential| credential.enabled.unwrap_or(true))
            .count()
    }
}

pub(super) fn identity_category_requested_count(provider: &ProviderConfigYaml) -> usize {
    provider
        .credential_identity_categories
        .iter()
        .filter_map(serde_json::Value::as_object)
        .filter(|category| {
            category
                .get("auto_refill_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|category| {
            let category_id = category
                .get("id")
                .and_then(serde_json::Value::as_str)?
                .trim();
            if category_id.is_empty() {
                return None;
            }
            let target_size = category
                .get("pool_target_size")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .map(|value| value.clamp(1, MAX_TARGET_SIZE))
                .unwrap_or(DEFAULT_TARGET_SIZE);
            let active_count = provider
                .credentials
                .iter()
                .filter(|credential| credential.enabled.unwrap_or(true))
                .filter(|credential| {
                    credential.credential_identity_category_id.as_deref() == Some(category_id)
                })
                .count();
            Some(target_size.saturating_sub(active_count))
        })
        .fold(0usize, usize::saturating_add)
        .min(MAX_TARGET_SIZE)
}

pub(super) fn effective_credential_id(
    provider_id: &str,
    index: usize,
    credential: &ProviderCredentialYaml,
) -> String {
    credential
        .id
        .clone()
        .unwrap_or_else(|| format!("{provider_id}-cred-{index}"))
}

pub(super) fn prune_credentials(
    provider: &mut ProviderConfigYaml,
    prune_ids: &HashSet<String>,
) -> usize {
    let before_prune = provider.credentials.len();
    let provider_id = provider.id.clone();
    provider.credentials = std::mem::take(&mut provider.credentials)
        .into_iter()
        .enumerate()
        .filter_map(|(index, credential)| {
            let credential_id = effective_credential_id(&provider_id, index, &credential);
            (!prune_ids.contains(&credential_id)).then_some(credential)
        })
        .collect();
    before_prune.saturating_sub(provider.credentials.len())
}
