//! Management credential quota admission and database-backed quota loading.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use super::route_config::load_route_config_provider_credential_quota;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_quota;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

pub async fn get_provider_credential_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if state.pg_pool.is_none() {
        let quota = load_route_config_provider_credential_quota(
            state.as_ref(),
            provider_credential_id.trim(),
            false,
        )
        .await?;
        return Ok(Json(serde_json::json!({
            "providerQuota": quota,
        })));
    }
    let quota =
        load_provider_credential_quota(state.as_ref(), provider_credential_id.trim(), false)
            .await?;
    Ok(Json(serde_json::json!({
        "providerQuota": quota,
    })))
}

pub async fn refresh_provider_credential_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if state.pg_pool.is_none() {
        let quota = load_route_config_provider_credential_quota(
            state.as_ref(),
            provider_credential_id.trim(),
            true,
        )
        .await?;
        return Ok(Json(serde_json::json!({
            "providerQuota": quota,
        })));
    }
    let quota =
        load_provider_credential_quota(state.as_ref(), provider_credential_id.trim(), true).await?;
    Ok(Json(serde_json::json!({
        "providerQuota": quota,
    })))
}

pub(super) async fn load_provider_credential_quota(
    state: &AppState,
    provider_credential_id: &str,
    force_refresh: bool,
) -> Result<Option<provider_quota::GatewayProviderQuotaView>, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let credential = db::get_provider_credential(pg_pool, provider_credential_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;
    let provider_account =
        db::get_provider_account(pg_pool, credential.provider_account_id.as_str())
            .await?
            .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let merged_payload = db::merge_provider_account_and_credential_payloads(
        &provider_account.payload,
        &credential.payload,
    );
    let merged_payload =
        super::super::internal_provider_accounts::make_provider_payload_serde_compatible(
            &provider_account,
            &merged_payload,
        );
    let payload: ProviderAccountPayload =
        serde_json::from_value(merged_payload).map_err(|error| {
            GatewayError::server_error(format!(
                "deserialize provider payload for credential quota {}: {error}",
                credential.id
            ))
        })?;
    if force_refresh {
        provider_quota::refresh_runtime_quota_snapshot(
            &state.redis_pool,
            state.config.upstream_timeout_secs,
            provider_account.id.as_str(),
            Some(credential.id.as_str()),
            &payload,
        )
        .await
    } else {
        Ok(provider_quota::get_or_refresh_runtime_quota_snapshot(
            &state.redis_pool,
            state.config.upstream_timeout_secs,
            provider_account.id.as_str(),
            Some(credential.id.as_str()),
            &payload,
        )
        .await)
    }
}
