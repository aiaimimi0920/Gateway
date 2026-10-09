//! Prices and account multipliers come from the existing operator-owned pricing payload.
use super::{amount_error, multiplier_ppm, CashQuote};
use crate::{
    error::GatewayError,
    routing::{candidate::RouteCandidate, config::RouteConfigSnapshot},
    state::AppState,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn missing_price() -> GatewayError {
    GatewayError::bad_request("现金计费缺少完整的模型价格或权益配置")
        .with_code("cash_pricing_unavailable")
}

pub(crate) fn account_id(candidate: &RouteCandidate) -> String {
    candidate
        .provider_credential_id
        .as_deref()
        .or(candidate.payload.credential_id.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            crate::routing::config::provider_default_account_id(&candidate.provider_account_id)
        })
}

pub(crate) async fn payload(state: &AppState, provider: &str) -> Result<Value, GatewayError> {
    if let Some(local) = &state.local_runtime {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM provider_pricing WHERE provider_id=?")
                .bind(provider)
                .fetch_optional(&local.pool)
                .await
                .map_err(crate::local_runtime::storage_error)?;
        return payload
            .map(|payload| serde_json::from_str(&payload).map_err(|_| missing_price()))
            .transpose()
            .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})));
    }
    let pool = state.pg_pool.as_ref().ok_or_else(missing_price)?;
    Ok(crate::db::get_provider_account(pool, provider)
        .await?
        .ok_or_else(missing_price)?
        .payload)
}

pub(crate) fn quote(
    snapshot: &RouteConfigSnapshot,
    candidate: &RouteCandidate,
    model: &str,
    allowed_groups: Option<&[String]>,
    requested_group: Option<&str>,
    payload: &Value,
) -> Result<CashQuote, GatewayError> {
    validate_price_values(payload, model)?;
    let rate = crate::db::operator::cash_model_rate(payload, model);
    let prompt = rate.prompt_micros_per_1k_tokens.ok_or_else(missing_price)?;
    let completion = rate
        .completion_micros_per_1k_tokens
        .ok_or_else(missing_price)?;
    let credential = account_id(candidate);
    let (group_id, group_multiplier_ppm) =
        group_multiplier(snapshot, &credential, allowed_groups, requested_group)?;
    let account_multiplier_ppm = payload
        .get("accountBillingMultipliers")
        .and_then(|rates| rates.get(&credential))
        .map(parse_multiplier)
        .transpose()?
        .unwrap_or(1_000_000);
    Ok(CashQuote {
        provider_account_id: candidate.provider_account_id.clone(),
        credential_id: credential,
        model: model.into(),
        group_id,
        price_source: rate.source,
        prompt_micros_per_1k_tokens: prompt,
        completion_micros_per_1k_tokens: completion,
        group_multiplier_ppm,
        account_multiplier_ppm,
        cache_tokens_separate: candidate.protocol_family == "anthropic"
            || candidate.adapter.contains("anthropic"),
    })
}

fn group_multiplier(
    snapshot: &RouteConfigSnapshot,
    credential: &str,
    allowed: Option<&[String]>,
    requested: Option<&str>,
) -> Result<(Option<String>, u64), GatewayError> {
    if allowed.is_none() && requested.is_none() {
        return Ok((None, 1_000_000));
    }
    let mut choices = Vec::new();
    for group in &snapshot.document().account_groups {
        if group.enabled.unwrap_or(true)
            && allowed.is_none_or(|ids| ids.iter().any(|id| id == &group.id))
            && requested.is_none_or(|id| id == group.id)
            && group
                .provider_credential_ids
                .iter()
                .any(|id| id.trim() == credential)
        {
            choices.push((
                multiplier_ppm(&group.billing_multiplier.unwrap_or(1.0).to_string())?,
                group.id.clone(),
            ));
        }
    }
    // The user chose the lowest authorized rate. ID tie-breaking is deterministic, never HashMap order.
    choices.sort();
    choices
        .into_iter()
        .next()
        .map(|(rate, id)| (Some(id), rate))
        .ok_or_else(missing_price)
}

fn parse_multiplier(value: &Value) -> Result<u64, GatewayError> {
    match value {
        Value::String(value) => multiplier_ppm(value),
        Value::Number(value) => multiplier_ppm(&value.to_string()),
        _ => Err(amount_error()),
    }
}

pub(crate) fn merge_account_rates(
    mut payload: Value,
    values: Option<BTreeMap<String, Value>>,
) -> Result<Value, GatewayError> {
    if let Some(values) = values {
        if values.len() > 1024 {
            return Err(amount_error());
        }
        let mut rates = payload
            .get("accountBillingMultipliers")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (id, value) in values {
            if id.is_empty()
                || id.len() > 256
                || id.trim() != id
                || id.chars().any(char::is_control)
            {
                return Err(amount_error());
            }
            if value.is_null() {
                rates.remove(&id);
            } else {
                parse_multiplier(&value)?;
                rates.insert(id, value);
            }
        }
        if rates.len() > 1024 {
            return Err(amount_error());
        }
        payload
            .as_object_mut()
            .ok_or_else(amount_error)?
            .insert("accountBillingMultipliers".into(), Value::Object(rates));
    }
    Ok(payload)
}

fn validate_price_values(payload: &Value, model: &str) -> Result<(), GatewayError> {
    let extra = payload
        .get("extraBody")
        .or_else(|| payload.get("extra_body"));
    for owner in [Some(payload), extra].into_iter().flatten() {
        let model_rate = [
            "modelPricing",
            "model_pricing",
            "pricingByModel",
            "pricing_by_model",
        ]
        .iter()
        .find_map(|key| owner.get(key).and_then(Value::as_object))
        .and_then(|map| map.iter().find(|(key, _)| key.trim() == model))
        .map(|(_, rate)| rate);
        for rate in [Some(owner), model_rate].into_iter().flatten() {
            for key in [
                "staticInputMicrosPer1kTokens",
                "pricingInputMicrosPer1kTokens",
                "staticOutputMicrosPer1kTokens",
                "pricingOutputMicrosPer1kTokens",
            ] {
                if let Some(value) = rate.get(key) {
                    let value = value
                        .as_i64()
                        .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()));
                    if !value.is_some_and(|value| (0..=super::MAX_MICROS).contains(&value)) {
                        return Err(missing_price());
                    }
                }
            }
        }
    }
    Ok(())
}
