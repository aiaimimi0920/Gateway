use crate::error::GatewayError;
use crate::state::AppState;
use deadpool_redis::Pool as RedisPool;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tracing::{debug, warn};

use super::*;

#[cfg(test)]
mod cooldown_contract;

pub fn build_signal_key(
    stock_class_key: &str,
    metric_kind: &str,
    deficit_credential_count: Option<usize>,
    token_window_key: Option<&str>,
) -> String {
    let normalized = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        stock_class_key.trim().to_ascii_lowercase(),
        metric_kind.trim().to_ascii_lowercase(),
        deficit_credential_count.unwrap_or(0),
        token_window_key.unwrap_or("").trim().to_ascii_lowercase()
    );
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("credential_stock_signal_{}", hex::encode(hasher.finalize()))
}

pub async fn start_credential_stock_monitor_task(state: Arc<AppState>) {
    if !state.config.credential_stock_monitor_enabled {
        debug!("credential stock monitor disabled");
        return;
    }
    if state.pg_pool.is_none() {
        debug!("credential stock monitor disabled because PostgreSQL is not configured");
        return;
    }
    let interval_secs = state.config.credential_stock_monitor_interval_secs.max(15);
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    loop {
        interval.tick().await;
        if state.lifecycle.is_draining() {
            break;
        }
        match sweep_credential_stock_signals_once(state.as_ref()).await {
            Ok(result) => {
                debug!(
                    scanned = result.scanned_policy_count,
                    emitted = result.emitted_signal_count,
                    suppressed = result.suppressed_signal_count,
                    redis_errors = result.redis_error_count,
                    "credential stock signal sweep finished"
                );
            }
            Err(error) => {
                warn!(
                    error = %error.message,
                    "credential stock signal sweep failed"
                );
            }
        }
    }
}

pub async fn sweep_credential_stock_signals_once(
    state: &AppState,
) -> Result<CredentialStockSignalSweepResult, GatewayError> {
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))?;
    let report = get_credential_stock_status_report(
        pg_pool,
        &state.redis_pool,
        CredentialStockStatusFilters::default(),
    )
    .await?;
    let mut emitted_signal_count = 0usize;
    let mut suppressed_signal_count = 0usize;
    let mut redis_error_count = 0usize;
    let mut signals = Vec::new();

    for status in report.policies.iter().filter(|status| {
        status.needs_replenishment && status.policy.signal_enabled && status.policy.enabled
    }) {
        if signal_suppressed_by_cooldown(&status.policy, &status.signal_key) {
            suppressed_signal_count += 1;
            continue;
        }
        let mut event = insert_signal_event(
            pg_pool,
            &status.policy,
            &status.signal_key,
            status.severity,
            &status.signal_payload,
        )
        .await?;
        let mut published = false;
        match publish_signal_to_redis(&state.redis_pool, &event).await {
            Ok(()) => {
                let published_at = mark_signal_event_published(pg_pool, &event.id).await?;
                event.published_at = Some(published_at);
                published = true;
            }
            Err(error) => {
                warn!(
                    stock_class_key = %event.stock_class_key,
                    error = %error.message,
                    "publish credential stock signal failed"
                );
                redis_error_count += 1;
            }
        }
        if published {
            mark_policy_signal_sent(pg_pool, &status.policy.id, &status.signal_key).await?;
            emitted_signal_count += 1;
        }
        signals.push(event);
    }

    Ok(CredentialStockSignalSweepResult {
        scanned_policy_count: report.summary.enabled_policy_count,
        replenishment_policy_count: report.summary.needs_replenishment_policy_count,
        emitted_signal_count,
        suppressed_signal_count,
        redis_error_count,
        signals,
    })
}

fn signal_suppressed_by_cooldown(policy: &CredentialStockPolicyView, signal_key: &str) -> bool {
    if policy.last_signal_key.as_deref() != Some(signal_key) {
        return false;
    }
    let Some(last_signal_at) = policy.last_signal_at.as_deref().and_then(|value| {
        OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
    }) else {
        return false;
    };
    let cooldown = time::Duration::seconds(policy.signal_cooldown_secs.max(0));
    let now = OffsetDateTime::now_utc();
    // A positive cooldown beyond the calendar range cannot have expired.
    last_signal_at
        .checked_add(cooldown)
        .is_none_or(|deadline| now < deadline)
}

async fn publish_signal_to_redis(
    redis_pool: &RedisPool,
    event: &CredentialStockSignalEventView,
) -> Result<(), GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let payload = serde_json::to_string(&event.payload).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize credential stock signal payload: {error}"
        ))
    })?;
    let _: String = redis::cmd("XADD")
        .arg(event.stream.as_str())
        .arg("*")
        .arg("eventId")
        .arg(event.id.as_str())
        .arg("stockClassKey")
        .arg(event.stock_class_key.as_str())
        .arg("signalKey")
        .arg(event.signal_key.as_str())
        .arg("severity")
        .arg(format!("{:?}", event.severity).to_ascii_lowercase())
        .arg("payload")
        .arg(payload)
        .query_async(&mut conn)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("publish credential stock signal: {error}"))
        })?;
    Ok(())
}
