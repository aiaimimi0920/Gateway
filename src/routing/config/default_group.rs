//! Derived fallback membership never rewrites historical revision bytes or secrets.
use super::{
    effective_credential_id, provider_default_account_id, AccountGroupYaml, RouteConfigYaml,
};
use std::collections::HashSet;

pub(super) fn effective_groups(document: &RouteConfigYaml) -> Vec<AccountGroupYaml> {
    let mut groups = document.account_groups.clone();
    // Disabled groups still represent an explicit choice: do not bypass them
    // by silently moving their members into an enabled fallback group.
    let assigned = groups
        .iter()
        .filter(|g| g.id.trim() != "default")
        .flat_map(|g| g.provider_credential_ids.iter().map(|id| id.trim()))
        .collect::<HashSet<_>>();
    let mut members = Vec::new();
    let mut seen = HashSet::new();
    for provider in &document.providers {
        let ids = if provider.credentials.is_empty() {
            vec![provider_default_account_id(provider.id.trim())]
        } else {
            provider
                .credentials
                .iter()
                .enumerate()
                .map(|(index, credential)| {
                    effective_credential_id(&provider.id, index, credential)
                        .trim()
                        .to_owned()
                })
                .collect()
        };
        for id in ids {
            if !assigned.contains(id.as_str()) && seen.insert(id.clone()) {
                members.push(id);
            }
        }
    }
    if let Some(group) = groups.iter_mut().find(|g| g.id.trim() == "default") {
        group.id = "default".into();
        group.provider_credential_ids = members;
    } else {
        groups.push(AccountGroupYaml {
            id: "default".into(),
            name: "default".into(),
            description: None,
            billing_multiplier: Some(1.0),
            enabled: Some(true),
            notes: None,
            provider_credential_ids: members,
        });
    }
    groups
}
