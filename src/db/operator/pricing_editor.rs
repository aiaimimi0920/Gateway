//! Model pricing precedence and editor row projections.

use super::pricing::{
    build_gateway_price_rate, estimate_observed_cost_micros, read_model_static_pricing_map,
};
use super::*;

pub(crate) fn cash_model_rate(payload: &Value, model: &str) -> GatewayPriceRateView {
    // Market-display defaults are estimates, not an operator-approved customer tariff.
    read_model_static_pricing_map(payload)
        .remove(model)
        .unwrap_or_else(|| build_gateway_price_rate(payload, PriceMode::Static))
}

pub(super) fn build_provider_pricing_editor_rows(
    provider_account: &CostProviderRef<'_>,
    supported_models: &[String],
    usage_lookup: &HashMap<String, GatewayUsageAggregate>,
) -> Vec<GatewayProviderPricingEditorModelRowView> {
    let model_pricing = provider_account
        .pricing_payload
        .map(read_model_static_pricing_map)
        .unwrap_or_default();
    let fallback_static_rate = provider_account
        .pricing_payload
        .map(|payload| build_gateway_price_rate(payload, PriceMode::Static))
        .unwrap_or_else(unconfigured_price_rate);
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

pub(super) fn resolve_provider_model_market_rate(
    provider_account: &CostProviderRef<'_>,
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
        provider_account.protocol_family,
        provider_account.adapter,
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

    unconfigured_price_rate()
}

/// The rate to report when nothing prices a model.
fn unconfigured_price_rate() -> GatewayPriceRateView {
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
