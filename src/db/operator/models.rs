//! Public operator report data contracts.

use super::*;

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
    /// The adaptive ceiling the concurrency controller currently allows, when the
    /// in-process registry has a snapshot for this provider account.
    pub concurrency_limit: Option<usize>,
    /// Head-room left under `concurrency_limit`, mirrored from the same snapshot.
    pub concurrency_available: Option<usize>,
    pub running_request_count: usize,
    pub breaker_open: bool,
}

/// A provider the runtime can serve traffic with, as the runtime knows it.
///
/// A Platform-managed deployment keeps every provider in
/// `gateway_provider_accounts`, so the pressure and cost views can look up the
/// id each request audit recorded. A standalone deployment keeps its providers
/// in the route document instead and that table stays empty, so the same lookup
/// finds nothing and the per-provider panels come back empty even while the
/// audits hold real traffic. Callers pass the route document's providers here to
/// fill that gap; a database row for the same id wins, because it is the one an
/// operator can edit.
///
/// Pricing is deliberately absent: the route document carries no prices, so a
/// provider known only from it reports token and request counts with an
/// unconfigured rate rather than inventing a cost.
#[derive(Debug, Clone)]
pub struct GatewayRuntimeProviderIdentity {
    pub id: String,
    pub label: String,
    pub status: String,
    pub adapter: String,
    pub protocol_family: String,
    /// Models the route document lets this provider serve. Empty means the
    /// provider accepts whatever model the request names, so only models with
    /// recorded traffic can be listed for it.
    pub supported_models: Vec<String>,
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
