//! Configured pricing parsing and observed-cost estimates.

use super::*;

pub(super) fn build_gateway_price_rate(payload: &Value, mode: PriceMode) -> GatewayPriceRateView {
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

pub(super) fn read_model_static_pricing_map(
    payload: &Value,
) -> HashMap<String, GatewayPriceRateView> {
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

pub(super) fn estimate_observed_cost_micros(
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
