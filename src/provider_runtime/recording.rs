use std::sync::Arc;

use redis::AsyncCommands;
use tracing::warn;

use super::{
    provider_error_message_for_persistence, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::db;
use crate::error::GatewayError;
use crate::state::AppState;

pub async fn record_provider_success(
    state: &Arc<AppState>,
    provider_account_id: &str,
) -> Result<(), GatewayError> {
    if !is_runtime_managed_provider(provider_account_id) {
        return Ok(());
    }
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    db::note_provider_runtime_success(&state.redis_pool, pg_pool, provider_account_id).await
}

pub async fn record_provider_candidate_success(
    state: &Arc<AppState>,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> Result<(), GatewayError> {
    let Some(provider_credential_id) = provider_credential_id else {
        return record_provider_success(state, provider_account_id).await;
    };
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    clear_provider_credential_runtime_keys(&state.redis_pool, provider_credential_id).await?;
    db::note_provider_credential_runtime_success(pg_pool, provider_credential_id).await
}

pub async fn record_provider_failure(
    state: &Arc<AppState>,
    provider_account_id: &str,
    route_policy: Option<&db::GatewayRoutePolicyConfig>,
    message: &str,
) -> Result<(), GatewayError> {
    if !is_runtime_managed_provider(provider_account_id) {
        return Ok(());
    }
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    let fallback_policy = db::GatewayRoutePolicyConfig::default();
    let message = provider_error_message_for_persistence(message);
    db::note_provider_runtime_failure(
        &state.redis_pool,
        pg_pool,
        provider_account_id,
        route_policy.unwrap_or(&fallback_policy),
        &message,
    )
    .await
}

pub async fn record_provider_candidate_failure(
    state: &Arc<AppState>,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    route_policy: Option<&db::GatewayRoutePolicyConfig>,
    message: &str,
) -> Result<(), GatewayError> {
    let Some(provider_credential_id) = provider_credential_id else {
        return record_provider_failure(state, provider_account_id, route_policy, message).await;
    };
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(());
    };
    let fallback_policy = db::GatewayRoutePolicyConfig::default();
    let route_policy = route_policy.unwrap_or(&fallback_policy);
    let ttl_seconds = route_policy.circuit_breaker_cooldown_seconds.max(30) as u64;
    let threshold = route_policy.circuit_breaker_threshold.max(1) as u64;
    let permanent_failure = is_permanent_provider_credential_failure(message);
    let message = provider_error_message_for_persistence(message);
    let failure_count_key =
        crate::redis::keys::provider_credential_failure_count_key(provider_credential_id);
    let breaker_open_key =
        crate::redis::keys::provider_credential_breaker_open_key(provider_credential_id);
    let failure_count = {
        let mut conn = state.redis_pool.get().await.map_err(|error| {
            GatewayError::server_error(format!("get redis connection: {error}"))
        })?;
        let failure_count: u64 = conn.incr(&failure_count_key, 1).await.map_err(|error| {
            GatewayError::server_error(format!("increment provider credential failures: {error}"))
        })?;
        let _: bool = conn
            .expire(&failure_count_key, ttl_seconds as i64)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("expire provider credential failures: {error}"))
            })?;
        if permanent_failure || failure_count >= threshold {
            let _: () = redis::cmd("SET")
                .arg(&breaker_open_key)
                .arg("1")
                .arg("EX")
                .arg(ttl_seconds)
                .query_async(&mut conn)
                .await
                .map_err(|error| {
                    GatewayError::server_error(format!("open provider credential breaker: {error}"))
                })?;
        }
        failure_count
    };

    db::note_provider_credential_runtime_failure(
        pg_pool,
        provider_credential_id,
        failure_count,
        permanent_failure || failure_count >= threshold,
        ttl_seconds,
        &message,
    )
    .await
}

pub(super) fn is_permanent_provider_credential_failure(message: &str) -> bool {
    if crate::provider_failure::classify_provider_failure(None, None, Some(message)).permanent {
        return true;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("token_invalidated")
        || lower.contains("token_revoked")
        || lower.contains("token has been invalidated")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("appidnoautherror")
        || lower.contains("deactivated_workspace")
}

pub async fn read_provider_breaker_open(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
) -> bool {
    read_runtime_breaker_open(redis_pool, provider_account_id, None).await
}

pub async fn read_runtime_breaker_open(
    redis_pool: &deadpool_redis::Pool,
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
) -> bool {
    let Ok(mut conn) = redis_pool.get().await else {
        return false;
    };
    let key = provider_credential_id
        .map(crate::redis::keys::provider_credential_breaker_open_key)
        .unwrap_or_else(|| crate::redis::keys::provider_breaker_open_key(provider_account_id));
    conn.get::<_, Option<String>>(key)
        .await
        .ok()
        .flatten()
        .is_some()
}

pub async fn record_provider_credential_probe_report(
    state: &AppState,
    provider_id: &str,
    credential_id: &str,
    report: &ProviderPayloadProbeReport,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let credential_exists = match db::get_provider_credential(pg_pool, credential_id).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            warn!(
                credential_id,
                error = %error,
                "failed to inspect provider credential before recording probe result"
            );
            false
        }
    };
    if credential_exists {
        let result = match report.status {
            ProviderPayloadProbeStatus::Passed => {
                db::mark_provider_credential_probe_success(pg_pool, credential_id).await
            }
            ProviderPayloadProbeStatus::Failed => {
                let message = provider_error_message_for_persistence(&report.message);
                db::mark_provider_credential_probe_failure(pg_pool, credential_id, &message).await
            }
            ProviderPayloadProbeStatus::Unsupported => return,
        };
        if let Err(error) = result {
            warn!(
                provider_id,
                credential_id,
                error = %error,
                "failed to persist provider credential probe result"
            );
        }
        return;
    }

    if credential_id != crate::routing::config::provider_default_account_id(provider_id) {
        return;
    }
    let provider_exists = match db::get_provider_account(pg_pool, provider_id).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            warn!(
                provider_id,
                error = %error,
                "failed to inspect provider account before recording default probe result"
            );
            false
        }
    };
    if !provider_exists {
        return;
    }

    let result = match report.status {
        ProviderPayloadProbeStatus::Passed => {
            db::mark_provider_probe_success(pg_pool, provider_id).await
        }
        ProviderPayloadProbeStatus::Failed => {
            let message = provider_error_message_for_persistence(&report.message);
            db::mark_provider_probe_failure(pg_pool, provider_id, &message).await
        }
        ProviderPayloadProbeStatus::Unsupported => return,
    };
    if let Err(error) = result {
        warn!(
            provider_id,
            error = %error,
            "failed to persist provider default probe result"
        );
    }
}

fn is_runtime_managed_provider(provider_account_id: &str) -> bool {
    !provider_account_id.starts_with("cred:")
}

pub async fn clear_provider_credential_runtime_keys(
    redis_pool: &deadpool_redis::Pool,
    provider_credential_id: &str,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: usize = conn
        .del(&[
            crate::redis::keys::provider_credential_failure_count_key(provider_credential_id),
            crate::redis::keys::provider_credential_breaker_open_key(provider_credential_id),
            crate::redis::keys::provider_credential_quota_snapshot_key(provider_credential_id),
            crate::redis::keys::provider_credential_quota_lock_key(provider_credential_id),
        ])
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("clear provider credential runtime keys: {error}"))
        })?;
    Ok(())
}
