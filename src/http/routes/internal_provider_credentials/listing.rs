//! Management credential listings and credential-detail admission.

use super::super::internal_gateway::assert_management_access;
use super::super::provider_credential_quota_batch::collect_bounded_ordered;
use super::required_pg_pool;
use super::route_config::list_route_config_provider_credentials;
use super::views::{
    build_provider_credential_response, build_provider_credential_response_value,
    mask_provider_account_view,
};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_quota;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialListQuery {
    #[serde(default, alias = "maskSecrets")]
    pub mask_secrets: Option<bool>,
}

pub async fn list_provider_credentials(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<ProviderCredentialListQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if state.pg_pool.is_none() {
        return Ok(Json(
            list_route_config_provider_credentials(
                state.as_ref(),
                query.mask_secrets.unwrap_or(true),
            )
            .await?,
        ));
    }
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_accounts = db::list_provider_accounts(pg_pool).await?;
    let provider_payloads = provider_accounts
        .into_iter()
        .map(|provider_account| (provider_account.id, Arc::new(provider_account.payload)))
        .collect::<HashMap<_, _>>();
    let credentials = db::list_provider_credentials(pg_pool, None).await?;
    let credential_inputs = credentials
        .into_iter()
        .map(|credential| {
            let provider_account_payload = provider_payloads
                .get(credential.provider_account_id.as_str())
                .cloned()
                .ok_or_else(|| {
                    GatewayError::server_error(format!(
                        "provider credential {} references missing provider account {}",
                        credential.id, credential.provider_account_id
                    ))
                })?;
            Ok((credential, provider_account_payload))
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;
    let mask_secrets = query.mask_secrets.unwrap_or(true);
    let redis_pool = &state.redis_pool;
    let items = collect_bounded_ordered(credential_inputs.into_iter().map(
        |(credential, provider_account_payload)| async move {
            let quota = provider_quota::read_cached_runtime_quota_snapshot(
                redis_pool,
                credential.provider_account_id.as_str(),
                Some(credential.id.as_str()),
            )
            .await
            .ok()
            .flatten();
            build_provider_credential_response_value(
                provider_account_payload.as_ref(),
                credential,
                mask_secrets,
                quota,
            )
        },
    ))
    .await;

    Ok(Json(serde_json::json!({ "credentials": items })))
}

pub async fn list_provider_credentials_for_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_account_id): Path<String>,
    Query(query): Query<ProviderCredentialListQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id.trim())
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let credentials =
        db::list_provider_credentials(pg_pool, Some(provider_account_id.trim())).await?;
    let mut items = Vec::with_capacity(credentials.len());
    for credential in credentials {
        items.push(
            build_provider_credential_response(
                state.as_ref(),
                &provider_account.payload,
                credential,
                query.mask_secrets.unwrap_or(true),
            )
            .await?,
        );
    }
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
        "credentials": items,
    })))
}

pub async fn get_provider_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
    Query(query): Query<ProviderCredentialListQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let credential = db::get_provider_credential(pg_pool, provider_credential_id.trim())
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;
    let provider_account =
        db::get_provider_account(pg_pool, credential.provider_account_id.as_str())
            .await?
            .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;

    Ok(Json(serde_json::json!({
        "providerCredential": build_provider_credential_response(
            state.as_ref(),
            &provider_account.payload,
            credential,
            query.mask_secrets.unwrap_or(true),
        )
        .await?,
    })))
}
