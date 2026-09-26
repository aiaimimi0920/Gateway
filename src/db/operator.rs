//! Operator reports and shared private database records.

use std::collections::{BTreeMap, HashMap, HashSet};

use deadpool_redis::Pool;
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::balance::BalanceStatus;
use crate::concurrency::aimd::ConcurrencySnapshot;
use crate::error::GatewayError;
use crate::provider_quota::{self, GatewayProviderQuotaView};
use crate::routing::score::build_routing_score;

use super::provider_accounts::{
    get_provider_account, list_provider_accounts, GatewayProviderAccountView,
};
use super::provider_credentials::list_active_provider_credential_refs_for_accounts;
use super::{format_timestamp, list_model_aliases, map_db_error, GatewayRoutePolicyConfig};

mod associations;
mod cost;
mod cost_hints;
mod health;
mod identity;
mod inventory;
mod models;
mod pressure;
mod pricing;
mod pricing_editor;
mod probe_state;
mod readiness;
mod runtime_state;
mod usage_queries;

pub use associations::get_model_association_matrix;
pub use cost::get_cost_overview;
pub use inventory::get_provider_inventory;
pub use models::{
    GatewayCatalogMetadataView, GatewayCostModelBucketView, GatewayCostModelProviderRowView,
    GatewayCostOverviewSummaryView, GatewayCostOverviewView, GatewayCostProviderBucketView,
    GatewayCostProviderModelRowView, GatewayModelAssociationAliasRowView,
    GatewayModelAssociationMatrixSummaryView, GatewayModelAssociationMatrixView,
    GatewayModelAssociationProviderAliasLinkView, GatewayModelAssociationProviderLinkView,
    GatewayModelAssociationProviderRowView, GatewayPriceRateView, GatewayProjectPressureView,
    GatewayProviderCostHintsView, GatewayProviderHealthView, GatewayProviderInventoryEntryView,
    GatewayProviderInventorySummaryView, GatewayProviderInventoryView,
    GatewayProviderModelStaticPricingEntryView, GatewayProviderPressureView,
    GatewayProviderPricingEditorModelRowView, GatewayProviderPricingEditorView,
    GatewayProviderStaticPricingCoverageView, GatewayReadinessChecksView,
    GatewayReadinessProviderStatsView, GatewayReadinessReport, GatewayRuntimePressureFilters,
    GatewayRuntimePressureView, GatewayRuntimeProviderIdentity, GatewaySourceProfileView,
    GatewaySummaryBucketView,
};
pub use pressure::get_runtime_pressure;
pub use probe_state::{
    list_expired_cooling_provider_account_ids, mark_provider_cooling_retry_failure,
    mark_provider_probe_failure, mark_provider_probe_success,
};
pub use readiness::get_readiness_provider_stats;
pub use runtime_state::{
    clear_provider_runtime_keys, note_provider_runtime_failure, note_provider_runtime_success,
};

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

/// A provider as the cost view needs it, whatever it was learned from.
///
/// Only four facts matter here: who the provider is, how to price its models,
/// and the two identifiers the default price registry keys on. Borrowing them
/// lets a route-document provider join the view without fabricating a
/// [`GatewayProviderAccountView`] — that type carries the account payload, and
/// the payload carries API keys, cookies and refresh tokens that have no reason
/// to travel through a cost report.
struct CostProviderRef<'a> {
    id: &'a str,
    label: &'a str,
    adapter: &'a str,
    protocol_family: &'a str,
    /// Operator-entered pricing, which lives only in the database payload. A
    /// provider known from the route document alone has none, so its models
    /// resolve to the default registry or to an unconfigured rate.
    pricing_payload: Option<&'a Value>,
    /// Whether an operator can edit this provider's pricing. Editing writes to
    /// `gateway_provider_accounts`, so a provider without a row there is
    /// reported in the usage buckets but left out of the pricing editors.
    pricing_editable: bool,
}

impl<'a> CostProviderRef<'a> {
    fn from_account(provider: &'a GatewayProviderAccountView) -> Self {
        Self {
            id: provider.id.as_str(),
            label: provider.label.as_str(),
            adapter: provider.adapter.as_str(),
            protocol_family: provider.protocol_family.as_str(),
            pricing_payload: Some(&provider.payload),
            pricing_editable: true,
        }
    }

    fn from_runtime(provider: &'a GatewayRuntimeProviderIdentity) -> Self {
        Self {
            id: provider.id.as_str(),
            label: provider.label.as_str(),
            adapter: provider.adapter.as_str(),
            protocol_family: provider.protocol_family.as_str(),
            pricing_payload: None,
            pricing_editable: false,
        }
    }
}

#[derive(Clone, Copy)]
enum PriceMode {
    Static,
    Quote,
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

#[cfg(test)]
mod tests;
