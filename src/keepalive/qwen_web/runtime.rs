//! Apply Qwen runtime material and persist it in the original cache/database order.
use super::super::external_gateway_headers;
use super::types::QwenWebRefreshedRuntime;
use crate::db;
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;
use deadpool_redis::Pool;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::warn;

pub(in crate::keepalive) fn merge_qwen_web_runtime_headers(
    current_headers: &HashMap<String, String>,
    cookie_header: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = external_gateway_headers(current_headers);
    if let Some(cookie_header) = cookie_header {
        headers.insert("Cookie".to_string(), cookie_header.to_string());
    }
    headers
}

pub(in crate::keepalive) fn apply_qwen_web_runtime_refresh(
    payload: &ProviderAccountPayload,
    refreshed: &QwenWebRefreshedRuntime,
) -> ProviderAccountPayload {
    let mut effective = payload.clone();
    effective.api_key = refreshed.api_key.clone();
    effective.expires_at = refreshed.expires_at.clone();
    effective.headers =
        merge_qwen_web_runtime_headers(&effective.headers, refreshed.cookie_header.as_deref());
    if let Some(session_auth) = effective.session_auth.as_mut() {
        session_auth.expires_at = refreshed.expires_at.clone();
    }
    if let Some(account_name) = refreshed.account_name.as_ref() {
        effective.account_name = Some(account_name.clone());
    }
    effective
}

pub(in crate::keepalive) async fn persist_qwen_web_runtime_refresh(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    refreshed: &QwenWebRefreshedRuntime,
) {
    let merged_headers =
        merge_qwen_web_runtime_headers(&payload.headers, refreshed.cookie_header.as_deref());

    if let Some(credential_id) = payload.credential_id.as_deref() {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            Some(refreshed.api_key.as_str()),
            Some(&merged_headers),
            None,
            payload.session_auth.as_ref(),
            payload.keepalive.as_ref(),
            refreshed.expires_at.as_deref(),
            payload.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    if let (Some(pg_pool), Some(credential_id)) = (pg_pool, payload.credential_id.as_deref()) {
        match db::get_provider_credential(pg_pool, credential_id).await {
            Ok(Some(existing)) => {
                let mut updated_payload = existing.payload.clone();
                if let Some(map) = updated_payload.as_object_mut() {
                    map.insert(
                        "apiKey".to_string(),
                        Value::String(refreshed.api_key.clone()),
                    );
                    if let Some(expires_at) = refreshed.expires_at.as_ref() {
                        map.insert("expiresAt".to_string(), Value::String(expires_at.clone()));
                    }
                    let headers_value = map
                        .entry("headers".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(headers_map) = headers_value.as_object_mut() {
                        for (key, value) in &merged_headers {
                            headers_map.insert(key.clone(), Value::String(value.clone()));
                        }
                    }
                    if let Some(account_name) = refreshed.account_name.as_ref() {
                        map.insert(
                            "accountName".to_string(),
                            Value::String(account_name.clone()),
                        );
                    }
                    if let Some(selected_display_model) = refreshed.selected_display_model.as_ref()
                    {
                        map.insert(
                            "selectedDisplayModel".to_string(),
                            Value::String(selected_display_model.clone()),
                        );
                    }
                    if let Some(credential_material_key) =
                        refreshed.credential_material_key.as_ref()
                    {
                        map.insert(
                            "credentialMaterialKey".to_string(),
                            Value::String(credential_material_key.clone()),
                        );
                    }
                }

                let update_input = db::UpsertProviderCredentialInput {
                    provider_account_id: existing.provider_account_id.clone(),
                    label: existing.label.clone(),
                    status: Some(existing.status.clone()),
                    payload: updated_payload,
                    source_kind: Some(existing.source_kind.clone()),
                    source_path: existing.source_path.clone(),
                    source_hash: existing.source_hash.clone(),
                    sync_mode: Some(existing.sync_mode.clone()),
                    sync_state: Some(existing.sync_state.clone()),
                    sync_error: existing.sync_error.clone(),
                };
                if let Err(error) =
                    db::update_provider_credential(pg_pool, credential_id, update_input).await
                {
                    warn!(
                        provider_credential_id = %credential_id,
                        error = %error,
                        "failed to persist refreshed Qwen Web credential payload to Postgres"
                    );
                }
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    provider_credential_id = %credential_id,
                    error = %error,
                    "failed to load Qwen Web credential before persisting refreshed runtime material"
                );
            }
        }
    }
}
