//! Alias/provider association projections with stable ordering.

use super::identity::{
    filter_visible_provider_accounts, read_provider_default_model, source_profile_from_account,
};
use super::*;

pub async fn get_model_association_matrix(
    pool: &PgPool,
) -> Result<GatewayModelAssociationMatrixView, GatewayError> {
    let (provider_accounts, model_aliases) = tokio::try_join!(
        list_provider_accounts(pool),
        super::list_model_aliases(pool, None),
    )?;
    let provider_accounts = filter_visible_provider_accounts(provider_accounts);

    let providers_by_id = provider_accounts
        .iter()
        .map(|provider| (provider.id.clone(), provider.clone()))
        .collect::<HashMap<_, _>>();

    let mut alias_groups = HashMap::<
        String,
        (
            String,
            Option<String>,
            String,
            Vec<GatewayModelAssociationProviderLinkView>,
        ),
    >::new();
    let mut provider_groups = HashMap::<
        String,
        (
            GatewayProviderAccountView,
            Vec<GatewayModelAssociationProviderAliasLinkView>,
        ),
    >::new();

    for alias in model_aliases {
        let Some(provider) = providers_by_id.get(&alias.provider_account_id) else {
            continue;
        };
        let alias_group_key = format!(
            "{}::{}::{}",
            alias.project_id.as_deref().unwrap_or("__platform__"),
            alias.scope_type,
            alias.alias
        );
        let provider_link = GatewayModelAssociationProviderLinkView {
            provider_account_id: provider.id.clone(),
            label: provider.label.clone(),
            adapter: provider.adapter.clone(),
            protocol_family: provider.protocol_family.clone(),
            status: provider.status.clone(),
            source_profile: source_profile_from_account(provider),
            upstream_model: alias.upstream_model.clone(),
            priority: alias.priority,
            weight: alias.weight,
            enabled: alias.enabled,
            default_model: read_provider_default_model(&provider.payload),
        };
        alias_groups
            .entry(alias_group_key)
            .and_modify(|(_, _, _, providers)| providers.push(provider_link.clone()))
            .or_insert_with(|| {
                (
                    alias.alias.clone(),
                    alias.project_id.clone(),
                    alias.scope_type.clone(),
                    vec![provider_link.clone()],
                )
            });

        let alias_link = GatewayModelAssociationProviderAliasLinkView {
            alias: alias.alias.clone(),
            project_id: alias.project_id.clone(),
            scope_type: alias.scope_type.clone(),
            upstream_model: alias.upstream_model.clone(),
            priority: alias.priority,
            weight: alias.weight,
            enabled: alias.enabled,
        };
        provider_groups
            .entry(provider.id.clone())
            .and_modify(|(_, aliases)| aliases.push(alias_link.clone()))
            .or_insert_with(|| (provider.clone(), vec![alias_link]));
    }

    let mut alias_rows = alias_groups
        .into_values()
        .map(|(alias, project_id, scope_type, mut providers)| {
            providers.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| left.label.cmp(&right.label))
            });
            let upstream_models = providers
                .iter()
                .filter_map(|provider| provider.upstream_model.clone())
                .collect::<std::collections::BTreeSet<_>>();
            let upstream_model = match upstream_models.len() {
                0 => None,
                1 => upstream_models.iter().next().cloned(),
                count => Some(format!("mixed ({count})")),
            };
            let mut source_kind_distribution = BTreeMap::new();
            for provider in &providers {
                *source_kind_distribution
                    .entry(provider.source_profile.source_kind.clone())
                    .or_insert(0usize) += 1;
            }
            GatewayModelAssociationAliasRowView {
                alias,
                project_id,
                scope_type,
                upstream_model,
                provider_count: providers.len(),
                enabled_provider_count: providers
                    .iter()
                    .filter(|provider| provider.enabled)
                    .count(),
                fallback_priority: build_fallback_priority_label(&providers),
                source_kind_distribution: summary_buckets(source_kind_distribution),
                providers,
            }
        })
        .collect::<Vec<_>>();
    alias_rows.sort_by(|left, right| {
        left.alias
            .cmp(&right.alias)
            .then_with(|| left.scope_type.cmp(&right.scope_type))
            .then_with(|| left.project_id.cmp(&right.project_id))
    });

    let mut provider_rows = provider_groups
        .into_values()
        .map(|(provider, mut aliases)| {
            aliases.sort_by(|left, right| {
                left.priority
                    .cmp(&right.priority)
                    .then_with(|| left.alias.cmp(&right.alias))
            });
            GatewayModelAssociationProviderRowView {
                provider_account_id: provider.id.clone(),
                label: provider.label.clone(),
                adapter: provider.adapter.clone(),
                protocol_family: provider.protocol_family.clone(),
                status: provider.status.clone(),
                source_profile: source_profile_from_account(&provider),
                default_model: read_provider_default_model(&provider.payload),
                supported_alias_count: aliases.len(),
                aliases,
            }
        })
        .collect::<Vec<_>>();
    provider_rows.sort_by(|left, right| left.label.cmp(&right.label));

    let mut by_source_kind = BTreeMap::new();
    let mut by_protocol_family = BTreeMap::new();
    for row in &provider_rows {
        *by_source_kind
            .entry(row.source_profile.source_kind.clone())
            .or_insert(0usize) += 1;
        *by_protocol_family
            .entry(row.protocol_family.clone())
            .or_insert(0usize) += 1;
    }

    Ok(GatewayModelAssociationMatrixView {
        summary: GatewayModelAssociationMatrixSummaryView {
            total_aliases: alias_rows.len(),
            total_providers: provider_rows.len(),
            total_links: alias_rows.iter().map(|row| row.providers.len()).sum(),
            by_source_kind: summary_buckets(by_source_kind),
            by_protocol_family: summary_buckets(by_protocol_family),
        },
        alias_rows,
        provider_rows,
    })
}

pub(super) fn build_fallback_priority_label(
    links: &[GatewayModelAssociationProviderLinkView],
) -> String {
    if links.is_empty() {
        return "未绑定 provider".to_string();
    }
    links
        .iter()
        .map(|link| format!("{}(P{})", link.label, link.priority))
        .collect::<Vec<_>>()
        .join(" -> ")
}
