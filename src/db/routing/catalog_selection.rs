//! Filter provider/model witnesses before alias shadowing; no random or round-robin selection.
use super::*;

pub(super) fn collect_catalog_ids(
    policy: Option<&GatewayRoutePolicyConfig>,
    aliases: &[GatewayModelAliasRow],
    providers: &[GatewayProviderRouteRow],
    capabilities: &[ProviderCapabilityModelRow],
    defaults: &HashMap<String, Option<String>>,
    allows: &impl Fn(&str, &str, &str) -> bool,
) -> Vec<String> {
    let providers = providers
        .iter()
        .filter(|provider| provider_allowed_by_route_policy(provider, policy))
        .map(|provider| provider.id.as_str())
        .collect::<HashSet<_>>();
    let mut entries = Vec::new();
    let mut add = |provider: &str, model: &str, upstream: Option<&str>| {
        let actual = upstream.unwrap_or(model);
        if providers.contains(provider)
            && route_policy_allows_models(policy, [Some(model), upstream])
            && allows(provider, model, actual)
        {
            entries.push((provider.to_string(), model.to_string(), actual.to_string()));
        }
    };
    for row in aliases.iter().filter(|row| row.enabled) {
        add(
            &row.provider_account_id,
            &row.alias,
            row.upstream_model.as_deref(),
        );
    }
    for row in capabilities {
        add(
            &row.provider_account_id,
            &row.model_code,
            row.upstream_model.as_deref(),
        );
    }
    for (provider, model) in defaults {
        if let Some(model) = model {
            add(provider, model, None);
        }
    }
    let shadowed = entries
        .iter()
        .filter(|(_, model, upstream)| model != upstream)
        .map(|(provider, _, upstream)| (provider.as_str(), upstream.as_str()))
        .collect::<HashSet<_>>();
    entries
        .iter()
        .filter(|(provider, model, upstream)| {
            model != upstream || !shadowed.contains(&(provider.as_str(), upstream.as_str()))
        })
        .map(|(_, model, _)| model.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests;
