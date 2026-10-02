pub(super) use super::capacity::active_credential_count;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};
use std::collections::HashSet;

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

pub(super) fn current_prune_ids(
    ids: HashSet<String>,
    observed_revision: &str,
    current_revision: &str,
) -> HashSet<String> {
    if observed_revision == current_revision {
        ids
    } else {
        HashSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_prune_evidence_cannot_delete_a_repaired_credential_with_the_same_id() {
        let mut provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id":"p", "base_url":"https://example.invalid", "credential_permanent_delete_enabled":true,
            "credentials":[{"id":"same-id", "api_key":"repaired-key"}]
        })).unwrap();
        let decisions = HashSet::from(["same-id".to_string()]);
        let accepted = current_prune_ids(decisions.clone(), "before-repair", "after-repair");
        assert!(accepted.is_empty());
        assert_eq!(prune_credentials(&mut provider, &accepted), 0);
        assert_eq!(
            provider.credentials[0].api_key.as_deref(),
            Some("repaired-key")
        );
        assert_eq!(
            current_prune_ids(decisions, "same-revision", "same-revision").len(),
            1
        );
    }
}
