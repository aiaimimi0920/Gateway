//! Management model-pricing partial updates and canonical rate merging.

use super::super::internal_gateway::assert_management_access;
use super::account_input::upsert_input_from_existing;
use super::redaction::mask_provider_account_view;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::redis::provider_sync;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

/// Only expose the billing projection, never the provider payload or credentials.
pub async fn get_provider_model_pricing(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<impl axum::response::IntoResponse, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let id = provider_account_id.trim();
    if state.local_runtime.is_some()
        && !state
            .route_config
            .get_providers()
            .iter()
            .any(|provider| provider.id == id)
    {
        return Err(GatewayError::not_found("Provider account does not exist"));
    }
    if state.local_runtime.is_none()
        && db::get_provider_account(required_pg_pool(&state)?, id)
            .await?
            .is_none()
    {
        return Err(GatewayError::not_found("Provider account does not exist"));
    }
    let payload = crate::cash_billing::pricing::payload(&state, id).await?;
    let rates = payload
        .get("accountBillingMultipliers")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let mut normalized = serde_json::Map::new();
    for (id, value) in rates
        .as_object()
        .ok_or_else(crate::cash_billing::amount_error)?
    {
        let text = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        let ppm = crate::cash_billing::multiplier_ppm(&text)?;
        normalized.insert(
            id.clone(),
            Value::String(format!("{}.{:06}", ppm / 1_000_000, ppm % 1_000_000)),
        );
    }
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"accountBillingMultipliers": normalized})),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPricingEntryBody {
    pub model: String,
    #[serde(default)]
    pub prompt_micros_per_1k_tokens: Option<i64>,
    #[serde(default)]
    pub completion_micros_per_1k_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPricingPatchBody {
    #[serde(default)]
    pub entries: Vec<ProviderModelPricingEntryBody>,
    pub account_billing_multipliers: Option<std::collections::BTreeMap<String, Value>>,
}

pub async fn patch_provider_model_pricing(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderModelPricingPatchBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if let Some(local) = &state.local_runtime {
        let provider = state
            .route_config
            .get_providers()
            .into_iter()
            .find(|provider| provider.id == provider_account_id.trim())
            .ok_or_else(|| GatewayError::not_found("Provider account does not exist"))?;
        let mut tx = local
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(crate::local_runtime::storage_error)?;
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM provider_pricing WHERE provider_id = ?")
                .bind(&provider.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(crate::local_runtime::storage_error)?;
        let payload = payload
            .map(|payload| serde_json::from_str(&payload))
            .transpose()
            .map_err(|_| GatewayError::server_error("Local pricing data is corrupt"))?
            .unwrap_or_else(|| serde_json::json!({}));
        let payload = merge_provider_model_pricing(payload, body.entries)?;
        let payload = crate::cash_billing::pricing::merge_account_rates(
            payload,
            body.account_billing_multipliers,
        )?;
        sqlx::query(
            "INSERT INTO provider_pricing(provider_id, payload) VALUES (?, ?)
            ON CONFLICT(provider_id) DO UPDATE SET payload = excluded.payload",
        )
        .bind(&provider.id)
        .bind(payload.to_string())
        .execute(&mut *tx)
        .await
        .map_err(crate::local_runtime::storage_error)?;
        tx.commit()
            .await
            .map_err(crate::local_runtime::storage_error)?;
        return Ok(Json(serde_json::json!({ "providerAccount": {
            "id": provider.id, "label": provider.label, "adapter": provider.payload.adapter,
            "protocolFamily": provider.protocol_family, "payload": payload,
        }})));
    }
    let existing = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let mut input = upsert_input_from_existing(&existing);
    input.payload = merge_provider_model_pricing(existing.payload.clone(), body.entries)?;
    input.payload = crate::cash_billing::pricing::merge_account_rates(
        input.payload,
        body.account_billing_multipliers,
    )?;

    let provider_account = db::update_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
        input,
    )
    .await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        provider_account_id.trim(),
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;

    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub(super) fn merge_provider_model_pricing(
    payload: Value,
    entries: Vec<ProviderModelPricingEntryBody>,
) -> Result<Value, GatewayError> {
    let mut payload_map = payload
        .as_object()
        .cloned()
        .ok_or_else(|| GatewayError::bad_request("Provider payload 必须是 JSON object"))?;

    let mut pricing_map = payload_map
        .remove("modelPricing")
        .or_else(|| payload_map.remove("model_pricing"))
        .or_else(|| payload_map.remove("pricingByModel"))
        .or_else(|| payload_map.remove("pricing_by_model"))
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    for entry in entries {
        let model = entry.model.trim();
        if model.is_empty() {
            continue;
        }

        let prompt = entry.prompt_micros_per_1k_tokens;
        let completion = entry.completion_micros_per_1k_tokens;
        if [prompt, completion]
            .into_iter()
            .flatten()
            .any(|value| !(0..=crate::cash_billing::MAX_MICROS).contains(&value))
        {
            return Err(crate::cash_billing::amount_error());
        }

        if prompt.is_none() && completion.is_none() {
            pricing_map.remove(model);
            continue;
        }

        let mut rate = serde_json::Map::new();
        if let Some(prompt) = prompt {
            rate.insert(
                "staticInputMicrosPer1kTokens".to_string(),
                Value::Number(prompt.into()),
            );
        }
        if let Some(completion) = completion {
            rate.insert(
                "staticOutputMicrosPer1kTokens".to_string(),
                Value::Number(completion.into()),
            );
        }
        pricing_map.insert(model.to_string(), Value::Object(rate));
    }

    if pricing_map.is_empty() {
        payload_map.remove("modelPricing");
    } else {
        payload_map.insert("modelPricing".to_string(), Value::Object(pricing_map));
    }

    Ok(Value::Object(payload_map))
}
