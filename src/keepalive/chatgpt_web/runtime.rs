//! Project refreshed material and persist it in the original Redis/PostgreSQL order.
use super::super::{external_gateway_headers, upsert_header_case_insensitive};
use super::types::ChatGptWebRefreshedRuntime;
use crate::db;
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;
use deadpool_redis::Pool;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use time::OffsetDateTime;
use tracing::warn;

pub(in crate::keepalive) fn merge_chatgpt_web_runtime_headers(
    current_headers: &HashMap<String, String>,
    cookie_header: Option<&str>,
    user_agent: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = external_gateway_headers(current_headers);
    if let Some(cookie_header) = cookie_header {
        upsert_header_case_insensitive(&mut headers, "Cookie", cookie_header.to_string());
    }
    if let Some(user_agent) = user_agent {
        upsert_header_case_insensitive(&mut headers, "User-Agent", user_agent.to_string());
    }
    headers
}

pub(in crate::keepalive) fn apply_chatgpt_web_runtime_refresh(
    payload: &ProviderAccountPayload,
    refreshed: &ChatGptWebRefreshedRuntime,
) -> ProviderAccountPayload {
    let mut effective = payload.clone();
    effective.api_key = refreshed.api_key.clone();
    effective.expires_at = refreshed.expires_at.clone();
    effective.headers = merge_chatgpt_web_runtime_headers(
        &effective.headers,
        refreshed.cookie_header.as_deref(),
        refreshed.user_agent.as_deref(),
    );
    if let Some(session_auth) = effective.session_auth.as_mut() {
        session_auth.expires_at = refreshed.expires_at.clone();
    }
    let extra_body = effective.extra_body.get_or_insert_with(HashMap::new);
    if let Some(device_id) = refreshed.device_id.as_ref() {
        extra_body.insert("deviceId".to_string(), Value::String(device_id.clone()));
    }
    if let Some(session_id) = refreshed.session_id.as_ref() {
        extra_body.insert("sessionId".to_string(), Value::String(session_id.clone()));
    }
    if let Some(client_version) = refreshed.client_version.as_ref() {
        extra_body.insert(
            "clientVersion".to_string(),
            Value::String(client_version.clone()),
        );
    }
    if let Some(client_build_number) = refreshed.client_build_number.as_ref() {
        extra_body.insert(
            "clientBuildNumber".to_string(),
            Value::String(client_build_number.clone()),
        );
    }
    if let Some(user_agent) = refreshed.user_agent.as_ref() {
        extra_body.insert("userAgent".to_string(), Value::String(user_agent.clone()));
    }
    if let Some(language) = refreshed.language.as_ref() {
        extra_body.insert("language".to_string(), Value::String(language.clone()));
    }
    if let Some(language_code) = refreshed.language_code.as_ref() {
        extra_body.insert(
            "languageCode".to_string(),
            Value::String(language_code.clone()),
        );
    }
    if let Some(timezone) = refreshed.timezone.as_ref() {
        extra_body.insert("timezone".to_string(), Value::String(timezone.clone()));
    }
    if let Some(pow_sources) = refreshed.chatgpt_pow_sources.as_ref() {
        extra_body.insert(
            "chatgptPowSources".to_string(),
            Value::Array(
                pow_sources
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(pow_data_build) = refreshed.chatgpt_pow_data_build.as_ref() {
        extra_body.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.clone()),
        );
    }
    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
        extra_body.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.clone()),
        );
        extra_body.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body.insert(
            "lastTokenRefreshAt".to_string(),
            Value::String(db::format_timestamp(OffsetDateTime::now_utc())),
        );
    }
    if let Some(id_token) = refreshed.id_token.as_ref() {
        extra_body.insert("idToken".to_string(), Value::String(id_token.clone()));
    }
    if let Some(expires_at) = refreshed.expires_at.as_ref() {
        extra_body.insert(
            "accessTokenExpiresAt".to_string(),
            Value::String(expires_at.clone()),
        );
    }
    if let Some(account_name) = refreshed.account_name.as_ref() {
        effective.account_name = Some(account_name.clone());
    }
    effective
}

pub(in crate::keepalive) async fn persist_chatgpt_web_runtime_refresh(
    redis_pool: &Pool,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    refreshed: &ChatGptWebRefreshedRuntime,
) {
    let merged_headers = merge_chatgpt_web_runtime_headers(
        &payload.headers,
        refreshed.cookie_header.as_deref(),
        refreshed.user_agent.as_deref(),
    );
    let mut extra_body_patch = payload.extra_body.clone().unwrap_or_default();
    if let Some(device_id) = refreshed.device_id.as_ref() {
        extra_body_patch.insert("deviceId".to_string(), Value::String(device_id.clone()));
    }
    if let Some(session_id) = refreshed.session_id.as_ref() {
        extra_body_patch.insert("sessionId".to_string(), Value::String(session_id.clone()));
    }
    if let Some(client_version) = refreshed.client_version.as_ref() {
        extra_body_patch.insert(
            "clientVersion".to_string(),
            Value::String(client_version.clone()),
        );
    }
    if let Some(client_build_number) = refreshed.client_build_number.as_ref() {
        extra_body_patch.insert(
            "clientBuildNumber".to_string(),
            Value::String(client_build_number.clone()),
        );
    }
    if let Some(user_agent) = refreshed.user_agent.as_ref() {
        extra_body_patch.insert("userAgent".to_string(), Value::String(user_agent.clone()));
    }
    if let Some(language) = refreshed.language.as_ref() {
        extra_body_patch.insert("language".to_string(), Value::String(language.clone()));
    }
    if let Some(language_code) = refreshed.language_code.as_ref() {
        extra_body_patch.insert(
            "languageCode".to_string(),
            Value::String(language_code.clone()),
        );
    }
    if let Some(timezone) = refreshed.timezone.as_ref() {
        extra_body_patch.insert("timezone".to_string(), Value::String(timezone.clone()));
    }
    if let Some(pow_sources) = refreshed.chatgpt_pow_sources.as_ref() {
        extra_body_patch.insert(
            "chatgptPowSources".to_string(),
            Value::Array(
                pow_sources
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    if let Some(pow_data_build) = refreshed.chatgpt_pow_data_build.as_ref() {
        extra_body_patch.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.clone()),
        );
    }
    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
        extra_body_patch.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.clone()),
        );
        extra_body_patch.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body_patch.insert(
            "lastTokenRefreshAt".to_string(),
            Value::String(db::format_timestamp(OffsetDateTime::now_utc())),
        );
    }
    if let Some(id_token) = refreshed.id_token.as_ref() {
        extra_body_patch.insert("idToken".to_string(), Value::String(id_token.clone()));
    }
    if let Some(expires_at) = refreshed.expires_at.as_ref() {
        extra_body_patch.insert(
            "accessTokenExpiresAt".to_string(),
            Value::String(expires_at.clone()),
        );
    }

    if let Some(credential_id) = payload.credential_id.as_deref() {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            Some(refreshed.api_key.as_str()),
            Some(&merged_headers),
            Some(&extra_body_patch),
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
                    if let Some(refresh_token) = refreshed.refresh_token.as_ref() {
                        map.insert(
                            "refreshToken".to_string(),
                            Value::String(refresh_token.clone()),
                        );
                    }
                    if let Some(id_token) = refreshed.id_token.as_ref() {
                        map.insert("idToken".to_string(), Value::String(id_token.clone()));
                    }
                    let headers_value = map
                        .entry("headers".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(headers_map) = headers_value.as_object_mut() {
                        for (key, value) in &merged_headers {
                            headers_map.insert(key.clone(), Value::String(value.clone()));
                        }
                    }
                    let extra_body_value = map
                        .entry("extraBody".to_string())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                    if let Some(extra_body_map) = extra_body_value.as_object_mut() {
                        for (key, value) in &extra_body_patch {
                            extra_body_map.insert(key.clone(), value.clone());
                        }
                    }
                    if let Some(account_name) = refreshed.account_name.as_ref() {
                        map.insert(
                            "accountName".to_string(),
                            Value::String(account_name.clone()),
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
                        "failed to persist refreshed ChatGPT Web credential payload to Postgres"
                    );
                }
            }
            Ok(None) => {}
            Err(error) => {
                warn!(
                    provider_credential_id = %credential_id,
                    error = %error,
                    "failed to load ChatGPT Web credential before persisting refreshed runtime material"
                );
            }
        }
    }
}
