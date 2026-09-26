//! Management source-profile patch and backfill persistence.

use super::super::internal_gateway::assert_management_access;
use super::account_input::upsert_input_from_existing;
use super::redaction::mask_provider_account_view;
use super::required_pg_pool;
use super::source_profile::{
    infer_source_profile, normalize_explicit_source_profile, ProviderSourceProfileBackfillBody,
    ProviderSourceProfilePatchBody,
};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::redis::provider_sync;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

pub async fn patch_provider_source_profile(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderSourceProfilePatchBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let normalized = normalize_explicit_source_profile(body.source_profile)?;
    let existing = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let mut input = upsert_input_from_existing(&existing);
    input.source_kind = Some(normalized.source_kind);
    input.aggregator_api_mode = normalized.aggregator_api_mode;
    input.web_reverse_access_mode = normalized.web_reverse_access_mode;
    input.source_notes = normalized.notes;
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

pub async fn backfill_provider_source_profiles(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ProviderSourceProfileBackfillBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account_ids = body
        .provider_account_ids
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let only_missing = body.only_missing.unwrap_or(true);
    let provider_accounts = db::list_provider_accounts(required_pg_pool(state.as_ref())?).await?;
    let mut updated_provider_accounts = Vec::new();
    let mut skipped_count = 0usize;
    let scoped_provider_accounts = provider_accounts
        .into_iter()
        .filter(|provider| {
            provider_account_ids.is_empty()
                || provider_account_ids
                    .iter()
                    .any(|value| value == &provider.id)
        })
        .collect::<Vec<_>>();

    for existing in scoped_provider_accounts.iter() {
        if only_missing && existing.source_kind.is_some() {
            skipped_count += 1;
            continue;
        }
        let inferred = infer_source_profile(existing);
        let mut input = upsert_input_from_existing(existing);
        input.source_kind = Some(inferred.source_kind);
        input.aggregator_api_mode = inferred.aggregator_api_mode;
        input.web_reverse_access_mode = inferred.web_reverse_access_mode;
        input.source_notes = inferred.notes;
        let updated = db::update_provider_account(
            required_pg_pool(state.as_ref())?,
            existing.id.as_str(),
            input,
        )
        .await?;
        provider_sync::set_cached_provider_payload(
            &state.redis_pool,
            existing.id.as_str(),
            &updated.payload,
            None,
        )
        .await
        .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
        updated_provider_accounts.push(updated);
    }

    Ok(Json(serde_json::json!({
        "result": {
            "scannedCount": scoped_provider_accounts.len(),
            "updatedCount": updated_provider_accounts.len(),
            "skippedCount": skipped_count,
            "providerAccounts": updated_provider_accounts
                .into_iter()
                .map(mask_provider_account_view)
                .collect::<Vec<_>>(),
        }
    })))
}
