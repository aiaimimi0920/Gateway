use std::sync::Arc;

use tracing::warn;

use super::probe_lock::{acquire_provider_probe_lock, release_provider_probe_lock};
use super::{
    probe_provider_account_payload, provider_error_message_for_persistence, ProviderProbeOutcome,
};
use crate::db;
use crate::error::GatewayError;
use crate::provider_quota;
use crate::state::AppState;

pub async fn probe_provider_account_for_management(
    state: &Arc<AppState>,
    provider_account_id: &str,
) -> Result<ProviderProbeOutcome, GatewayError> {
    let provider_account_id = provider_account_id.trim();
    let pg_pool = required_pg_pool(state.as_ref())?;
    let provider_account = db::get_provider_account(pg_pool, provider_account_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
    let payload = crate::routing::candidate::deserialize_provider_payload(
        &provider_account.payload,
        Some(&provider_account.adapter),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for probe {}: {error}",
            provider_account.id
        ))
    })?;
    let lock = acquire_provider_probe_lock(&state.redis_pool, provider_account_id).await?;

    let outcome = match probe_provider_account_payload(state.as_ref(), &provider_account).await {
        Ok(()) => {
            db::clear_provider_runtime_keys(&state.redis_pool, provider_account_id).await?;
            let provider_quota = provider_quota::refresh_provider_quota_snapshot(
                &state.redis_pool,
                state.config.upstream_timeout_secs,
                provider_account_id,
                &payload,
            )
            .await
            .ok()
            .flatten();
            ProviderProbeOutcome {
                ok: true,
                provider_account: db::mark_provider_probe_success(pg_pool, provider_account_id)
                    .await?,
                error_message: None,
                provider_quota,
            }
        }
        Err(error) => {
            let message = provider_error_message_for_persistence(&error.message);
            ProviderProbeOutcome {
                ok: false,
                provider_account: db::mark_provider_probe_failure(
                    pg_pool,
                    provider_account_id,
                    &message,
                )
                .await?,
                error_message: Some(message),
                provider_quota: provider_quota::read_cached_provider_quota_snapshot(
                    &state.redis_pool,
                    provider_account_id,
                )
                .await
                .ok()
                .flatten(),
            }
        }
    };

    let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
    Ok(outcome)
}

pub async fn sweep_cooling_provider_accounts(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<Vec<ProviderProbeOutcome>, GatewayError> {
    let pg_pool = required_pg_pool(state.as_ref())?;
    let mut outcomes = Vec::new();
    let provider_ids = db::list_expired_cooling_provider_account_ids(pg_pool, limit).await?;

    for provider_account_id in provider_ids {
        let now = now_rfc3339();
        let lock = match acquire_provider_probe_lock(&state.redis_pool, &provider_account_id).await
        {
            Ok(lock) => lock,
            Err(error) if error.code.as_deref() == Some("conflict") => continue,
            Err(error) => return Err(error),
        };

        let fresh = db::get_provider_account(pg_pool, &provider_account_id)
            .await?
            .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))?;
        if fresh.status != "cooling" {
            let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
            continue;
        }
        if fresh
            .cooldown_until
            .as_ref()
            .is_some_and(|value| value.as_str() > now.as_str())
        {
            let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
            continue;
        }

        let outcome = match probe_provider_account_payload(state.as_ref(), &fresh).await {
            Ok(()) => {
                db::clear_provider_runtime_keys(&state.redis_pool, &provider_account_id).await?;
                let payload = crate::routing::candidate::deserialize_provider_payload(
                    &fresh.payload,
                    Some(&fresh.adapter),
                )
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "deserialize provider payload for quota refresh {}: {error}",
                        provider_account_id
                    ))
                })?;
                let provider_quota = provider_quota::refresh_provider_quota_snapshot(
                    &state.redis_pool,
                    state.config.upstream_timeout_secs,
                    &provider_account_id,
                    &payload,
                )
                .await
                .ok()
                .flatten();
                ProviderProbeOutcome {
                    ok: true,
                    provider_account: db::mark_provider_probe_success(
                        pg_pool,
                        &provider_account_id,
                    )
                    .await?,
                    error_message: None,
                    provider_quota,
                }
            }
            Err(error) => {
                let message = provider_error_message_for_persistence(&error.message);
                ProviderProbeOutcome {
                    ok: false,
                    provider_account: db::mark_provider_cooling_retry_failure(
                        pg_pool,
                        &provider_account_id,
                        &message,
                    )
                    .await?,
                    error_message: Some(message),
                    provider_quota: provider_quota::read_cached_provider_quota_snapshot(
                        &state.redis_pool,
                        &provider_account_id,
                    )
                    .await
                    .ok()
                    .flatten(),
                }
            }
        };

        let _ = release_provider_probe_lock(&state.redis_pool, &lock).await;
        outcomes.push(outcome);
    }

    Ok(outcomes)
}

pub async fn sweep_cooling_provider_accounts_best_effort(state: &Arc<AppState>, limit: i64) {
    if let Err(error) = sweep_cooling_provider_accounts(state, limit).await {
        warn!(error = %error, "cooling provider sweep failed");
    }
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}

fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}
