//! Management provider account CRUD and inventory admission.

use super::super::internal_gateway::assert_management_access;
use super::account_input::{into_input, ProviderAccountBody};
use super::quota::load_provider_account_quota;
use super::redaction::{mask_provider_account_view, mask_provider_inventory_view};
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::implementation_lines;
use crate::provider_credential_folder_sync;
use crate::provider_runtime;
use crate::redis::provider_sync;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde_json::Value;
use std::sync::Arc;

pub async fn list_provider_accounts(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let provider_accounts = db::list_provider_accounts(required_pg_pool(state.as_ref())?).await?;
    Ok(Json(serde_json::json!({
        "providerAccounts": provider_accounts.into_iter().map(mask_provider_account_view).collect::<Vec<_>>(),
    })))
}

pub async fn get_provider_account(
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
        "providerAccount": mask_provider_account_view(provider_account),
        "providerQuota": load_provider_account_quota(
            state.as_ref(),
            provider_account_id.trim(),
            false,
        )
        .await?,
    })))
}

pub async fn create_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ProviderAccountBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    implementation_lines::assert_adapter_compiled(
        body.adapter.as_str(),
        &format!(
            "provider account create requested for adapter {}",
            body.adapter.trim()
        ),
    )?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    let provider_account =
        db::create_provider_account(required_pg_pool(state.as_ref())?, into_input(body)).await?;
    provider_sync::set_cached_provider_payload(
        &state.redis_pool,
        &provider_account.id,
        &provider_account.payload,
        None,
    )
    .await
    .map_err(|error| GatewayError::server_error(format!("warm provider cache: {error}")))?;
    Ok(Json(serde_json::json!({
        "providerAccount": mask_provider_account_view(provider_account),
    })))
}

pub async fn update_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderAccountBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    implementation_lines::assert_adapter_compiled(
        body.adapter.as_str(),
        &format!(
            "provider account update requested for adapter {}",
            body.adapter.trim()
        ),
    )?;
    if let Some(protocol_profile) = body.protocol_profile.as_deref() {
        implementation_lines::ensure_protocol_profile_compiled(protocol_profile)?;
    }
    let provider_account = db::update_provider_account(
        required_pg_pool(state.as_ref())?,
        provider_account_id.trim(),
        into_input(body),
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

pub async fn delete_provider_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(provider_account_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    super::super::provider_deletion::run_owned(&state.lifecycle, || {
        let state = Arc::clone(&state);
        let pg_pool = pg_pool.clone();
        async move {
            let provider_account_id = provider_account_id.trim();
            let provider_account = db::get_provider_account(&pg_pool, provider_account_id)
                .await?
                .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
            let credentials =
                db::list_provider_credentials(&pg_pool, Some(provider_account_id)).await?;

            for credential in &credentials {
                if credential.sync_mode == "folder_sync" {
                    if let Some(source_path) = credential.source_path.as_deref() {
                        let _ = provider_credential_folder_sync::delete_synced_credential_file(
                            state.as_ref(),
                            source_path,
                        )?;
                    }
                }
                db::delete_provider_credential(&pg_pool, credential.id.as_str()).await?;
                provider_runtime::clear_provider_credential_runtime_keys(
                    &state.redis_pool,
                    credential.id.as_str(),
                )
                .await?;
            }

            db::delete_provider_account(&pg_pool, provider_account_id).await?;
            provider_sync::delete_cached_provider_payload(&state.redis_pool, provider_account_id)
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("delete provider cache: {error}"))
                })?;
            db::clear_provider_runtime_keys(&state.redis_pool, provider_account_id).await?;

            Ok(Json(serde_json::json!({
                "deleted": true,
                "providerAccountId": provider_account.id,
                "label": provider_account.label,
                "deletedCredentialCount": credentials.len(),
            })))
        }
    })
    .await
}

pub async fn get_provider_inventory(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let inventory = db::get_provider_inventory(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "inventory": mask_provider_inventory_view(inventory),
    })))
}
