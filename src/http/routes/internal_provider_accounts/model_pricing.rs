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
    pub entries: Vec<ProviderModelPricingEntryBody>,
}

pub async fn patch_provider_model_pricing(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderModelPricingPatchBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let existing = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    let mut input = upsert_input_from_existing(&existing);
    input.payload = merge_provider_model_pricing(existing.payload.clone(), body.entries)?;

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

        let prompt = entry.prompt_micros_per_1k_tokens.map(|value| value.max(0));
        let completion = entry
            .completion_micros_per_1k_tokens
            .map(|value| value.max(0));

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
