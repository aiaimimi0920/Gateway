use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::provider_credential_folder_sync::{self, FolderSyncDirection};
use crate::provider_quota;
use crate::provider_runtime;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

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

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialListQuery {
    #[serde(default, alias = "maskSecrets")]
    pub mask_secrets: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialFolderSyncStatusUpdateBody {
    pub enabled: bool,
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
    let existing = db::get_provider_credential(pg_pool, provider_credential_id.trim())
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
    if let Err(error) = db::delete_provider_credential(pg_pool, provider_credential_id.trim()).await
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

pub async fn get_provider_credential_quota(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Path(provider_credential_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
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
    let quota =
        load_provider_credential_quota(state.as_ref(), provider_credential_id.trim(), true).await?;
    Ok(Json(serde_json::json!({
        "providerQuota": quota,
    })))
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

async fn load_provider_credential_quota(
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
    let merged_payload = super::internal_provider_accounts::make_provider_payload_serde_compatible(
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

async fn build_provider_credential_response(
    state: &AppState,
    provider_account_payload: &Value,
    credential: db::GatewayProviderCredentialView,
    mask_secrets: bool,
) -> Result<Value, GatewayError> {
    let quota = match load_provider_credential_quota(state, credential.id.as_str(), false).await {
        Ok(value) => value,
        Err(error)
            if error
                .message
                .contains("deserialize provider payload for credential quota") =>
        {
            None
        }
        Err(error) => return Err(error),
    };
    let credential_payload = credential.payload.clone();
    Ok(serde_json::json!({
        "id": credential.id,
        "providerAccountId": credential.provider_account_id,
        "label": credential.label,
        "status": credential.status,
        "credential": if mask_secrets { mask_provider_payload_secrets(credential_payload.clone()) } else { credential_payload.clone() },
        "credentialMaterialKey": derive_credential_material_key(&credential_payload),
        "selectedDisplayModel": read_selected_display_model(&credential_payload),
        "supportedModels": read_supported_models(&credential_payload),
        "storageMode": credential.storage_mode,
        "sourceKind": credential.source_kind,
        "sourcePath": credential.source_path,
        "sourceHash": credential.source_hash,
        "syncMode": credential.sync_mode,
        "syncState": credential.sync_state,
        "syncError": credential.sync_error,
        "cooldownUntil": credential.cooldown_until,
        "lastError": credential.last_error,
        "failureCount": credential.failure_count,
        "lastHealthCheckAt": credential.last_health_check_at,
        "createdAt": credential.created_at,
        "updatedAt": credential.updated_at,
        "archivedAt": credential.archived_at,
        "providerQuota": quota,
        "sharedPayloadHints": shared_provider_payload_hints(provider_account_payload),
    }))
}

fn shared_provider_payload_hints(payload: &Value) -> Value {
    serde_json::json!({
        "baseUrl": payload.get("baseUrl").cloned().or_else(|| payload.get("base_url").cloned()),
        "defaultModel": payload.get("defaultModel").cloned().or_else(|| payload.get("default_model").cloned()),
        "accountLabel": payload.get("accountLabel").cloned().or_else(|| payload.get("account_label").cloned()),
    })
}

fn read_selected_display_model(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    [
        "selectedDisplayModel",
        "selectedModelDisplayName",
        "resolvedDisplayModel",
        "boundDisplayModel",
        "boundModelLabel",
        "displayModel",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

fn read_supported_models(payload: &Value) -> Vec<String> {
    fn push_text_like(value: &Value, values: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                for item in text
                    .split([',', '\n'])
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                {
                    values.push(item.to_string());
                }
            }
            Value::Array(items) => {
                for item in items {
                    push_text_like(item, values);
                }
            }
            _ => {}
        }
    }

    let Some(object) = payload.as_object() else {
        return Vec::new();
    };

    let mut values = Vec::new();
    for key in [
        "supportedModels",
        "supported_models",
        "allowedModels",
        "allowed_models",
        "modelCode",
        "model_code",
        "modelId",
        "model_id",
        "defaultModel",
        "default_model",
        "model",
    ] {
        if let Some(value) = object.get(key) {
            push_text_like(value, &mut values);
        }
    }

    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn derive_credential_material_key(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    if let Some(explicit) = [
        "credentialMaterialKey",
        "sharedCredentialKey",
        "credentialFamilyKey",
        "materialKey",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let extra = object
        .get("extraBody")
        .or_else(|| object.get("extra_body"))
        .and_then(Value::as_object);
    let mut components: Vec<(&str, String)> = Vec::new();
    for key in [
        "apiKey",
        "api_key",
        "authToken",
        "auth_token",
        "apiSecret",
        "api_secret",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                components.push((key, trimmed.to_string()));
            }
        }
    }
    for key in ["appId", "app_id", "uid"] {
        if let Some(value) = extra.and_then(|map| map.get(key)).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                components.push((key, trimmed.to_string()));
            }
        }
    }
    if components.is_empty() {
        return None;
    }

    components.sort_by(|left, right| left.0.cmp(right.0));
    let mut hasher = Sha256::new();
    for (key, value) in components {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut fingerprint = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(&mut fingerprint, "{byte:02x}");
    }
    Some(format!("credmat:{fingerprint}"))
}

fn mask_provider_account_view(provider_account: db::GatewayProviderAccountView) -> Value {
    serde_json::json!({
        "id": provider_account.id,
        "label": provider_account.label,
        "adapter": provider_account.adapter,
        "protocolFamily": provider_account.protocol_family,
        "status": provider_account.status,
        "sourceProfile": {
            "sourceKind": provider_account.source_kind,
            "aggregatorApiMode": provider_account.aggregator_api_mode,
            "webReverseAccessMode": provider_account.web_reverse_access_mode,
            "notes": provider_account.source_notes,
            "derived": false,
        },
        "executionMode": provider_account.execution_mode,
        "endpointExecutionModes": provider_account.endpoint_execution_modes,
        "payload": mask_provider_payload_secrets(provider_account.payload),
        "createdAt": provider_account.created_at,
        "updatedAt": provider_account.updated_at,
        "cooldownUntil": provider_account.cooldown_until,
        "lastError": provider_account.last_error,
        "failureCount": provider_account.failure_count,
    })
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

fn normalize_provider_credential_payload_for_storage(
    provider_account: &db::GatewayProviderAccountView,
    payload: &Value,
) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("adapter") {
            object.insert(
                "adapter".to_string(),
                Value::String(provider_account.adapter.clone()),
            );
        }
        strip_empty_top_level_override(object, "baseUrl");
        strip_empty_top_level_override(object, "base_url");
        strip_empty_top_level_override(object, "apiKey");
        strip_empty_top_level_override(object, "api_key");
    }
    payload
}

fn strip_empty_top_level_override(object: &mut serde_json::Map<String, Value>, key: &str) {
    let should_remove = object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value.is_empty());
    if should_remove {
        object.remove(key);
    }
}

fn mask_provider_payload_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let lower = key.to_lowercase();
                    let next = if lower == "headers" {
                        mask_headers(value)
                    } else if is_secret_key(&lower) {
                        Value::String(mask_secret_string(value.as_str().unwrap_or("***")))
                    } else {
                        mask_provider_payload_secrets(value)
                    };
                    (key, next)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(mask_provider_payload_secrets)
                .collect(),
        ),
        other => other,
    }
}

fn mask_headers(value: Value) -> Value {
    match value {
        Value::Object(headers) => Value::Object(
            headers
                .into_iter()
                .map(|(key, value)| {
                    let masked = if is_secret_key(&key.to_lowercase()) {
                        Value::String(mask_secret_string(value.as_str().unwrap_or("***")))
                    } else {
                        value
                    };
                    (key, masked)
                })
                .collect(),
        ),
        other => other,
    }
}

fn is_secret_key(key: &str) -> bool {
    key.contains("auth")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("cookie")
        || key.contains("apikey")
        || key.ends_with("key")
}

fn mask_secret_string(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 8 {
        return "***".to_string();
    }
    format!("{}***{}", &trimmed[..3], &trimmed[trimmed.len() - 3..])
}

#[cfg(test)]
mod tests {
    use super::normalize_provider_credential_payload_for_storage;
    use crate::db::GatewayProviderAccountView;
    use crate::routing::candidate::ProviderExecutionMode;
    use serde_json::{json, Value};

    fn build_test_provider_account() -> GatewayProviderAccountView {
        GatewayProviderAccountView {
            id: "provider-accio-live".to_string(),
            label: "Accio Live".to_string(),
            service_provider_key: "accio_platform".to_string(),
            service_provider_label: "Accio".to_string(),
            adapter: "accio_compatible".to_string(),
            protocol_family: "openai_responses".to_string(),
            protocol_profile: "accio".to_string(),
            status: "active".to_string(),
            source_kind: Some("web_reverse_api".to_string()),
            aggregator_api_mode: None,
            web_reverse_access_mode: Some("direct_http_replay".to_string()),
            source_notes: Some(
                "Accio phoenix-gw live regression surface backed by imported manager accounts."
                    .to_string(),
            ),
            execution_mode: ProviderExecutionMode::DirectHttp,
            endpoint_execution_modes: None,
            payload: json!({
                "adapter": "accio_compatible",
                "baseUrl": "https://phoenix-gw.alibaba.com",
                "defaultModel": "claude-opus-4-6",
            }),
            storage_mode: "inline".to_string(),
            cooldown_until: None,
            last_error: None,
            failure_count: 0,
            last_health_check_at: None,
            created_at: "2026-05-17T00:00:00Z".to_string(),
            updated_at: "2026-05-17T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn provider_credential_storage_normalization_drops_empty_top_level_overrides() {
        let provider = build_test_provider_account();
        let normalized = normalize_provider_credential_payload_for_storage(
            &provider,
            &json!({
                "baseUrl": "",
                "apiKey": "   ",
                "headers": {
                    "Cookie": "cna=test-cna"
                }
            }),
        );
        let object = normalized
            .as_object()
            .expect("normalized provider credential payload should stay object");
        assert_eq!(
            object.get("adapter").and_then(Value::as_str),
            Some("accio_compatible")
        );
        assert!(
            !object.contains_key("baseUrl"),
            "empty baseUrl should not persist as credential-level override"
        );
        assert!(
            !object.contains_key("apiKey"),
            "empty apiKey should not persist as credential-level override"
        );
        let headers = object
            .get("headers")
            .and_then(Value::as_object)
            .expect("headers should remain an object");
        assert_eq!(
            headers.get("Cookie").and_then(Value::as_str),
            Some("cna=test-cna")
        );
    }
}
