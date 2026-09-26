//! Management folder-sync status, enablement and explicit import/export operations.

use super::super::internal_gateway::assert_management_access;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_credential_folder_sync;
use crate::provider_credential_folder_sync::FolderSyncDirection;
use crate::state::AppState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialFolderSyncStatusUpdateBody {
    pub enabled: bool,
}

pub async fn get_provider_credential_folder_sync_status(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let status = provider_credential_folder_sync::get_folder_sync_status(state.as_ref()).await?;
    Ok(Json(serde_json::json!({
        "status": status,
    })))
}

pub async fn update_provider_credential_folder_sync_status(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ProviderCredentialFolderSyncStatusUpdateBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let status =
        provider_credential_folder_sync::set_runtime_enabled(state.as_ref(), body.enabled).await?;
    Ok(Json(serde_json::json!({
        "status": status,
        "message": if body.enabled {
            "已启用服务商凭证文件夹同步模式"
        } else {
            "已停用服务商凭证文件夹同步模式"
        },
    })))
}

pub async fn import_provider_credentials_from_folder(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let status = provider_credential_folder_sync::run_folder_sync_once(
        state.as_ref(),
        FolderSyncDirection::Import,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "status": status,
        "message": "已执行服务商凭证文件夹导入",
    })))
}

pub async fn export_provider_credentials_to_folder(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let status = provider_credential_folder_sync::run_folder_sync_once(
        state.as_ref(),
        FolderSyncDirection::Export,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "status": status,
        "message": "已执行服务商凭证文件夹导出",
    })))
}
