//! Management provider quota aggregation, probes and cooling sweeps.

use super::super::internal_gateway::assert_management_access;
use super::redaction::{mask_provider_account_view, mask_provider_inventory_view};
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_quota;
use crate::provider_runtime;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

pub async fn get_provider_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    Ok(Json(serde_json::json!({
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account.id.as_str(),
            false,
        )
        .await?,
    })))
}

pub async fn refresh_provider_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_account = db::get_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    Ok(Json(serde_json::json!({
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account.id.as_str(),
            true,
        )
        .await?,
    })))
}

pub async fn probe_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let outcome =
        provider_runtime::probe_provider_account_for_management(&state, provider_account_id.trim())
            .await?;

    Ok(Json(serde_json::json!({
        "result": {
            "ok": outcome.ok,
            "providerAccount": mask_provider_account_view(outcome.provider_account),
            "errorMessage": outcome.error_message,
            "providerQuota": outcome.provider_quota,
        },
    })))
}

pub async fn sweep_cooling_provider_accounts(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let outcomes = provider_runtime::sweep_cooling_provider_accounts(&state, 50).await?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let inventory = db::get_provider_inventory(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
    )
    .await?;
    Ok(Json(serde_json::json!({
      "sweep": {
          "scanned": outcomes.len(),
          "reactivated": outcomes.iter().filter(|outcome| outcome.ok).count(),
          "extendedCooling": outcomes.iter().filter(|outcome| !outcome.ok).count(),
          "results": outcomes.into_iter().map(|outcome| serde_json::json!({
              "ok": outcome.ok,
              "providerAccount": mask_provider_account_view(outcome.provider_account),
              "errorMessage": outcome.error_message,
              "providerQuota": outcome.provider_quota,
          })).collect::<Vec<_>>(),
      },
        "providerInventory": mask_provider_inventory_view(inventory),
    })))
}

pub(super) async fn load_provider_account_quota(
    state: &AppState,
    provider_account_id: &str,
    force_refresh: bool,
) -> Result<Option<provider_quota::GatewayProviderQuotaView>, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let provider_credentials = db::list_active_provider_credentials_for_accounts(
        pg_pool,
        &[provider_account_id.to_string()],
    )
    .await?;
    if provider_credentials.is_empty() {
        let payload = crate::routing::candidate::deserialize_provider_payload(
            &provider_account.payload,
            Some(&provider_account.adapter),
        )
        .map_err(|error| {
            GatewayError::server_error(format!(
                "deserialize provider payload for quota {}: {error}",
                provider_account.id
            ))
        })?;
        return if force_refresh {
            provider_quota::refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                &payload,
            )
            .await
        } else {
            Ok(provider_quota::get_or_refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                &payload,
            )
            .await)
        };
    }

    let mut snapshots = Vec::new();
    for provider_credential in provider_credentials {
        let merged_payload = db::merge_provider_account_and_credential_payloads(
            &provider_account.payload,
            &provider_credential.payload,
        );
        let payload = crate::routing::candidate::deserialize_provider_payload(
            &merged_payload,
            Some(&provider_account.adapter),
        )
        .map_err(|error| {
            GatewayError::server_error(format!(
                "deserialize provider payload for provider credential quota {}: {error}",
                provider_credential.id
            ))
        })?;
        let snapshot = if force_refresh {
            provider_quota::refresh_runtime_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                Some(provider_credential.id.as_str()),
                &payload,
            )
            .await?
        } else {
            provider_quota::get_or_refresh_runtime_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account.id.as_str(),
                Some(provider_credential.id.as_str()),
                &payload,
            )
            .await
        };
        if let Some(snapshot) = snapshot {
            snapshots.push(snapshot);
        }
    }
    Ok(provider_quota::aggregate_provider_quota_snapshots(
        provider_account.id.as_str(),
        &snapshots,
    ))
}
