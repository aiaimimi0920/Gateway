//! Management credential creation, partial updates and deletion lifecycle.

use super::super::internal_gateway::assert_management_access;
use super::credential_payload::normalize_provider_credential_payload_for_storage;
use super::required_pg_pool;
use super::views::build_provider_credential_response;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_credential_folder_sync;
use crate::provider_runtime;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialCreateBody {
    #[serde(default, alias = "providerAccountId")]
    pub provider_account_id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(alias = "credential")]
    pub payload: Value,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub sync_mode: Option<String>,
    #[serde(default)]
    pub sync_state: Option<String>,
    #[serde(default)]
    pub sync_error: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialUpdateBody {
    #[serde(default)]
    pub provider_account_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, alias = "credential")]
    pub payload: Option<Value>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub sync_mode: Option<String>,
    #[serde(default)]
    pub sync_state: Option<String>,
    #[serde(default)]
    pub sync_error: Option<String>,
}

pub async fn create_provider_credential_for_account(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_account_id): Path<String>,
    Json(body): Json<ProviderCredentialCreateBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let created =
        create_provider_credential_inner(state.as_ref(), Some(provider_account_id), body).await?;
    Ok(Json(serde_json::json!({
        "providerCredential": created,
    })))
}

pub async fn create_provider_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ProviderCredentialCreateBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let created = create_provider_credential_inner(state.as_ref(), None, body).await?;
    Ok(Json(serde_json::json!({
        "providerCredential": created,
    })))
}

pub async fn update_provider_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
    Json(body): Json<ProviderCredentialUpdateBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    let existing = db::get_provider_credential(pg_pool, provider_credential_id.trim())
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;
    let provider_account_id = body
        .provider_account_id
        .clone()
        .unwrap_or_else(|| existing.provider_account_id.clone());
    let provider_account = db::get_provider_account(pg_pool, provider_account_id.as_str())
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let next_payload = body
        .payload
        .clone()
        .unwrap_or_else(|| existing.payload.clone());
    let normalized_payload =
        normalize_provider_credential_payload_for_storage(&provider_account, &next_payload);
    let updated = db::update_provider_credential(
        pg_pool,
        provider_credential_id.trim(),
        db::UpsertProviderCredentialInput {
            provider_account_id,
            label: body.label.clone().unwrap_or_else(|| existing.label.clone()),
            status: body
                .status
                .clone()
                .or_else(|| Some(existing.status.clone())),
            payload: normalized_payload,
            source_kind: body
                .source_kind
                .clone()
                .or_else(|| Some(existing.source_kind.clone())),
            source_path: body.source_path.clone().or(existing.source_path.clone()),
            source_hash: body.source_hash.clone().or(existing.source_hash.clone()),
            sync_mode: body
                .sync_mode
                .clone()
                .or_else(|| Some(existing.sync_mode.clone())),
            sync_state: body
                .sync_state
                .clone()
                .or_else(|| Some(existing.sync_state.clone())),
            sync_error: body.sync_error.clone().or(existing.sync_error.clone()),
        },
    )
    .await?;
    provider_runtime::clear_provider_credential_runtime_keys(
        &state.redis_pool,
        updated.id.as_str(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "providerCredential": build_provider_credential_response(
            state.as_ref(),
            &provider_account.payload,
            updated,
            true,
        )
        .await?,
    })))
}

pub async fn delete_provider_credential(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = required_pg_pool(state.as_ref())?;
    super::super::provider_deletion::run_owned(&state.lifecycle, || {
        let state = Arc::clone(&state);
        let pg_pool = pg_pool.clone();
        async move {
            let existing = db::get_provider_credential(&pg_pool, provider_credential_id.trim())
                .await?
                .ok_or_else(|| GatewayError::not_found("Provider credential 不存在"))?;
            if existing.sync_mode == "folder_sync" {
                if let Some(source_path) = existing.source_path.as_deref() {
                    let _ = provider_credential_folder_sync::delete_synced_credential_file(
                        state.as_ref(),
                        source_path,
                    )?;
                }
            }
            if let Err(error) =
                db::delete_provider_credential(&pg_pool, provider_credential_id.trim()).await
            {
                if error.http_status != Some(404) {
                    return Err(error);
                }
            }
            provider_runtime::clear_provider_credential_runtime_keys(
                &state.redis_pool,
                provider_credential_id.trim(),
            )
            .await?;
            Ok(Json(serde_json::json!({
                "success": true,
                "providerCredentialId": provider_credential_id.trim(),
                "providerAccountId": existing.provider_account_id,
                "message": "服务商凭证已删除",
            })))
        }
    })
    .await
}

async fn create_provider_credential_inner(
    state: &AppState,
    provider_account_id_from_path: Option<String>,
    body: ProviderCredentialCreateBody,
) -> Result<Value, GatewayError> {
    let pg_pool = required_pg_pool(state)?;
    let provider_account_id = provider_account_id_from_path
        .unwrap_or_else(|| body.provider_account_id.clone().unwrap_or_default())
        .trim()
        .to_string();
    if provider_account_id.is_empty() {
        return Err(GatewayError::bad_request("providerAccountId 不能为空"));
    }
    let provider_account = db::get_provider_account(pg_pool, provider_account_id.as_str())
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let normalized_payload =
        normalize_provider_credential_payload_for_storage(&provider_account, &body.payload);
    let created = db::create_provider_credential(
        pg_pool,
        db::UpsertProviderCredentialInput {
            provider_account_id,
            label: body.label,
            status: body.status,
            payload: normalized_payload,
            source_kind: body.source_kind,
            source_path: body.source_path,
            source_hash: body.source_hash,
            sync_mode: body.sync_mode,
            sync_state: body.sync_state,
            sync_error: body.sync_error,
        },
    )
    .await?;
    build_provider_credential_response(state, &provider_account.payload, created, true).await
}
