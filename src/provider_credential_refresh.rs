use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::{self, GatewayProviderAccountView};
use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::keepalive;
use crate::routing::candidate::{deserialize_provider_payload, ProviderAccountPayload};
use crate::state::AppState;

const CHATGPT_WEB_REVERSE_ADAPTER: &str = "chatgpt_web_reverse_compatible";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialRefreshSweepSummary {
    pub scanned_count: usize,
    pub due_count: usize,
    pub refreshed_count: usize,
    pub skipped_count: usize,
    pub locked_count: usize,
    pub failed_count: usize,
}

#[derive(Debug, Clone)]
struct ProviderCredentialRefreshLock {
    key: String,
    token: String,
}

fn refresh_error_message_for_persistence(message: &str) -> String {
    sanitize_provider_error_message(message)
}

pub async fn start_provider_credential_refresh_task(state: Arc<AppState>) {
    if !state.config.provider_credential_refresh_enabled {
        tracing::info!("provider credential refresh steward disabled");
        return;
    }
    if state.pg_pool.is_none() {
        tracing::info!(
            "provider credential refresh steward disabled because PostgreSQL is not configured"
        );
        return;
    }

    let interval_secs = state
        .config
        .provider_credential_refresh_interval_secs
        .max(60);
    tracing::info!(
        interval_secs,
        batch_limit = state.config.provider_credential_refresh_batch_limit,
        refresh_before_secs = state.config.provider_credential_refresh_before_secs,
        "provider credential refresh steward started"
    );

    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    loop {
        interval.tick().await;
        match sweep_provider_credentials_once(&state).await {
            Ok(summary) => {
                debug!(
                    scanned = summary.scanned_count,
                    due = summary.due_count,
                    refreshed = summary.refreshed_count,
                    skipped = summary.skipped_count,
                    locked = summary.locked_count,
                    failed = summary.failed_count,
                    "provider credential refresh sweep completed"
                );
            }
            Err(error) => {
                warn!(error = %error, "provider credential refresh sweep failed");
            }
        }
    }
}

pub async fn sweep_provider_credentials_once(
    state: &Arc<AppState>,
) -> Result<ProviderCredentialRefreshSweepSummary, GatewayError> {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(ProviderCredentialRefreshSweepSummary::default());
    };

    let credentials = db::list_active_provider_credentials_for_adapter(
        pg_pool,
        CHATGPT_WEB_REVERSE_ADAPTER,
        state.config.provider_credential_refresh_batch_limit,
    )
    .await?;
    let mut account_cache = HashMap::<String, Option<GatewayProviderAccountView>>::new();
    let mut summary = ProviderCredentialRefreshSweepSummary::default();

    for credential in credentials {
        summary.scanned_count += 1;
        let account = match account_cache.get(&credential.provider_account_id) {
            Some(account) => account.clone(),
            None => {
                let account =
                    db::get_provider_account(pg_pool, &credential.provider_account_id).await?;
                account_cache.insert(credential.provider_account_id.clone(), account.clone());
                account
            }
        };
        let Some(account) = account else {
            summary.skipped_count += 1;
            warn!(
                provider_credential_id = %credential.id,
                provider_account_id = %credential.provider_account_id,
                "provider credential refresh skipped because provider account no longer exists"
            );
            continue;
        };
        if account.adapter.trim() != CHATGPT_WEB_REVERSE_ADAPTER
            || account.status.trim() != "active"
        {
            summary.skipped_count += 1;
            continue;
        }

        let payload = match build_chatgpt_web_refresh_payload(
            &account.payload,
            &account.adapter,
            &credential.id,
            &credential.payload,
        ) {
            Ok(payload) => payload,
            Err(error) => {
                summary.failed_count += 1;
                note_refresh_failure(state, &credential.id, credential.failure_count, &error).await;
                continue;
            }
        };

        if !chatgpt_web_refresh_due(
            &payload,
            state.config.provider_credential_refresh_before_secs,
        ) {
            summary.skipped_count += 1;
            continue;
        }
        summary.due_count += 1;

        let Some(lock) = acquire_refresh_lock(
            &state.redis_pool,
            &credential.id,
            state.config.provider_credential_refresh_lock_ttl_secs,
        )
        .await?
        else {
            summary.locked_count += 1;
            continue;
        };

        let refresh_result = keepalive::refresh_chatgpt_web_oauth_payload_if_due(
            &state.redis_pool,
            state.pg_pool.as_ref(),
            state.upstream_client.client(),
            &payload,
            state.config.provider_credential_refresh_before_secs,
            false,
        )
        .await;

        let release_result = release_refresh_lock(&state.redis_pool, &lock).await;
        if let Err(error) = release_result {
            warn!(
                provider_credential_id = %credential.id,
                error = %error,
                "failed to release provider credential refresh lock"
            );
        }

        match refresh_result {
            Ok(Some(_refreshed_payload)) => {
                summary.refreshed_count += 1;
                if let Err(error) =
                    db::note_provider_credential_runtime_success(pg_pool, &credential.id).await
                {
                    warn!(
                        provider_credential_id = %credential.id,
                        error = %error,
                        "failed to record provider credential refresh success"
                    );
                }
                info!(
                    provider_credential_id = %credential.id,
                    provider_account_id = %credential.provider_account_id,
                    "ChatGPT Web provider credential refreshed by background steward"
                );
            }
            Ok(None) => {
                summary.skipped_count += 1;
            }
            Err(error) => {
                summary.failed_count += 1;
                note_refresh_failure(state, &credential.id, credential.failure_count, &error).await;
            }
        }
    }

    Ok(summary)
}

fn build_chatgpt_web_refresh_payload(
    account_payload: &Value,
    adapter: &str,
    credential_id: &str,
    credential_payload: &Value,
) -> Result<ProviderAccountPayload, GatewayError> {
    let merged =
        db::merge_provider_account_and_credential_payloads(account_payload, credential_payload);
    let mut payload = deserialize_provider_payload(&merged, Some(adapter)).map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize ChatGPT Web provider credential payload for refresh: {error}"
        ))
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_refresh_payload_decode_failed")
    })?;
    payload.credential_id = Some(credential_id.to_string());
    Ok(payload)
}

fn chatgpt_web_refresh_due(payload: &ProviderAccountPayload, refresh_before_secs: u64) -> bool {
    keepalive::chatgpt_web_oauth_refresh_due(payload, refresh_before_secs, false)
}

async fn note_refresh_failure(
    state: &Arc<AppState>,
    credential_id: &str,
    current_failure_count: i32,
    error: &GatewayError,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let next_failure_count = current_failure_count.max(0) as u64 + 1;
    let message = refresh_error_message_for_persistence(&error.message);
    if let Err(write_error) = db::note_provider_credential_runtime_failure(
        pg_pool,
        credential_id,
        next_failure_count,
        false,
        0,
        &message,
    )
    .await
    {
        warn!(
            provider_credential_id = %credential_id,
            error = %write_error,
            refresh_error_kind = ?error.kind,
            refresh_error = %message,
            "failed to record provider credential refresh failure"
        );
    }
}

async fn acquire_refresh_lock(
    redis_pool: &deadpool_redis::Pool,
    credential_id: &str,
    lock_ttl_secs: u64,
) -> Result<Option<ProviderCredentialRefreshLock>, GatewayError> {
    let mut conn = redis_pool.get().await.map_err(|error| {
        GatewayError::service_unavailable(format!(
            "provider credential refresh lock Redis connection failed: {error}"
        ))
        .with_provider("provider_credential_refresh")
        .with_code("provider_credential_refresh_redis_unavailable")
    })?;
    let key = format!("gw:provider-credential-refresh-lock:{credential_id}");
    let token = Uuid::new_v4().to_string();
    let lock_ttl_secs = lock_ttl_secs.max(30);
    let result: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("NX")
        .arg("EX")
        .arg(lock_ttl_secs)
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "provider credential refresh lock acquire failed: {error}"
            ))
            .with_provider("provider_credential_refresh")
            .with_code("provider_credential_refresh_lock_failed")
        })?;

    Ok((result.as_deref() == Some("OK")).then_some(ProviderCredentialRefreshLock { key, token }))
}

async fn release_refresh_lock(
    redis_pool: &deadpool_redis::Pool,
    lock: &ProviderCredentialRefreshLock,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool.get().await.map_err(|error| {
        GatewayError::service_unavailable(format!(
            "provider credential refresh lock Redis connection failed: {error}"
        ))
        .with_provider("provider_credential_refresh")
        .with_code("provider_credential_refresh_redis_unavailable")
    })?;
    let script = redis::Script::new(
        "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end",
    );
    let _: i64 = script
        .key(&lock.key)
        .arg(&lock.token)
        .invoke_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!(
                "provider credential refresh lock release failed: {error}"
            ))
            .with_provider("provider_credential_refresh")
            .with_code("provider_credential_refresh_lock_release_failed")
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn build_chatgpt_web_refresh_payload_merges_account_and_credential_material() {
        let account_payload = json!({
            "adapter": "chatgpt_web_reverse_compatible",
            "baseUrl": "https://chatgpt.com",
            "apiKey": "",
            "headers": {
                "User-Agent": "account-ua"
            },
            "extraBody": {
                "oauthClientId": "account-client"
            }
        });
        let credential_payload = json!({
            "apiKey": "expired-access",
            "expiresAt": "2000-01-01T00:00:00Z",
            "extraBody": {
                "refreshToken": "refresh-123"
            }
        });

        let payload = build_chatgpt_web_refresh_payload(
            &account_payload,
            "chatgpt_web_reverse_compatible",
            "cred-1",
            &credential_payload,
        )
        .expect("payload should deserialize");

        assert_eq!(payload.adapter, "chatgpt_web_reverse_compatible");
        assert_eq!(payload.base_url, "https://chatgpt.com");
        assert_eq!(payload.api_key, "expired-access");
        assert_eq!(payload.credential_id.as_deref(), Some("cred-1"));
        assert_eq!(
            payload.headers.get("User-Agent").map(String::as_str),
            Some("account-ua")
        );
        let extra = payload.extra_body.as_ref().expect("extra body");
        assert_eq!(
            extra.get("oauthClientId").and_then(|value| value.as_str()),
            Some("account-client")
        );
        assert_eq!(
            extra.get("refreshToken").and_then(|value| value.as_str()),
            Some("refresh-123")
        );
    }

    #[test]
    fn chatgpt_web_refresh_due_requires_refresh_token_and_near_expiry() {
        let due_payload = build_chatgpt_web_refresh_payload(
            &json!({
                "adapter": "chatgpt_web_reverse_compatible",
                "baseUrl": "https://chatgpt.com"
            }),
            "chatgpt_web_reverse_compatible",
            "cred-due",
            &json!({
                "apiKey": "access",
                "expiresAt": "2000-01-01T00:00:00Z",
                "extraBody": {
                    "refreshToken": "refresh-123"
                }
            }),
        )
        .expect("due payload");
        assert!(chatgpt_web_refresh_due(&due_payload, 86_400));

        let missing_refresh = build_chatgpt_web_refresh_payload(
            &json!({
                "adapter": "chatgpt_web_reverse_compatible",
                "baseUrl": "https://chatgpt.com"
            }),
            "chatgpt_web_reverse_compatible",
            "cred-no-refresh",
            &json!({
                "apiKey": "access",
                "expiresAt": "2000-01-01T00:00:00Z"
            }),
        )
        .expect("missing refresh payload");
        assert!(!chatgpt_web_refresh_due(&missing_refresh, 86_400));

        let fresh_payload = build_chatgpt_web_refresh_payload(
            &json!({
                "adapter": "chatgpt_web_reverse_compatible",
                "baseUrl": "https://chatgpt.com"
            }),
            "chatgpt_web_reverse_compatible",
            "cred-fresh",
            &json!({
                "apiKey": "access",
                "expiresAt": "2099-01-01T00:00:00Z",
                "extraBody": {
                    "refreshToken": "refresh-123"
                }
            }),
        )
        .expect("fresh payload");
        assert!(!chatgpt_web_refresh_due(&fresh_payload, 86_400));
    }

    #[test]
    fn refresh_failure_persistence_boundary_redacts_provider_secrets() {
        let sanitized = refresh_error_message_for_persistence(
            "invalid api key; Authorization: Bearer refresh-secret; session=refresh-session",
        );

        assert!(sanitized.contains("invalid api key"));
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("refresh-secret"));
        assert!(!sanitized.contains("refresh-session"));
    }
}
