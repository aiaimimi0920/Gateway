use std::collections::{BTreeMap, HashMap};

use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::balance::BalanceStatus;
use crate::concurrency::aimd::ConcurrencySnapshot;
use crate::error::GatewayError;
use crate::provider_quota::{self, GatewayProviderQuotaView};
use crate::redis::keys;
use crate::routing::score::build_routing_score;

use super::provider_accounts::{
    get_provider_account, list_provider_accounts, GatewayProviderAccountView,
};
use super::provider_credentials::list_active_provider_credential_refs_for_accounts;
use super::{format_timestamp, map_db_error, GatewayRoutePolicyConfig};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySourceProfileView {
    pub source_kind: String,
    pub aggregator_api_mode: Option<String>,
    pub web_reverse_access_mode: Option<String>,
    pub source_notes: Option<String>,
    pub derived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderHealthView {
    pub provider_account_id: String,
    pub label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub status: String,
    pub cooldown_until: Option<String>,
    pub failure_count: i32,
    pub last_error: Option<String>,
    pub last_health_check_at: Option<String>,
    pub active_concurrency: usize,
    pub breaker_open: bool,
    pub routing_score: f64,
    pub health_weight: f64,
    pub capacity_weight: f64,
    pub degraded: bool,
    pub saturated: bool,
    pub degradation_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPriceRateView {
    pub prompt_micros_per_1k_tokens: Option<i64>,
    pub completion_micros_per_1k_tokens: Option<i64>,
    pub currency: String,
    pub configured: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderModelStaticPricingEntryView {
    pub model: String,
    pub static_rate: GatewayPriceRateView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderStaticPricingCoverageView {
    pub total_models: usize,
    pub configured_models: usize,
    pub fully_configured: bool,
    pub configured_entries: Vec<GatewayProviderModelStaticPricingEntryView>,
    pub missing_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderCostHintsView {
    pub static_rate: GatewayPriceRateView,
    pub platform_quote_rate: GatewayPriceRateView,
    pub static_pricing_coverage: GatewayProviderStaticPricingCoverageView,
    pub observed_request_count: usize,
    pub observed_failure_count: usize,
    pub recent_request_count_10m: usize,
    pub recent_failure_count_10m: usize,
    pub observed_prompt_tokens: i64,
    pub observed_completion_tokens: i64,
    pub observed_total_tokens: i64,
    pub observed_cost_micros: Option<i64>,
    pub observed_cost_source: String,
    pub last_request_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCatalogMetadataView {
    pub provider_account_count: usize,
    pub model_alias_count: usize,
    pub route_policy_count: usize,
    pub fetched_provider_accounts: usize,
    pub fetched_model_aliases: usize,
    pub fetched_route_policies: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderInventoryEntryView {
    pub provider_account: GatewayProviderAccountView,
    pub provider_health: GatewayProviderHealthView,
    pub cost_hints: GatewayProviderCostHintsView,
    pub provider_quota: Option<GatewayProviderQuotaView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySummaryBucketView {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderInventorySummaryView {
    pub total_providers: usize,
    pub total_provider_surfaces: usize,
    pub active_providers: usize,
    pub active_provider_surfaces: usize,
    pub degraded_providers: usize,
    pub degraded_provider_surfaces: usize,
    pub breaker_open_providers: usize,
    pub breaker_open_provider_surfaces: usize,
    pub total_active_concurrency: usize,
    pub avg_routing_score: Option<f64>,
    pub configured_source_profiles: usize,
    pub derived_source_profiles: usize,
    pub providers_with_observed_cost: usize,
    pub providers_with_platform_quote: usize,
    pub providers_with_quota: usize,
    pub warning_quota_providers: usize,
    pub exhausted_quota_providers: usize,
    pub by_source_kind: Vec<GatewaySummaryBucketView>,
    pub by_protocol_family: Vec<GatewaySummaryBucketView>,
    pub by_adapter: Vec<GatewaySummaryBucketView>,
    pub catalog_metadata: GatewayCatalogMetadataView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderInventoryView {
    pub providers: Vec<GatewayProviderInventoryEntryView>,
    pub summary: GatewayProviderInventorySummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationProviderLinkView {
    pub provider_account_id: String,
    pub label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub status: String,
    pub source_profile: GatewaySourceProfileView,
    pub upstream_model: Option<String>,
    pub priority: i32,
    pub weight: i32,
    pub enabled: bool,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationAliasRowView {
    pub alias: String,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub upstream_model: Option<String>,
    pub provider_count: usize,
    pub enabled_provider_count: usize,
    pub fallback_priority: String,
    pub source_kind_distribution: Vec<GatewaySummaryBucketView>,
    pub providers: Vec<GatewayModelAssociationProviderLinkView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationProviderAliasLinkView {
    pub alias: String,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub upstream_model: Option<String>,
    pub priority: i32,
    pub weight: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationProviderRowView {
    pub provider_account_id: String,
    pub label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub status: String,
    pub source_profile: GatewaySourceProfileView,
    pub default_model: Option<String>,
    pub supported_alias_count: usize,
    pub aliases: Vec<GatewayModelAssociationProviderAliasLinkView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationMatrixSummaryView {
    pub total_aliases: usize,
    pub total_providers: usize,
    pub total_links: usize,
    pub by_source_kind: Vec<GatewaySummaryBucketView>,
    pub by_protocol_family: Vec<GatewaySummaryBucketView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAssociationMatrixView {
    pub alias_rows: Vec<GatewayModelAssociationAliasRowView>,
    pub provider_rows: Vec<GatewayModelAssociationProviderRowView>,
    pub summary: GatewayModelAssociationMatrixSummaryView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostProviderModelRowView {
    pub model: String,
    pub request_count: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub thinking_tokens: i64,
    pub cached_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub market_rate: GatewayPriceRateView,
    pub estimated_market_cost_micros: Option<i64>,
    pub last_request_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostProviderBucketView {
    pub provider_account_id: String,
    pub label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub request_count: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub thinking_tokens: i64,
    pub cached_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_market_cost_micros: Option<i64>,
    pub priced_model_count: usize,
    pub unpriced_model_count: usize,
    pub last_request_at: Option<String>,
    pub models: Vec<GatewayCostProviderModelRowView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostModelProviderRowView {
    pub provider_account_id: String,
    pub label: String,
    pub request_count: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub thinking_tokens: i64,
    pub cached_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub market_rate: GatewayPriceRateView,
    pub estimated_market_cost_micros: Option<i64>,
    pub last_request_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostModelBucketView {
    pub model: String,
    pub request_count: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub thinking_tokens: i64,
    pub cached_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_market_cost_micros: Option<i64>,
    pub provider_count: usize,
    pub priced_provider_count: usize,
    pub last_request_at: Option<String>,
    pub providers: Vec<GatewayCostModelProviderRowView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderPricingEditorModelRowView {
    pub model: String,
    pub market_rate: GatewayPriceRateView,
    pub request_count: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub thinking_tokens: i64,
    pub cached_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_market_cost_micros: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderPricingEditorView {
    pub provider_account_id: String,
    pub label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub model_count: usize,
    pub configured_model_count: usize,
    pub rows: Vec<GatewayProviderPricingEditorModelRowView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostOverviewSummaryView {
    pub provider_count: usize,
    pub priced_provider_count: usize,
    pub unpriced_provider_count: usize,
    pub model_count: usize,
    pub priced_model_count: usize,
    pub unpriced_model_count: usize,
    pub total_requests: usize,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_thinking_tokens: i64,
    pub total_cached_tokens: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_market_cost_micros: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayCostOverviewView {
    pub summary: GatewayCostOverviewSummaryView,
    pub provider_buckets: Vec<GatewayCostProviderBucketView>,
    pub model_buckets: Vec<GatewayCostModelBucketView>,
    pub pricing_editors: Vec<GatewayProviderPricingEditorView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProjectPressureView {
    pub project_id: String,
    pub display_name: String,
    pub active_concurrency: usize,
    pub running_request_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderPressureView {
    pub provider_account_id: String,
    pub label: String,
    pub status: String,
    pub protocol_family: String,
    pub active_concurrency: usize,
    pub running_request_count: usize,
    pub breaker_open: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayRuntimePressureFilters {
    pub project_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRuntimePressureView {
    pub total_running_requests: usize,
    pub total_project_concurrency: usize,
    pub total_provider_concurrency: usize,
    pub projects: Vec<GatewayProjectPressureView>,
    pub providers: Vec<GatewayProviderPressureView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayReadinessChecksView {
    pub database: bool,
    pub redis: bool,
    pub object_storage: bool,
    pub api_key_secret: bool,
    pub public_base_url: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GatewayReadinessProviderStatsView {
    pub active_providers: usize,
    pub cooling_providers: usize,
    pub disabled_providers: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayReadinessReport {
    pub ok: bool,
    pub checks: GatewayReadinessChecksView,
    pub provider_stats: GatewayReadinessProviderStatsView,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderModelUsageAggregateRow {
    provider_account_id: String,
    model_name: String,
    request_count: i64,
    input_tokens: i64,
    output_tokens: i64,
    thinking_tokens: i64,
    cached_tokens: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    last_request_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderUsageAggregateRow {
    provider_account_id: String,
    request_count: i64,
    failure_count: i64,
    recent_request_count_10m: i64,
    recent_failure_count_10m: i64,
    input_tokens: i64,
    output_tokens: i64,
    thinking_tokens: i64,
    cached_tokens: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    last_request_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProjectDisplayRow {
    id: String,
    display_name: String,
}

#[derive(Debug, Clone, FromRow)]
struct RunningAuditRow {
    project_id: String,
    provider_account_id: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct CountByStatusRow {
    status: String,
    count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderSupportedModelRow {
    provider_account_id: String,
    model_name: String,
}

#[derive(Debug, Clone, Default)]
struct GatewayUsageAggregate {
    request_count: usize,
    failure_count: usize,
    recent_request_count_10m: usize,
    recent_failure_count_10m: usize,
    input_tokens: i64,
    output_tokens: i64,
    thinking_tokens: i64,
    cached_tokens: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    last_request_at: Option<String>,
}

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

pub async fn get_cost_overview(pool: &PgPool) -> Result<GatewayCostOverviewView, GatewayError> {
    let (provider_accounts, provider_model_rows, provider_supported_models) = tokio::try_join!(
        list_provider_accounts(pool),
        load_provider_model_usage_aggregates(pool),
        async {
            let provider_accounts = list_provider_accounts(pool).await?;
            let provider_accounts = filter_visible_provider_accounts(provider_accounts);
            let provider_account_ids = provider_accounts
                .iter()
                .map(|provider| provider.id.clone())
                .collect::<Vec<_>>();
            load_provider_supported_models(pool, &provider_account_ids).await
        },
    )?;
    let provider_accounts = filter_visible_provider_accounts(provider_accounts);

    let mut provider_model_aggregates =
        HashMap::<String, Vec<(String, GatewayUsageAggregate)>>::new();

    for row in provider_model_rows {
        let aggregate = GatewayUsageAggregate {
            request_count: row.request_count.max(0) as usize,
            failure_count: 0,
            recent_request_count_10m: 0,
            recent_failure_count_10m: 0,
            input_tokens: row.input_tokens.max(0),
            output_tokens: row.output_tokens.max(0),
            thinking_tokens: row.thinking_tokens.max(0),
            cached_tokens: row.cached_tokens.max(0),
            prompt_tokens: row.prompt_tokens.max(0),
            completion_tokens: row.completion_tokens.max(0),
            total_tokens: row.total_tokens.max(0),
            last_request_at: row.last_request_at.map(format_timestamp),
            ..GatewayUsageAggregate::default()
        };
        provider_model_aggregates
            .entry(row.provider_account_id)
            .or_default()
            .push((row.model_name, aggregate));
    }

    let mut provider_buckets = Vec::new();
    let mut model_grouped = HashMap::<String, Vec<GatewayCostModelProviderRowView>>::new();
    let mut pricing_editors = Vec::new();
    let mut total_requests = 0usize;
    let mut total_input_tokens = 0i64;
    let mut total_output_tokens = 0i64;
    let mut total_thinking_tokens = 0i64;
    let mut total_cached_tokens = 0i64;
    let mut total_prompt_tokens = 0i64;
    let mut total_completion_tokens = 0i64;
    let mut total_tokens = 0i64;
    let mut total_estimated_market_cost_micros = 0i64;
    let mut has_any_priced_cost = false;
    let mut priced_provider_count = 0usize;
    let mut model_count = 0usize;
    let mut priced_model_count = 0usize;

    for provider in &provider_accounts {
        let supported_models = provider_supported_models
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();
        let usage_by_model = provider_model_aggregates
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();

        let mut usage_lookup = HashMap::<String, GatewayUsageAggregate>::new();
        for (model, aggregate) in usage_by_model {
            usage_lookup.insert(model, aggregate);
        }

        let editor_rows =
            build_provider_pricing_editor_rows(provider, &supported_models, &usage_lookup);
        let configured_model_count = editor_rows
            .iter()
            .filter(|row| row.market_rate.configured)
            .count();
        let model_rows_with_traffic = editor_rows
            .iter()
            .filter(|row| row.request_count > 0 || row.total_tokens > 0)
            .map(|row| GatewayCostProviderModelRowView {
                model: row.model.clone(),
                request_count: row.request_count,
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                thinking_tokens: row.thinking_tokens,
                cached_tokens: row.cached_tokens,
                prompt_tokens: row.prompt_tokens,
                completion_tokens: row.completion_tokens,
                total_tokens: row.total_tokens,
                market_rate: row.market_rate.clone(),
                estimated_market_cost_micros: row.estimated_market_cost_micros,
                last_request_at: usage_lookup
                    .get(&row.model)
                    .and_then(|aggregate| aggregate.last_request_at.clone()),
            })
            .collect::<Vec<_>>();

        let provider_request_count = model_rows_with_traffic
            .iter()
            .map(|row| row.request_count)
            .sum::<usize>();
        let provider_input_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.input_tokens)
            .sum::<i64>();
        let provider_output_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.output_tokens)
            .sum::<i64>();
        let provider_thinking_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.thinking_tokens)
            .sum::<i64>();
        let provider_cached_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.cached_tokens)
            .sum::<i64>();
        let provider_prompt_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.prompt_tokens)
            .sum::<i64>();
        let provider_completion_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.completion_tokens)
            .sum::<i64>();
        let provider_total_tokens = model_rows_with_traffic
            .iter()
            .map(|row| row.total_tokens)
            .sum::<i64>();
        let provider_estimated_market_cost_micros = model_rows_with_traffic
            .iter()
            .map(|row| row.estimated_market_cost_micros.unwrap_or(0))
            .sum::<i64>();
        let provider_has_priced_cost = model_rows_with_traffic
            .iter()
            .any(|row| row.estimated_market_cost_micros.is_some());
        let provider_last_request_at = model_rows_with_traffic
            .iter()
            .filter_map(|row| row.last_request_at.clone())
            .max();

        total_requests += provider_request_count;
        total_input_tokens += provider_input_tokens;
        total_output_tokens += provider_output_tokens;
        total_thinking_tokens += provider_thinking_tokens;
        total_cached_tokens += provider_cached_tokens;
        total_prompt_tokens += provider_prompt_tokens;
        total_completion_tokens += provider_completion_tokens;
        total_tokens += provider_total_tokens;
        if provider_has_priced_cost {
            total_estimated_market_cost_micros += provider_estimated_market_cost_micros;
            has_any_priced_cost = true;
            priced_provider_count += 1;
        }

        for row in &model_rows_with_traffic {
            model_grouped.entry(row.model.clone()).or_default().push(
                GatewayCostModelProviderRowView {
                    provider_account_id: provider.id.clone(),
                    label: provider.label.clone(),
                    request_count: row.request_count,
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    thinking_tokens: row.thinking_tokens,
                    cached_tokens: row.cached_tokens,
                    prompt_tokens: row.prompt_tokens,
                    completion_tokens: row.completion_tokens,
                    total_tokens: row.total_tokens,
                    market_rate: row.market_rate.clone(),
                    estimated_market_cost_micros: row.estimated_market_cost_micros,
                    last_request_at: row.last_request_at.clone(),
                },
            );
        }

        model_count += editor_rows.len();
        priced_model_count += configured_model_count;

        pricing_editors.push(GatewayProviderPricingEditorView {
            provider_account_id: provider.id.clone(),
            label: provider.label.clone(),
            adapter: provider.adapter.clone(),
            protocol_family: provider.protocol_family.clone(),
            model_count: editor_rows.len(),
            configured_model_count,
            rows: editor_rows.clone(),
        });

        provider_buckets.push(GatewayCostProviderBucketView {
            provider_account_id: provider.id.clone(),
            label: provider.label.clone(),
            adapter: provider.adapter.clone(),
            protocol_family: provider.protocol_family.clone(),
            request_count: provider_request_count,
            input_tokens: provider_input_tokens,
            output_tokens: provider_output_tokens,
            thinking_tokens: provider_thinking_tokens,
            cached_tokens: provider_cached_tokens,
            prompt_tokens: provider_prompt_tokens,
            completion_tokens: provider_completion_tokens,
            total_tokens: provider_total_tokens,
            estimated_market_cost_micros: provider_has_priced_cost
                .then_some(provider_estimated_market_cost_micros),
            priced_model_count: configured_model_count,
            unpriced_model_count: editor_rows.len().saturating_sub(configured_model_count),
            last_request_at: provider_last_request_at,
            models: model_rows_with_traffic,
        });
    }

    provider_buckets.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| left.label.cmp(&right.label))
    });

    let mut model_buckets = model_grouped
        .into_iter()
        .map(|(model, mut providers)| {
            providers.sort_by(|left, right| {
                right
                    .total_tokens
                    .cmp(&left.total_tokens)
                    .then_with(|| left.label.cmp(&right.label))
            });
            let request_count = providers.iter().map(|row| row.request_count).sum::<usize>();
            let input_tokens = providers.iter().map(|row| row.input_tokens).sum::<i64>();
            let output_tokens = providers.iter().map(|row| row.output_tokens).sum::<i64>();
            let thinking_tokens = providers.iter().map(|row| row.thinking_tokens).sum::<i64>();
            let cached_tokens = providers.iter().map(|row| row.cached_tokens).sum::<i64>();
            let prompt_tokens = providers.iter().map(|row| row.prompt_tokens).sum::<i64>();
            let completion_tokens = providers
                .iter()
                .map(|row| row.completion_tokens)
                .sum::<i64>();
            let total_tokens = providers.iter().map(|row| row.total_tokens).sum::<i64>();
            let estimated_market_cost_micros = providers
                .iter()
                .map(|row| row.estimated_market_cost_micros.unwrap_or(0))
                .sum::<i64>();
            let priced_provider_count = providers
                .iter()
                .filter(|row| row.market_rate.configured)
                .count();
            let has_priced_cost = providers
                .iter()
                .any(|row| row.estimated_market_cost_micros.is_some());
            let last_request_at = providers
                .iter()
                .filter_map(|row| row.last_request_at.clone())
                .max();
            GatewayCostModelBucketView {
                model,
                request_count,
                input_tokens,
                output_tokens,
                thinking_tokens,
                cached_tokens,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                estimated_market_cost_micros: has_priced_cost
                    .then_some(estimated_market_cost_micros),
                provider_count: providers.len(),
                priced_provider_count,
                last_request_at,
                providers,
            }
        })
        .collect::<Vec<_>>();
    model_buckets.sort_by(|left, right| {
        right
            .total_tokens
            .cmp(&left.total_tokens)
            .then_with(|| left.model.cmp(&right.model))
    });

    pricing_editors.sort_by(|left, right| left.label.cmp(&right.label));

    let provider_count = provider_accounts.len();
    let unpriced_provider_count = provider_count.saturating_sub(priced_provider_count);
    let unpriced_model_count = model_count.saturating_sub(priced_model_count);

    Ok(GatewayCostOverviewView {
        summary: GatewayCostOverviewSummaryView {
            provider_count,
            priced_provider_count,
            unpriced_provider_count,
            model_count,
            priced_model_count,
            unpriced_model_count,
            total_requests,
            total_input_tokens,
            total_output_tokens,
            total_thinking_tokens,
            total_cached_tokens,
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens,
            estimated_market_cost_micros: has_any_priced_cost
                .then_some(total_estimated_market_cost_micros),
        },
        provider_buckets,
        model_buckets,
        pricing_editors,
    })
}

pub async fn get_runtime_pressure(
    pool: &PgPool,
    redis_pool: &Pool,
    concurrency_snapshots: &HashMap<String, ConcurrencySnapshot>,
    filters: &GatewayRuntimePressureFilters,
) -> Result<GatewayRuntimePressureView, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500) as i64;
    let running_rows = sqlx::query_as::<_, RunningAuditRow>(
        r#"
        select project_id, provider_account_id
        from gateway_request_audits
        where status = 'running'
          and ($1::text is null or project_id = $1)
          and ($2::text is null or provider_account_id = $2)
        order by created_at desc
        limit $3
        "#,
    )
    .bind(filters.project_id.as_deref())
    .bind(filters.provider_account_id.as_deref())
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut project_ids = running_rows
        .iter()
        .map(|row| row.project_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(project_id) = filters.project_id.as_ref() {
        project_ids.insert(project_id.clone());
    }
    let mut provider_ids = running_rows
        .iter()
        .filter_map(|row| row.provider_account_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(provider_account_id) = filters.provider_account_id.as_ref() {
        provider_ids.insert(provider_account_id.clone());
    }

    let project_rows = if project_ids.is_empty() {
        Vec::new()
    } else {
        let ids = project_ids.into_iter().collect::<Vec<_>>();
        let placeholders = (1..=ids.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("select id, display_name from gateway_projects where id in ({placeholders})");
        let mut query = sqlx::query_as::<_, GatewayProjectDisplayRow>(&sql);
        for id in &ids {
            query = query.bind(id);
        }
        query.fetch_all(pool).await.map_err(map_db_error)?
    };
    let provider_rows = if provider_ids.is_empty() {
        Vec::new()
    } else {
        let ids = provider_ids;
        list_provider_accounts(pool)
            .await?
            .into_iter()
            .filter(|provider| ids.contains(&provider.id))
            .collect::<Vec<_>>()
    };

    let mut project_counts = HashMap::<String, usize>::new();
    let mut provider_counts = HashMap::<String, usize>::new();
    for row in &running_rows {
        *project_counts
            .entry(row.project_id.clone())
            .or_insert(0usize) += 1;
        if let Some(provider_account_id) = row.provider_account_id.as_ref() {
            *provider_counts
                .entry(provider_account_id.clone())
                .or_insert(0usize) += 1;
        }
    }

    let mut projects = Vec::with_capacity(project_rows.len());
    for project in project_rows {
        let running_request_count = project_counts.get(&project.id).copied().unwrap_or(0);
        let active_concurrency =
            read_redis_int(redis_pool, &legacy_project_concurrency_key(&project.id))
                .await
                .unwrap_or(running_request_count);
        projects.push(GatewayProjectPressureView {
            project_id: project.id.clone(),
            display_name: project.display_name,
            active_concurrency,
            running_request_count,
        });
    }
    projects.sort_by(|left, right| {
        right
            .active_concurrency
            .cmp(&left.active_concurrency)
            .then_with(|| right.running_request_count.cmp(&left.running_request_count))
    });

    let breaker_map = read_breaker_open_map(
        redis_pool,
        &provider_rows
            .iter()
            .map(|provider| provider.id.clone())
            .collect::<Vec<_>>(),
    )
    .await;
    let mut providers = Vec::with_capacity(provider_rows.len());
    for provider in provider_rows {
        let running_request_count = provider_counts.get(&provider.id).copied().unwrap_or(0);
        let legacy_active_concurrency =
            read_redis_int(redis_pool, &legacy_provider_concurrency_key(&provider.id)).await;
        let active_concurrency = concurrency_snapshots
            .get(&provider.id)
            .map(|snapshot| snapshot.active_count)
            .or(legacy_active_concurrency)
            .unwrap_or(running_request_count);
        providers.push(GatewayProviderPressureView {
            provider_account_id: provider.id.clone(),
            label: provider.label.clone(),
            status: provider.status.clone(),
            protocol_family: provider.protocol_family.clone(),
            active_concurrency,
            running_request_count,
            breaker_open: breaker_map.get(&provider.id).copied().unwrap_or(false),
        });
    }
    providers.sort_by(|left, right| {
        right
            .active_concurrency
            .cmp(&left.active_concurrency)
            .then_with(|| right.running_request_count.cmp(&left.running_request_count))
    });

    Ok(GatewayRuntimePressureView {
        total_running_requests: running_rows.len(),
        total_project_concurrency: projects.iter().map(|row| row.active_concurrency).sum(),
        total_provider_concurrency: providers.iter().map(|row| row.active_concurrency).sum(),
        projects,
        providers,
    })
}

pub async fn get_readiness_provider_stats(
    pool: &PgPool,
) -> Result<GatewayReadinessProviderStatsView, GatewayError> {
    let rows = sqlx::query_as::<_, CountByStatusRow>(
        r#"
        select status, count(*)::bigint as count
        from gateway_provider_accounts
        group by status
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut stats = GatewayReadinessProviderStatsView::default();
    for row in rows {
        match row.status.as_str() {
            "active" => stats.active_providers = row.count.max(0) as usize,
            "cooling" => stats.cooling_providers = row.count.max(0) as usize,
            "disabled" => stats.disabled_providers = row.count.max(0) as usize,
            _ => {}
        }
    }
    Ok(stats)
}

pub async fn mark_provider_probe_success(
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'active',
          cooldown_until = null,
          last_error = null,
          failure_count = 0,
          last_health_check_at = $2,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn mark_provider_probe_failure(
    pool: &PgPool,
    provider_account_id: &str,
    message: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let cooldown_until = now + time::Duration::seconds(30);
    let error_message = truncate_error_summary(message, 1000);
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = case when status = 'archived' then status else 'cooling' end,
          cooldown_until = case when status = 'archived' then cooldown_until else $2 end,
          last_error = $3,
          failure_count = failure_count + 1,
          last_health_check_at = $4,
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(cooldown_until)
    .bind(error_message)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn mark_provider_cooling_retry_failure(
    pool: &PgPool,
    provider_account_id: &str,
    message: &str,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let cooldown_until = now + time::Duration::seconds(30);
    let error_message = truncate_error_summary(message, 1000);
    let result = sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'cooling',
          cooldown_until = $2,
          last_error = $3,
          last_health_check_at = $4,
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(cooldown_until)
    .bind(error_message)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("Provider account 不存在"));
    }

    get_provider_account(pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}

pub async fn list_expired_cooling_provider_account_ids(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<String>, GatewayError> {
    sqlx::query_scalar::<_, String>(
        r#"
        select id
        from gateway_provider_accounts
        where status = 'cooling'
          and cooldown_until is not null
          and cooldown_until <= $1
        order by cooldown_until asc
        limit $2
        "#,
    )
    .bind(OffsetDateTime::now_utc())
    .bind(limit.max(1).min(100))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

pub async fn clear_provider_runtime_keys(
    redis_pool: &Pool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: usize = conn
        .del(&[
            keys::provider_failure_count_key(provider_account_id),
            keys::provider_breaker_open_key(provider_account_id),
            keys::provider_quota_snapshot_key(provider_account_id),
            keys::provider_quota_lock_key(provider_account_id),
        ])
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("clear provider runtime keys: {error}"))
        })?;
    Ok(())
}

pub async fn note_provider_runtime_failure(
    redis_pool: &Pool,
    pool: &PgPool,
    provider_account_id: &str,
    route_policy: &GatewayRoutePolicyConfig,
    message: &str,
) -> Result<(), GatewayError> {
    let ttl_seconds = route_policy.circuit_breaker_cooldown_seconds.max(30) as u64;
    let threshold = route_policy.circuit_breaker_threshold.max(1) as u64;
    let failure_count_key = keys::provider_failure_count_key(provider_account_id);
    let breaker_open_key = keys::provider_breaker_open_key(provider_account_id);
    let failure_count = {
        let mut conn = redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
        let failure_count: u64 = conn.incr(&failure_count_key, 1).await.map_err(|error| {
            GatewayError::server_error(format!("increment provider failures: {error}"))
        })?;
        let _: bool = conn
            .expire(&failure_count_key, ttl_seconds as i64)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("expire provider failures: {error}"))
            })?;
        if failure_count >= threshold {
            let _: () = redis::cmd("SET")
                .arg(&breaker_open_key)
                .arg("1")
                .arg("EX")
                .arg(ttl_seconds)
                .query_async(&mut conn)
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("open provider breaker: {error}"))
                })?;
        }
        failure_count
    };

    let now = OffsetDateTime::now_utc();
    let truncated = truncate_error_summary(message, 1000);
    if failure_count >= threshold {
        sqlx::query(
            r#"
            update gateway_provider_accounts
            set
              status = 'cooling',
              cooldown_until = $2,
              last_error = $3,
              failure_count = $4,
              updated_at = $5
            where id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(now + time::Duration::seconds(ttl_seconds as i64))
        .bind(truncated)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    } else {
        sqlx::query(
            r#"
            update gateway_provider_accounts
            set
              last_error = $2,
              failure_count = $3,
              updated_at = $4
            where id = $1
            "#,
        )
        .bind(provider_account_id)
        .bind(truncated)
        .bind((failure_count.min(i32::MAX as u64)) as i32)
        .bind(now)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
    }

    Ok(())
}

pub async fn note_provider_runtime_success(
    redis_pool: &Pool,
    pool: &PgPool,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    clear_provider_runtime_keys(redis_pool, provider_account_id).await?;
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_provider_accounts
        set
          status = 'active',
          cooldown_until = null,
          last_error = null,
          failure_count = 0,
          last_health_check_at = $2,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(provider_account_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

fn build_provider_health(
    provider_account: &GatewayProviderAccountView,
    snapshot: Option<&ConcurrencySnapshot>,
    breaker_open: bool,
    balance_status: Option<&BalanceStatus>,
) -> GatewayProviderHealthView {
    let (effective_status, effective_cooldown_until, effective_failure_count) =
        normalize_operator_provider_runtime_state(provider_account);
    let (active_concurrency, provider_limit) = snapshot
        .map(|snapshot| (snapshot.active_count, Some(snapshot.current_limit)))
        .unwrap_or((0, None));
    let routing_score = build_routing_score(
        effective_status.as_str(),
        effective_failure_count.max(0) as u32,
        breaker_open,
        active_concurrency,
        provider_limit,
        balance_status,
    );

    GatewayProviderHealthView {
        provider_account_id: provider_account.id.clone(),
        label: provider_account.label.clone(),
        adapter: provider_account.adapter.clone(),
        protocol_family: provider_account.protocol_family.clone(),
        status: effective_status,
        cooldown_until: effective_cooldown_until,
        failure_count: effective_failure_count,
        last_error: provider_account.last_error.clone(),
        last_health_check_at: provider_account.last_health_check_at.clone(),
        active_concurrency,
        breaker_open,
        routing_score: round_score(routing_score.score),
        health_weight: round_score(routing_score.health_weight),
        capacity_weight: round_score(routing_score.capacity_weight),
        degraded: routing_score.degraded,
        saturated: routing_score.capacity_weight <= 0.0,
        degradation_reasons: routing_score.degradation_reasons,
    }
}

fn normalize_operator_provider_runtime_state(
    provider_account: &GatewayProviderAccountView,
) -> (String, Option<String>, i32) {
    let cooldown_active = provider_account
        .cooldown_until
        .as_deref()
        .and_then(parse_rfc3339)
        .is_some_and(|until| until > OffsetDateTime::now_utc());

    if provider_account.status == "cooling" && !cooldown_active {
        return ("active".to_string(), None, 0);
    }

    (
        provider_account.status.clone(),
        provider_account.cooldown_until.clone(),
        provider_account.failure_count,
    )
}

fn parse_rfc3339(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

fn build_provider_cost_hints(
    provider_account: &GatewayProviderAccountView,
    aggregate: &GatewayUsageAggregate,
    supported_models: &[String],
) -> GatewayProviderCostHintsView {
    let payload = &provider_account.payload;
    let static_rate = build_gateway_price_rate(payload, PriceMode::Static);
    let platform_quote_rate = build_gateway_price_rate(payload, PriceMode::Quote);
    let observed_cost_micros = estimate_observed_cost_micros(aggregate, &static_rate);
    let static_pricing_coverage =
        build_provider_static_pricing_coverage(provider_account, supported_models, &static_rate);

    GatewayProviderCostHintsView {
        static_rate,
        platform_quote_rate: platform_quote_rate.clone(),
        static_pricing_coverage,
        observed_request_count: aggregate.request_count,
        observed_failure_count: aggregate.failure_count,
        recent_request_count_10m: aggregate.recent_request_count_10m,
        recent_failure_count_10m: aggregate.recent_failure_count_10m,
        observed_prompt_tokens: aggregate.prompt_tokens,
        observed_completion_tokens: aggregate.completion_tokens,
        observed_total_tokens: aggregate.total_tokens,
        observed_cost_micros,
        observed_cost_source: if observed_cost_micros.is_some() {
            "configured_rate_estimate".to_string()
        } else {
            "unavailable".to_string()
        },
        last_request_at: aggregate.last_request_at.clone(),
    }
}

fn build_provider_static_pricing_coverage(
    provider_account: &GatewayProviderAccountView,
    supported_models: &[String],
    fallback_static_rate: &GatewayPriceRateView,
) -> GatewayProviderStaticPricingCoverageView {
    let model_pricing = read_model_static_pricing_map(&provider_account.payload);
    let mut configured_entries = Vec::new();
    let mut missing_models = Vec::new();
    let mut deduped_models = supported_models
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    deduped_models.sort();
    deduped_models.dedup();

    for model in deduped_models.iter() {
        let model_rate = resolve_provider_model_market_rate(
            provider_account,
            model,
            &model_pricing,
            fallback_static_rate,
        );

        if model_rate.configured {
            configured_entries.push(GatewayProviderModelStaticPricingEntryView {
                model: model.clone(),
                static_rate: model_rate,
            });
            continue;
        }

        missing_models.push(model.clone());
    }

    GatewayProviderStaticPricingCoverageView {
        total_models: deduped_models.len(),
        configured_models: configured_entries.len(),
        fully_configured: !deduped_models.is_empty() && missing_models.is_empty(),
        configured_entries,
        missing_models,
    }
}

fn build_inventory_summary(
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

fn source_profile_from_account(
    provider_account: &GatewayProviderAccountView,
) -> GatewaySourceProfileView {
    GatewaySourceProfileView {
        source_kind: provider_account
            .source_kind
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        aggregator_api_mode: provider_account.aggregator_api_mode.clone(),
        web_reverse_access_mode: provider_account.web_reverse_access_mode.clone(),
        source_notes: provider_account.source_notes.clone(),
        derived: provider_account.source_kind.is_none(),
    }
}

fn filter_visible_provider_accounts(
    provider_accounts: Vec<GatewayProviderAccountView>,
) -> Vec<GatewayProviderAccountView> {
    provider_accounts
        .into_iter()
        .filter(|provider| !provider_is_hidden_from_operator_inventory(provider))
        .collect()
}

fn provider_is_hidden_from_operator_inventory(
    provider_account: &GatewayProviderAccountView,
) -> bool {
    let Some(payload) = provider_account.payload.as_object() else {
        return false;
    };

    let hidden_flag = [
        "hiddenFromOperatorInventory",
        "hiddenFromInventory",
        "internalOnly",
    ]
    .iter()
    .any(|key| payload.get(*key).and_then(Value::as_bool).unwrap_or(false));
    if hidden_flag {
        return true;
    }

    if provider_account
        .source_notes
        .as_deref()
        .map(str::to_ascii_lowercase)
        .is_some_and(|notes| {
            notes.contains("hidden_from_operator_inventory")
                || notes.contains("internal_fixture")
                || notes.contains("local_fixture")
        })
    {
        return true;
    }

    read_provider_default_model(&provider_account.payload)
        .is_some_and(|model| model == "xml-fallback-fixture")
}

async fn load_provider_usage_aggregates(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, GatewayUsageAggregate>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ProviderUsageAggregateRow>(
        r#"
        select
          provider_account_id,
          count(*)::bigint as request_count,
          count(*) filter (where status = 'failed')::bigint as failure_count,
          count(*) filter (where created_at >= now() - interval '10 minutes')::bigint as recent_request_count_10m,
          count(*) filter (
            where status = 'failed'
              and created_at >= now() - interval '10 minutes'
          )::bigint as recent_failure_count_10m,
          coalesce(sum(greatest(coalesce(prompt_tokens, 0), 0)), 0)::bigint as input_tokens,
          coalesce(sum(greatest(coalesce(completion_tokens, 0), 0)), 0)::bigint as output_tokens,
          0::bigint as thinking_tokens,
          coalesce(
            sum(
              greatest(coalesce(cache_creation_input_tokens, 0), 0) +
              greatest(coalesce(cache_read_input_tokens, 0), 0)
            ),
            0
          )::bigint as cached_tokens,
          coalesce(sum(greatest(coalesce(prompt_tokens, 0), 0)), 0)::bigint as prompt_tokens,
          coalesce(sum(greatest(coalesce(completion_tokens, 0), 0)), 0)::bigint as completion_tokens,
          coalesce(
            sum(
              greatest(
                coalesce(total_tokens, coalesce(prompt_tokens, 0) + coalesce(completion_tokens, 0)),
                0
              )
            ),
            0
          )::bigint as total_tokens,
          max(created_at) as last_request_at
        from gateway_request_audits
        where provider_account_id = any($1)
        group by provider_account_id
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.provider_account_id,
                GatewayUsageAggregate {
                    request_count: row.request_count.max(0) as usize,
                    failure_count: row.failure_count.max(0) as usize,
                    recent_request_count_10m: row.recent_request_count_10m.max(0) as usize,
                    recent_failure_count_10m: row.recent_failure_count_10m.max(0) as usize,
                    input_tokens: row.input_tokens.max(0),
                    output_tokens: row.output_tokens.max(0),
                    thinking_tokens: row.thinking_tokens.max(0),
                    cached_tokens: row.cached_tokens.max(0),
                    prompt_tokens: row.prompt_tokens.max(0),
                    completion_tokens: row.completion_tokens.max(0),
                    total_tokens: row.total_tokens.max(0),
                    last_request_at: row.last_request_at.map(format_timestamp),
                },
            )
        })
        .collect())
}

async fn load_catalog_metadata(pool: &PgPool) -> Result<GatewayCatalogMetadataView, GatewayError> {
    let provider_account_count = count_rows(pool, "gateway_provider_accounts").await?;
    let model_alias_count = count_rows(pool, "gateway_model_aliases").await?;
    let route_policy_count = count_rows(pool, "gateway_route_policies").await?;

    Ok(GatewayCatalogMetadataView {
        provider_account_count,
        model_alias_count,
        route_policy_count,
        fetched_provider_accounts: provider_account_count,
        fetched_model_aliases: model_alias_count,
        fetched_route_policies: route_policy_count,
    })
}

async fn load_provider_supported_models(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, Vec<String>>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, ProviderSupportedModelRow>(
        r#"
        select
          provider_account_id,
          coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), '')) as model_name
        from gateway_provider_capability_catalog
        where enabled = true
          and provider_account_id = any($1)
        group by provider_account_id, coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), ''))
        order by provider_account_id, coalesce(nullif(trim(upstream_model), ''), nullif(trim(model_code), ''))
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut supported_models = HashMap::<String, Vec<String>>::new();
    for row in rows {
        if row.model_name.trim().is_empty() {
            continue;
        }
        supported_models
            .entry(row.provider_account_id)
            .or_default()
            .push(row.model_name);
    }
    Ok(supported_models)
}

async fn load_provider_model_usage_aggregates(
    pool: &PgPool,
) -> Result<Vec<ProviderModelUsageAggregateRow>, GatewayError> {
    sqlx::query_as::<_, ProviderModelUsageAggregateRow>(
        r#"
        select
          source.provider_account_id,
          source.model_name,
          count(*)::bigint as request_count,
          coalesce(sum(greatest(coalesce(source.prompt_tokens, 0), 0)), 0)::bigint as input_tokens,
          coalesce(sum(greatest(coalesce(source.completion_tokens, 0), 0)), 0)::bigint as output_tokens,
          0::bigint as thinking_tokens,
          coalesce(
            sum(
              greatest(coalesce(source.cache_creation_input_tokens, 0), 0) +
              greatest(coalesce(source.cache_read_input_tokens, 0), 0)
            ),
            0
          )::bigint as cached_tokens,
          coalesce(sum(greatest(coalesce(source.prompt_tokens, 0), 0)), 0)::bigint as prompt_tokens,
          coalesce(sum(greatest(coalesce(source.completion_tokens, 0), 0)), 0)::bigint as completion_tokens,
          coalesce(
            sum(
              greatest(
                coalesce(source.total_tokens, coalesce(source.prompt_tokens, 0) + coalesce(source.completion_tokens, 0)),
                0
              )
            ),
            0
          )::bigint as total_tokens,
          max(source.created_at) as last_request_at
        from (
          select
            provider_account_id,
            coalesce(
              nullif(trim(resolved_model), ''),
              nullif(trim(requested_model), ''),
              nullif(trim(model_alias), ''),
              'unknown'
            ) as model_name,
            prompt_tokens,
            completion_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            total_tokens,
            created_at
          from gateway_request_audits
          where provider_account_id is not null
        ) as source
        group by source.provider_account_id, source.model_name
        order by source.provider_account_id, source.model_name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

async fn count_rows(pool: &PgPool, table_name: &str) -> Result<usize, GatewayError> {
    let sql = format!("select count(*)::bigint as count from {table_name}");
    let count = sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .map_err(map_db_error)?;
    Ok(count.max(0) as usize)
}

#[derive(Clone, Copy)]
enum PriceMode {
    Static,
    Quote,
}

fn build_gateway_price_rate(payload: &Value, mode: PriceMode) -> GatewayPriceRateView {
    let keys = match mode {
        PriceMode::Static => [
            "staticInputMicrosPer1kTokens",
            "pricingInputMicrosPer1kTokens",
            "staticOutputMicrosPer1kTokens",
            "pricingOutputMicrosPer1kTokens",
        ],
        PriceMode::Quote => [
            "platformQuoteInputMicrosPer1kTokens",
            "quoteInputMicrosPer1kTokens",
            "platformQuoteOutputMicrosPer1kTokens",
            "quoteOutputMicrosPer1kTokens",
        ],
    };

    let prompt_micros_per_1k_tokens = read_gateway_pricing_field(payload, &keys[..2]);
    let completion_micros_per_1k_tokens = read_gateway_pricing_field(payload, &keys[2..]);
    let configured =
        prompt_micros_per_1k_tokens.is_some() || completion_micros_per_1k_tokens.is_some();

    GatewayPriceRateView {
        prompt_micros_per_1k_tokens,
        completion_micros_per_1k_tokens,
        currency: "USD".to_string(),
        configured,
        source: if configured {
            "payload".to_string()
        } else {
            "unconfigured".to_string()
        },
    }
}

fn read_gateway_pricing_field(payload: &Value, keys: &[&str]) -> Option<i64> {
    let payload_record = payload.as_object();
    let extra_body = payload_record
        .and_then(|record| record.get("extraBody").or_else(|| record.get("extra_body")))
        .and_then(Value::as_object);

    for key in keys {
        if let Some(value) = payload_record.and_then(|record| record.get(*key)) {
            if let Some(normalized) = normalize_gateway_price_value(value) {
                return Some(normalized);
            }
        }
        if let Some(value) = extra_body.and_then(|record| record.get(*key)) {
            if let Some(normalized) = normalize_gateway_price_value(value) {
                return Some(normalized);
            }
        }
    }
    None
}

fn read_model_static_pricing_map(payload: &Value) -> HashMap<String, GatewayPriceRateView> {
    let payload_record = payload.as_object();
    let extra_body = payload_record
        .and_then(|record| record.get("extraBody").or_else(|| record.get("extra_body")))
        .and_then(Value::as_object);

    let pricing_map = payload_record
        .and_then(|record| {
            record
                .get("modelPricing")
                .or_else(|| record.get("model_pricing"))
                .or_else(|| record.get("pricingByModel"))
                .or_else(|| record.get("pricing_by_model"))
        })
        .or_else(|| {
            extra_body.and_then(|record| {
                record
                    .get("modelPricing")
                    .or_else(|| record.get("model_pricing"))
                    .or_else(|| record.get("pricingByModel"))
                    .or_else(|| record.get("pricing_by_model"))
            })
        })
        .and_then(Value::as_object);

    let mut result = HashMap::new();
    let Some(pricing_map) = pricing_map else {
        return result;
    };

    for (model, entry) in pricing_map {
        let model = model.trim();
        if model.is_empty() {
            continue;
        }
        let Some(record) = entry.as_object() else {
            continue;
        };
        let prompt = read_pricing_field_from_record(
            record,
            &[
                "staticInputMicrosPer1kTokens",
                "pricingInputMicrosPer1kTokens",
            ],
        );
        let completion = read_pricing_field_from_record(
            record,
            &[
                "staticOutputMicrosPer1kTokens",
                "pricingOutputMicrosPer1kTokens",
            ],
        );
        let configured = prompt.is_some() || completion.is_some();
        result.insert(
            model.to_string(),
            GatewayPriceRateView {
                prompt_micros_per_1k_tokens: prompt,
                completion_micros_per_1k_tokens: completion,
                currency: "USD".to_string(),
                configured,
                source: if configured {
                    "model_pricing".to_string()
                } else {
                    "unconfigured".to_string()
                },
            },
        );
    }

    result
}

fn build_provider_pricing_editor_rows(
    provider_account: &GatewayProviderAccountView,
    supported_models: &[String],
    usage_lookup: &HashMap<String, GatewayUsageAggregate>,
) -> Vec<GatewayProviderPricingEditorModelRowView> {
    let model_pricing = read_model_static_pricing_map(&provider_account.payload);
    let fallback_static_rate =
        build_gateway_price_rate(&provider_account.payload, PriceMode::Static);
    let mut models = supported_models
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    for model in usage_lookup.keys() {
        if !model.trim().is_empty() {
            models.push(model.trim().to_string());
        }
    }

    models.sort();
    models.dedup();

    models
        .into_iter()
        .map(|model| {
            let aggregate = usage_lookup
                .get(&model)
                .cloned()
                .unwrap_or_else(GatewayUsageAggregate::default);
            let market_rate = resolve_provider_model_market_rate(
                provider_account,
                &model,
                &model_pricing,
                &fallback_static_rate,
            );
            let estimated_market_cost_micros =
                estimate_observed_cost_micros(&aggregate, &market_rate);
            GatewayProviderPricingEditorModelRowView {
                model,
                market_rate,
                request_count: aggregate.request_count,
                input_tokens: aggregate.input_tokens,
                output_tokens: aggregate.output_tokens,
                thinking_tokens: aggregate.thinking_tokens,
                cached_tokens: aggregate.cached_tokens,
                prompt_tokens: aggregate.prompt_tokens,
                completion_tokens: aggregate.completion_tokens,
                total_tokens: aggregate.total_tokens,
                estimated_market_cost_micros,
            }
        })
        .collect()
}

fn resolve_provider_model_market_rate(
    provider_account: &GatewayProviderAccountView,
    model: &str,
    configured_model_pricing: &HashMap<String, GatewayPriceRateView>,
    fallback_static_rate: &GatewayPriceRateView,
) -> GatewayPriceRateView {
    if let Some(rate) = configured_model_pricing.get(model) {
        if rate.configured {
            return rate.clone();
        }
    }

    if let Some((prompt, completion)) = default_market_price_for_model(
        provider_account.protocol_family.as_str(),
        provider_account.adapter.as_str(),
        model,
    ) {
        return GatewayPriceRateView {
            prompt_micros_per_1k_tokens: Some(prompt),
            completion_micros_per_1k_tokens: Some(completion),
            currency: "USD".to_string(),
            configured: true,
            source: "default_registry".to_string(),
        };
    }

    if fallback_static_rate.configured {
        return fallback_static_rate.clone();
    }

    GatewayPriceRateView {
        prompt_micros_per_1k_tokens: None,
        completion_micros_per_1k_tokens: None,
        currency: "USD".to_string(),
        configured: false,
        source: "unconfigured".to_string(),
    }
}

fn default_market_price_for_model(
    protocol_family: &str,
    adapter: &str,
    model: &str,
) -> Option<(i64, i64)> {
    let normalized_model = model.trim().to_lowercase();
    let normalized_protocol = protocol_family.trim().to_lowercase();
    let normalized_adapter = adapter.trim().to_lowercase();

    if normalized_protocol == "openai"
        || normalized_protocol == "codex"
        || matches!(
            normalized_adapter.as_str(),
            "openai_compatible" | "codex_cli" | "custom_http"
        )
    {
        return match normalized_model.as_str() {
            "gpt-5.4" | "gpt-5" => Some((2_500, 15_000)),
            "gpt-5.4-mini" | "gpt-5-mini" => Some((750, 4_500)),
            "gpt-5.3-codex" | "gpt-5-codex" => Some((1_750, 14_000)),
            "gpt-5.2" | "gpt-5-classic" => Some((1_750, 14_000)),
            _ => None,
        };
    }

    None
}

fn read_pricing_field_from_record(
    record: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<i64> {
    for key in keys {
        if let Some(value) = record.get(*key) {
            if let Some(normalized) = normalize_gateway_price_value(value) {
                return Some(normalized);
            }
        }
    }
    None
}

fn normalize_gateway_price_value(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number.max(0));
    }
    if let Some(number) = value.as_u64() {
        return Some(number as i64);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value.max(0.0).round() as i64)
}

fn estimate_observed_cost_micros(
    aggregate: &GatewayUsageAggregate,
    rate: &GatewayPriceRateView,
) -> Option<i64> {
    if !rate.configured {
        return None;
    }

    let prompt_cost = rate
        .prompt_micros_per_1k_tokens
        .map(|rate| ((aggregate.prompt_tokens as f64 * rate as f64) / 1000.0).round() as i64)
        .unwrap_or(0);
    let completion_cost = rate
        .completion_micros_per_1k_tokens
        .map(|rate| ((aggregate.completion_tokens as f64 * rate as f64) / 1000.0).round() as i64)
        .unwrap_or(0);

    Some(prompt_cost + completion_cost)
}

fn read_provider_default_model(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|record| record.get("defaultModel"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn build_fallback_priority_label(links: &[GatewayModelAssociationProviderLinkView]) -> String {
    if links.is_empty() {
        return "未绑定 provider".to_string();
    }
    links
        .iter()
        .map(|link| format!("{}(P{})", link.label, link.priority))
        .collect::<Vec<_>>()
        .join(" -> ")
}

async fn read_redis_int(redis_pool: &Pool, key: &str) -> Option<usize> {
    let mut conn = redis_pool.get().await.ok()?;
    let raw = conn.get::<_, Option<String>>(key).await.ok().flatten()?;
    raw.trim()
        .parse::<i64>()
        .ok()
        .map(|value| value.max(0) as usize)
}

fn legacy_provider_concurrency_key(provider_account_id: &str) -> String {
    format!("ai-gateway:provider:{provider_account_id}:concurrency")
}

fn legacy_project_concurrency_key(project_id: &str) -> String {
    format!("ai-gateway:project:{project_id}:concurrency")
}

fn summary_buckets(buckets: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketView> {
    buckets
        .into_iter()
        .map(|(value, count)| GatewaySummaryBucketView { value, count })
        .collect()
}

fn round_score(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn truncate_error_summary(message: &str, max_chars: usize) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "probe failed".to_string();
    }
    match trimmed.char_indices().nth(max_chars) {
        Some((index, _)) => trimmed[..index].to_string(),
        None => trimmed.to_string(),
    }
}

async fn read_breaker_open_map(
    redis_pool: &Pool,
    provider_ids: &[String],
) -> HashMap<String, bool> {
    let mut result = HashMap::new();
    let Ok(mut conn) = redis_pool.get().await else {
        return result;
    };

    for provider_id in provider_ids {
        let breaker_open = conn
            .get::<_, Option<String>>(keys::provider_breaker_open_key(provider_id))
            .await
            .ok()
            .flatten()
            .is_some();
        result.insert(provider_id.clone(), breaker_open);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pricing_fields_support_direct_and_extra_body_values() {
        let payload = json!({
            "staticInputMicrosPer1kTokens": 300,
            "extraBody": {
                "platformQuoteOutputMicrosPer1kTokens": "420"
            }
        });

        let static_rate = build_gateway_price_rate(&payload, PriceMode::Static);
        let quote_rate = build_gateway_price_rate(&payload, PriceMode::Quote);

        assert_eq!(static_rate.prompt_micros_per_1k_tokens, Some(300));
        assert_eq!(static_rate.completion_micros_per_1k_tokens, None);
        assert_eq!(quote_rate.completion_micros_per_1k_tokens, Some(420));
        assert!(quote_rate.configured);
    }

    #[test]
    fn observed_cost_uses_prompt_and_completion_rates() {
        let aggregate = GatewayUsageAggregate {
            request_count: 1,
            failure_count: 0,
            recent_request_count_10m: 0,
            recent_failure_count_10m: 0,
            input_tokens: 2500,
            output_tokens: 1000,
            thinking_tokens: 0,
            cached_tokens: 0,
            prompt_tokens: 2500,
            completion_tokens: 1000,
            total_tokens: 3500,
            last_request_at: None,
        };
        let rate = GatewayPriceRateView {
            prompt_micros_per_1k_tokens: Some(1000),
            completion_micros_per_1k_tokens: Some(2000),
            currency: "USD".to_string(),
            configured: true,
            source: "payload".to_string(),
        };

        assert_eq!(estimate_observed_cost_micros(&aggregate, &rate), Some(4500));
    }

    #[test]
    fn model_static_pricing_coverage_uses_model_pricing_map() {
        let provider = GatewayProviderAccountView {
            id: "prov-openai".to_string(),
            label: "OpenAI".to_string(),
            service_provider_key: "openai".to_string(),
            service_provider_label: "OpenAI".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            status: "active".to_string(),
            source_kind: Some("official_vendor_api".to_string()),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({
            "modelPricing": {
                "gpt-5.4": {
                    "staticInputMicrosPer1kTokens": 2500,
                    "staticOutputMicrosPer1kTokens": 15000
                },
                "gpt-5.4-mini": {
                    "staticInputMicrosPer1kTokens": 750,
                    "staticOutputMicrosPer1kTokens": 4500
                }
            }
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            updated_at: "2026-04-13T00:00:00Z".to_string(),
        };

        let fallback_rate = build_gateway_price_rate(&provider.payload, PriceMode::Static);
        let coverage = build_provider_static_pricing_coverage(
            &provider,
            &[
                "gpt-5.4".to_string(),
                "gpt-5.4-mini".to_string(),
                "gpt-5.3-codex".to_string(),
            ],
            &fallback_rate,
        );

        assert_eq!(coverage.total_models, 3);
        assert_eq!(coverage.configured_models, 3);
        assert!(coverage.fully_configured);
        assert!(coverage.missing_models.is_empty());
    }

    #[test]
    fn default_market_price_registry_covers_openai_family_models() {
        let provider = GatewayProviderAccountView {
            id: "codex-platform-provider".to_string(),
            label: "Codex Platform".to_string(),
            service_provider_key: "codex_platform".to_string(),
            service_provider_label: "Codex Platform".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "codex".to_string(),
            status: "active".to_string(),
            source_kind: Some("official_vendor_api".to_string()),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({}),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            updated_at: "2026-04-13T00:00:00Z".to_string(),
        };

        let rate = resolve_provider_model_market_rate(
            &provider,
            "gpt-5.4-mini",
            &HashMap::new(),
            &GatewayPriceRateView {
                prompt_micros_per_1k_tokens: None,
                completion_micros_per_1k_tokens: None,
                currency: "USD".to_string(),
                configured: false,
                source: "unconfigured".to_string(),
            },
        );

        assert_eq!(rate.prompt_micros_per_1k_tokens, Some(750));
        assert_eq!(rate.completion_micros_per_1k_tokens, Some(4500));
        assert_eq!(rate.source, "default_registry");
    }

    #[test]
    fn source_profile_is_marked_derived_when_source_kind_missing() {
        let provider = GatewayProviderAccountView {
            id: "prov-1".to_string(),
            label: "Provider".to_string(),
            service_provider_key: "provider".to_string(),
            service_provider_label: "Provider".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            status: "active".to_string(),
            source_kind: None,
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({}),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-13T00:00:00Z".to_string(),
            updated_at: "2026-04-13T00:00:00Z".to_string(),
        };

        let source_profile = source_profile_from_account(&provider);
        assert_eq!(source_profile.source_kind, "unknown");
        assert!(source_profile.derived);
    }

    #[test]
    fn fallback_priority_label_uses_provider_priority_order() {
        let label = build_fallback_priority_label(&[
            GatewayModelAssociationProviderLinkView {
                provider_account_id: "prov-1".to_string(),
                label: "Alpha".to_string(),
                adapter: "openai_compatible".to_string(),
                protocol_family: "openai".to_string(),
                status: "active".to_string(),
                source_profile: GatewaySourceProfileView {
                    source_kind: "aggregator_api".to_string(),
                    aggregator_api_mode: None,
                    web_reverse_access_mode: None,
                    source_notes: None,
                    derived: false,
                },
                upstream_model: Some("gpt-4.1".to_string()),
                priority: 0,
                weight: 1,
                enabled: true,
                default_model: Some("gpt-4.1".to_string()),
            },
            GatewayModelAssociationProviderLinkView {
                provider_account_id: "prov-2".to_string(),
                label: "Beta".to_string(),
                adapter: "anthropic_compatible".to_string(),
                protocol_family: "anthropic".to_string(),
                status: "active".to_string(),
                source_profile: GatewaySourceProfileView {
                    source_kind: "official_vendor_api".to_string(),
                    aggregator_api_mode: None,
                    web_reverse_access_mode: None,
                    source_notes: None,
                    derived: false,
                },
                upstream_model: Some("claude-sonnet-4".to_string()),
                priority: 1,
                weight: 1,
                enabled: true,
                default_model: Some("claude-sonnet-4".to_string()),
            },
        ]);

        assert_eq!(label, "Alpha(P0) -> Beta(P1)");
    }

    #[test]
    fn operator_inventory_hides_provider_when_payload_flag_enabled() {
        let provider = GatewayProviderAccountView {
            id: "fixture-provider".to_string(),
            label: "Local XML Fallback Fixture".to_string(),
            service_provider_key: "local_xml_fallback_fixture".to_string(),
            service_provider_label: "Local XML Fallback Fixture".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai_compatible_generic".to_string(),
            status: "active".to_string(),
            source_kind: Some("aggregator_api".to_string()),
            aggregator_api_mode: Some("hosted_compute".to_string()),
            web_reverse_access_mode: None,
            source_notes: None,
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({
                "baseUrl": "http://host.docker.internal:42327",
                "defaultModel": "xml-fallback-fixture",
                "hiddenFromOperatorInventory": true
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-18T00:00:00Z".to_string(),
            updated_at: "2026-04-18T00:00:00Z".to_string(),
        };

        assert!(provider_is_hidden_from_operator_inventory(&provider));
    }

    #[test]
    fn operator_inventory_hides_xml_fixture_default_model() {
        let provider = GatewayProviderAccountView {
            id: "fixture-provider".to_string(),
            label: "Fixture Provider".to_string(),
            service_provider_key: "fixture_provider".to_string(),
            service_provider_label: "Fixture Provider".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai_compatible_generic".to_string(),
            status: "active".to_string(),
            source_kind: Some("aggregator_api".to_string()),
            aggregator_api_mode: Some("hosted_compute".to_string()),
            web_reverse_access_mode: None,
            source_notes: Some("internal_fixture:hidden_from_operator_inventory".to_string()),
            execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({
                "baseUrl": "http://host.docker.internal:42327",
                "defaultModel": "xml-fallback-fixture"
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-04-18T00:00:00Z".to_string(),
            updated_at: "2026-04-18T00:00:00Z".to_string(),
        };

        assert!(provider_is_hidden_from_operator_inventory(&provider));
    }

    #[test]
    fn inventory_summary_counts_distinct_service_providers_separately_from_surfaces() {
        fn make_entry(
            account_id: &str,
            service_provider_key: &str,
            protocol_family: &str,
            status: &str,
            degraded: bool,
            breaker_open: bool,
        ) -> GatewayProviderInventoryEntryView {
            GatewayProviderInventoryEntryView {
                provider_account: GatewayProviderAccountView {
                    id: account_id.to_string(),
                    label: format!("{service_provider_key} {protocol_family}"),
                    service_provider_key: service_provider_key.to_string(),
                    service_provider_label: service_provider_key.to_string(),
                    adapter: format!("{protocol_family}_compatible"),
                    protocol_family: protocol_family.to_string(),
                    protocol_profile: protocol_family.to_string(),
                    status: status.to_string(),
                    source_kind: Some("official_vendor_api".to_string()),
                    aggregator_api_mode: None,
                    web_reverse_access_mode: None,
                    source_notes: None,
                    execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
                    endpoint_execution_modes: None,
                    payload: json!({}),
                    storage_mode: "inline".to_string(),
                    cooldown_until: None,
                    last_error: None,
                    failure_count: 0,
                    last_health_check_at: None,
                    created_at: "2026-04-19T00:00:00Z".to_string(),
                    updated_at: "2026-04-19T00:00:00Z".to_string(),
                },
                provider_health: GatewayProviderHealthView {
                    provider_account_id: account_id.to_string(),
                    label: account_id.to_string(),
                    adapter: format!("{protocol_family}_compatible"),
                    protocol_family: protocol_family.to_string(),
                    status: status.to_string(),
                    cooldown_until: None,
                    failure_count: 0,
                    last_error: None,
                    last_health_check_at: None,
                    active_concurrency: 0,
                    breaker_open,
                    routing_score: 1.0,
                    health_weight: 1.0,
                    capacity_weight: 1.0,
                    degraded,
                    saturated: false,
                    degradation_reasons: Vec::new(),
                },
                cost_hints: GatewayProviderCostHintsView {
                    static_rate: GatewayPriceRateView {
                        prompt_micros_per_1k_tokens: None,
                        completion_micros_per_1k_tokens: None,
                        currency: "USD".to_string(),
                        configured: false,
                        source: "unconfigured".to_string(),
                    },
                    platform_quote_rate: GatewayPriceRateView {
                        prompt_micros_per_1k_tokens: None,
                        completion_micros_per_1k_tokens: None,
                        currency: "USD".to_string(),
                        configured: false,
                        source: "unconfigured".to_string(),
                    },
                    static_pricing_coverage: GatewayProviderStaticPricingCoverageView {
                        total_models: 0,
                        configured_models: 0,
                        fully_configured: false,
                        configured_entries: Vec::new(),
                        missing_models: Vec::new(),
                    },
                    observed_request_count: 0,
                    observed_failure_count: 0,
                    recent_request_count_10m: 0,
                    recent_failure_count_10m: 0,
                    observed_prompt_tokens: 0,
                    observed_completion_tokens: 0,
                    observed_total_tokens: 0,
                    observed_cost_micros: None,
                    observed_cost_source: "unavailable".to_string(),
                    last_request_at: None,
                },
                provider_quota: None,
            }
        }

        let summary = build_inventory_summary(
            &[
                make_entry(
                    "xfyun-openai",
                    "xfyun_platform",
                    "openai",
                    "active",
                    false,
                    false,
                ),
                make_entry(
                    "xfyun-anthropic",
                    "xfyun_platform",
                    "anthropic",
                    "active",
                    true,
                    false,
                ),
                make_entry(
                    "codex-openai",
                    "codex_platform",
                    "openai",
                    "active",
                    false,
                    true,
                ),
            ],
            GatewayCatalogMetadataView {
                provider_account_count: 3,
                model_alias_count: 0,
                route_policy_count: 0,
                fetched_provider_accounts: 3,
                fetched_model_aliases: 0,
                fetched_route_policies: 0,
            },
        );

        assert_eq!(summary.total_providers, 2);
        assert_eq!(summary.total_provider_surfaces, 3);
        assert_eq!(summary.active_providers, 2);
        assert_eq!(summary.active_provider_surfaces, 3);
        assert_eq!(summary.degraded_providers, 1);
        assert_eq!(summary.degraded_provider_surfaces, 1);
        assert_eq!(summary.breaker_open_providers, 1);
        assert_eq!(summary.breaker_open_provider_surfaces, 1);
    }
}
