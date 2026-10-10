//! Account/group inventory projection from one immutable snapshot.

use super::*;

pub(super) fn build_route_account_group_inventory(
    document: &RouteConfigYaml,
    compiled: &RouteConfigInner,
    effective_groups: &[AccountGroupYaml],
) -> RouteAccountGroupInventory {
    let provider_config_by_id = document
        .providers
        .iter()
        .map(|provider| (provider.id.as_str(), provider))
        .collect::<HashMap<_, _>>();
    let mut memberships: HashMap<&str, Vec<&AccountGroupYaml>> = HashMap::new();
    for group in effective_groups {
        for account_id in &group.provider_credential_ids {
            let trimmed = account_id.trim();
            if !trimmed.is_empty() {
                memberships.entry(trimmed).or_default().push(group);
            }
        }
    }

    let mut accounts = Vec::new();
    let mut providers = Vec::new();
    let mut account_provider_map = HashMap::<String, String>::new();

    for provider in &compiled.providers {
        let provider_config = provider_config_by_id.get(provider.id.as_str()).copied();
        let provider_vendor_key =
            provider_config.and_then(|config| trim_optional_field(config.vendor_key.as_deref()));
        let provider_vendor_name =
            provider_config.and_then(|config| trim_optional_field(config.vendor_name.as_deref()));
        let provider_preset =
            provider_config.and_then(|config| trim_optional_field(config.preset.as_deref()));
        let provider_base_url = trim_optional_field(Some(provider.payload.base_url.as_str()));
        let mut provider_account_ids = Vec::new();
        if provider.credential_pool.is_empty() {
            let account_id = provider_default_account_id(&provider.id);
            let account_groups = memberships
                .get(account_id.as_str())
                .cloned()
                .unwrap_or_default();
            let display_name = provider
                .payload
                .account_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| provider.label.clone());
            provider_account_ids.push(account_id.clone());
            account_provider_map.insert(account_id.clone(), provider.id.clone());
            accounts.push(RouteAccountView {
                id: account_id,
                display_name,
                provider_id: provider.id.clone(),
                provider_label: provider.label.clone(),
                vendor_key: provider_vendor_key.clone(),
                vendor_name: provider_vendor_name.clone(),
                provider_preset: provider_preset.clone(),
                credential_id: None,
                base_url: provider_base_url.clone(),
                mode: "provider_default".to_string(),
                enabled: true,
                supported_models: provider.supported_models.clone(),
                group_ids: account_groups
                    .iter()
                    .map(|group| group.id.trim().to_string())
                    .collect(),
            });
        } else {
            for credential in &provider.credential_pool {
                let account_id = credential.id.trim().to_string();
                let account_groups = memberships
                    .get(account_id.as_str())
                    .cloned()
                    .unwrap_or_default();
                let display_name = credential
                    .payload
                    .account_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| account_id.clone());
                provider_account_ids.push(account_id.clone());
                account_provider_map.insert(account_id.clone(), provider.id.clone());
                accounts.push(RouteAccountView {
                    id: account_id.clone(),
                    display_name,
                    provider_id: provider.id.clone(),
                    provider_label: provider.label.clone(),
                    vendor_key: provider_vendor_key.clone(),
                    vendor_name: provider_vendor_name.clone(),
                    provider_preset: provider_preset.clone(),
                    credential_id: Some(account_id),
                    base_url: trim_optional_field(Some(credential.payload.base_url.as_str())),
                    mode: "credential".to_string(),
                    enabled: credential.enabled,
                    supported_models: if credential.supported_models.is_empty() {
                        provider.supported_models.clone()
                    } else {
                        credential.supported_models.clone()
                    },
                    group_ids: account_groups
                        .iter()
                        .map(|group| group.id.trim().to_string())
                        .collect(),
                });
            }
        }

        providers.push(RouteAccountProviderView {
            id: provider.id.clone(),
            label: provider.label.clone(),
            vendor_key: provider_vendor_key,
            vendor_name: provider_vendor_name,
            preset: provider_preset,
            base_url: provider_base_url,
            account_ids: provider_account_ids,
            supported_models: provider.supported_models.clone(),
        });
    }

    let mut account_groups = effective_groups
        .iter()
        .map(|group| {
            let mut provider_ids = BTreeSet::new();
            let provider_credential_ids = group
                .provider_credential_ids
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            for account_id in &provider_credential_ids {
                if let Some(provider_id) = account_provider_map.get(account_id) {
                    provider_ids.insert(provider_id.clone());
                }
            }

            RouteAccountGroupView {
                id: group.id.trim().to_string(),
                name: group.name.trim().to_string(),
                description: trim_optional_field(group.description.as_deref()),
                billing_multiplier: group.billing_multiplier.unwrap_or(1.0),
                configured_billing_multiplier: group.billing_multiplier,
                enabled: group.enabled.unwrap_or(true),
                notes: trim_optional_field(group.notes.as_deref()),
                member_count: provider_credential_ids.len(),
                provider_credential_ids,
                providers: provider_ids.into_iter().collect(),
            }
        })
        .collect::<Vec<_>>();

    account_groups.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    accounts.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then(left.id.cmp(&right.id))
    });
    providers.sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));

    RouteAccountGroupInventory {
        account_groups,
        accounts,
        providers,
    }
}

fn trim_optional_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
