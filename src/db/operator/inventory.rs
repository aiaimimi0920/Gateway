//! Provider inventory assembly and service-family summary.

use super::cost_hints::build_provider_cost_hints;
use super::health::build_provider_health;
use super::identity::{filter_visible_provider_accounts, source_profile_from_account};
use super::runtime_state::read_breaker_open_map;
use super::usage_queries::{
    load_catalog_metadata, load_provider_supported_models, load_provider_usage_aggregates,
};
use super::*;

pub async fn get_provider_inventory(
    pool: &PgPool,
    redis_pool: &Pool,
    concurrency_snapshots: &HashMap<String, ConcurrencySnapshot>,
) -> Result<GatewayProviderInventoryView, GatewayError> {
    let (provider_accounts, catalog_metadata) =
        tokio::try_join!(list_provider_accounts(pool), load_catalog_metadata(pool),)?;
    let provider_accounts = filter_visible_provider_accounts(provider_accounts);
    let provider_account_ids = provider_accounts
        .iter()
        .map(|provider| provider.id.clone())
        .collect::<Vec<_>>();
    let (usage_by_provider, provider_supported_models, provider_credentials) = tokio::try_join!(
        load_provider_usage_aggregates(pool, &provider_account_ids),
        load_provider_supported_models(pool, &provider_account_ids),
        list_active_provider_credential_refs_for_accounts(pool, &provider_account_ids),
    )?;
    let mut credential_ids_by_account: HashMap<String, Vec<String>> = HashMap::new();
    for credential in provider_credentials {
        credential_ids_by_account
            .entry(credential.provider_account_id.clone())
            .or_default()
            .push(credential.id);
    }

    let breaker_map = read_breaker_open_map(
        redis_pool,
        &provider_accounts
            .iter()
            .map(|provider| provider.id.clone())
            .collect::<Vec<_>>(),
    )
    .await;
    let mut providers = Vec::with_capacity(provider_accounts.len());

    for provider_account in provider_accounts {
        let snapshot = concurrency_snapshots.get(&provider_account.id);
        let provider_quota = if let Some(provider_credential_ids) =
            credential_ids_by_account.get(&provider_account.id)
        {
            let mut snapshots = Vec::new();
            for provider_credential_id in provider_credential_ids {
                if let Some(snapshot) = provider_quota::read_cached_runtime_quota_snapshot(
                    redis_pool,
                    &provider_account.id,
                    Some(provider_credential_id.as_str()),
                )
                .await?
                {
                    snapshots.push(snapshot);
                }
            }
            provider_quota::aggregate_provider_quota_snapshots(&provider_account.id, &snapshots)
        } else {
            provider_quota::read_cached_provider_quota_snapshot(redis_pool, &provider_account.id)
                .await?
        };
        let balance_status = provider_quota
            .as_ref()
            .map(provider_quota::quota_to_balance_status);
        let provider_health = build_provider_health(
            &provider_account,
            snapshot,
            breaker_map
                .get(&provider_account.id)
                .copied()
                .unwrap_or(false),
            balance_status.as_ref(),
        );
        let usage_aggregate = usage_by_provider
            .get(&provider_account.id)
            .cloned()
            .unwrap_or_default();
        let supported_models = provider_supported_models
            .get(&provider_account.id)
            .cloned()
            .unwrap_or_default();
        let cost_hints =
            build_provider_cost_hints(&provider_account, &usage_aggregate, &supported_models);
        providers.push(GatewayProviderInventoryEntryView {
            provider_account,
            provider_health,
            cost_hints,
            provider_quota,
        });
    }

    providers.sort_by(|left, right| {
        right
            .provider_health
            .routing_score
            .partial_cmp(&left.provider_health.routing_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.provider_account
                    .label
                    .cmp(&right.provider_account.label)
            })
    });

    Ok(GatewayProviderInventoryView {
        summary: build_inventory_summary(&providers, catalog_metadata),
        providers,
    })
}

pub(super) fn build_inventory_summary(
    providers: &[GatewayProviderInventoryEntryView],
    catalog_metadata: GatewayCatalogMetadataView,
) -> GatewayProviderInventorySummaryView {
    let mut provider_families: BTreeMap<String, Vec<&GatewayProviderInventoryEntryView>> =
        BTreeMap::new();
    let mut by_source_kind = BTreeMap::new();
    let mut by_protocol_family = BTreeMap::new();
    let mut by_adapter = BTreeMap::new();
    let mut configured_source_profiles = 0usize;
    let mut derived_source_profiles = 0usize;

    for entry in providers {
        provider_families
            .entry(provider_service_identity_key(&entry.provider_account))
            .or_default()
            .push(entry);
        let source_profile = source_profile_from_account(&entry.provider_account);
        *by_source_kind
            .entry(source_profile.source_kind.clone())
            .or_insert(0usize) += 1;
        *by_protocol_family
            .entry(entry.provider_account.protocol_family.clone())
            .or_insert(0usize) += 1;
        *by_adapter
            .entry(entry.provider_account.adapter.clone())
            .or_insert(0usize) += 1;
        if source_profile.derived {
            derived_source_profiles += 1;
        } else {
            configured_source_profiles += 1;
        }
    }

    GatewayProviderInventorySummaryView {
        total_providers: provider_families.len(),
        total_provider_surfaces: providers.len(),
        active_providers: provider_families
            .values()
            .filter(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.provider_health.status == "active")
            })
            .count(),
        active_provider_surfaces: providers
            .iter()
            .filter(|entry| entry.provider_health.status == "active")
            .count(),
        degraded_providers: provider_families
            .values()
            .filter(|entries| entries.iter().any(|entry| entry.provider_health.degraded))
            .count(),
        degraded_provider_surfaces: providers
            .iter()
            .filter(|entry| entry.provider_health.degraded)
            .count(),
        breaker_open_providers: provider_families
            .values()
            .filter(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.provider_health.breaker_open)
            })
            .count(),
        breaker_open_provider_surfaces: providers
            .iter()
            .filter(|entry| entry.provider_health.breaker_open)
            .count(),
        total_active_concurrency: providers
            .iter()
            .map(|entry| entry.provider_health.active_concurrency)
            .sum(),
        avg_routing_score: (!providers.is_empty()).then(|| {
            round_score(
                providers
                    .iter()
                    .map(|entry| entry.provider_health.routing_score)
                    .sum::<f64>()
                    / providers.len() as f64,
            )
        }),
        configured_source_profiles,
        derived_source_profiles,
        providers_with_observed_cost: provider_families
            .values()
            .filter(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.cost_hints.observed_cost_micros.is_some())
            })
            .count(),
        providers_with_platform_quote: provider_families
            .values()
            .filter(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.cost_hints.platform_quote_rate.configured)
            })
            .count(),
        providers_with_quota: provider_families
            .values()
            .filter(|entries| entries.iter().any(|entry| entry.provider_quota.is_some()))
            .count(),
        warning_quota_providers: provider_families
            .values()
            .filter(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .provider_quota
                        .as_ref()
                        .map(|quota| quota.status.as_str() == "warning")
                        .unwrap_or(false)
                })
            })
            .count(),
        exhausted_quota_providers: provider_families
            .values()
            .filter(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .provider_quota
                        .as_ref()
                        .map(|quota| quota.status.as_str() == "exhausted")
                        .unwrap_or(false)
                })
            })
            .count(),
        by_source_kind: summary_buckets(by_source_kind),
        by_protocol_family: summary_buckets(by_protocol_family),
        by_adapter: summary_buckets(by_adapter),
        catalog_metadata,
    }
}

fn provider_service_identity_key(provider_account: &GatewayProviderAccountView) -> String {
    let trimmed = provider_account.service_provider_key.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    provider_account.id.clone()
}
