//! Inventory cost hints and per-model pricing coverage.

use super::pricing::{
    build_gateway_price_rate, estimate_observed_cost_micros, read_model_static_pricing_map,
};
use super::pricing_editor::resolve_provider_model_market_rate;
use super::*;

pub(super) fn build_provider_cost_hints(
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

pub(super) fn build_provider_static_pricing_coverage(
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
            &CostProviderRef::from_account(provider_account),
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
