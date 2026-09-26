//! Persist runtime material in Redis before the best-effort credential record update.

use std::collections::HashMap;

use deadpool_redis::Pool as RedisPool;
use serde_json::Value;
use sqlx::PgPool;

use crate::db;
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;

pub(crate) fn build_gemini_canvas_runtime_persistence_payload(
    existing_payload: &Value,
    payload: &ProviderAccountPayload,
    extra_body_patch: &HashMap<String, Value>,
    provider_account_base_url: Option<&str>,
) -> Value {
    let mut updated_payload = existing_payload.clone();
    if let Some(map) = updated_payload.as_object_mut() {
        let extra_body = map
            .entry("extraBody".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(extra_body_map) = extra_body.as_object_mut() {
            for (key, value) in extra_body_patch {
                extra_body_map.insert(key.clone(), value.clone());
            }
        }
        if let Some(runtime_state_object_key) = payload.runtime_state_object_key.as_ref() {
            map.insert(
                "runtimeStateObjectKey".to_string(),
                Value::String(runtime_state_object_key.clone()),
            );
        }
        if map
            .get("adapter")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            map.insert(
                "adapter".to_string(),
                Value::String(payload.adapter.clone()),
            );
        }
        if map
            .get("baseUrl")
            .or_else(|| map.get("base_url"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            let fallback_base_url = if payload.base_url.trim().is_empty() {
                provider_account_base_url.map(str::to_string)
            } else {
                Some(payload.base_url.clone())
            };
            if let Some(base_url) = fallback_base_url {
                map.insert("baseUrl".to_string(), Value::String(base_url));
            }
        }
    }
    updated_payload
}

pub(crate) async fn persist_gemini_canvas_program_runtime_material(
    redis_pool: Option<&RedisPool>,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    extra_body_patch: Option<HashMap<String, Value>>,
) {
    let Some(credential_id) = payload.credential_id.as_deref() else {
        return;
    };
    let Some(extra_body_patch) = extra_body_patch else {
        return;
    };

    if let Some(redis_pool) = redis_pool {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            None,
            None,
            Some(&extra_body_patch),
            payload.session_auth.as_ref(),
            payload.keepalive.as_ref(),
            payload.expires_at.as_deref(),
            payload.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    if let Some(pg_pool) = pg_pool {
        match db::get_provider_credential(pg_pool, credential_id).await {
            Ok(Some(existing)) => {
                let provider_account_base_url =
                    db::get_provider_account(pg_pool, existing.provider_account_id.as_str())
                        .await
                        .ok()
                        .flatten()
                        .and_then(|provider_account| {
                            provider_account
                                .payload
                                .get("baseUrl")
                                .or_else(|| provider_account.payload.get("base_url"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string)
                        });
                let updated_payload = build_gemini_canvas_runtime_persistence_payload(
                    &existing.payload,
                    payload,
                    &extra_body_patch,
                    provider_account_base_url.as_deref(),
                );

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
                let _ = db::update_provider_credential(pg_pool, credential_id, update_input).await;
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }
}
