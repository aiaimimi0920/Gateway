//! Expiry-driven OAuth maintenance, independent of model probes and inference traffic.
mod persistence;
mod server_cache;
#[cfg(test)]
mod tests;
mod transport;

use crate::console::RouteConfigRuntime;
use crate::routing::config::{
    effective_refresh_lifetime_secs, future_rfc3339_after_secs, RouteConfigStore,
};
use crate::state::AppState;
use persistence::PendingRefresh;
use std::{collections::HashMap, sync::Arc, time::Duration};

type Pending = HashMap<(String, String), PendingRefresh>;

pub(crate) fn spawn(state: &Arc<AppState>) {
    let mut task = state.lifecycle.oauth_refresh_task.lock();
    if task.is_some() {
        return;
    }
    let state = Arc::clone(state);
    *task = Some(tokio::spawn(start_token_refresh_task(state, 60, 300)));
}

pub(crate) async fn join(state: &AppState) {
    let task = state.lifecycle.oauth_refresh_task.lock().take();
    if let Some(task) = task {
        if task.await.is_err() {
            tracing::error!("OAuth maintenance task did not exit cleanly");
        }
    }
}

/// Runs in both local and server modes. Local mode never touches the closed Redis pool.
pub async fn start_token_refresh_task(state: Arc<AppState>, check_secs: u64, margin_secs: u64) {
    let mut pending = Pending::new();
    tracing::info!(check_secs, margin_secs, "OAuth token refresh task started");
    loop {
        if state.shutdown.is_requested() {
            break;
        }
        refresh_once(
            &state.route_config,
            state.upstream_client.client(),
            state
                .local_runtime
                .as_ref()
                .and(state.route_config_runtime.as_deref()),
            state.local_runtime.is_none().then_some(&state.redis_pool),
            &mut pending,
            Duration::from_secs(margin_secs),
            Some(&state.shutdown),
        )
        .await;
        tokio::select! {
            _ = state.shutdown.wait() => break,
            _ = tokio::time::sleep(Duration::from_secs(check_secs.max(1))) => {}
        }
    }
    // Normal shutdown waits for the in-flight exchange/commit, then one final disk-only flush.
    if let Some(runtime) = state
        .local_runtime
        .as_ref()
        .and(state.route_config_runtime.as_deref())
    {
        for refresh in pending.values() {
            if !refresh.persist(runtime).await {
                tracing::error!(provider = %refresh.provider, cred_id = %refresh.credential,
                    "OAuth refreshed material could not be persisted before shutdown");
            }
        }
    } else if state.local_runtime.is_none() {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        for refresh in pending.values() {
            if !refresh.is_current(&state.route_config) {
                continue;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if !matches!(
                tokio::time::timeout(remaining, refresh.write_to(None, Some(&state.redis_pool)))
                    .await,
                Ok(true)
            ) {
                tracing::error!(cred_id = %refresh.credential, "OAuth cache material not persisted before shutdown");
            }
        }
    }
}

async fn refresh_once(
    store: &RouteConfigStore,
    http: &rquest::Client,
    local: Option<&RouteConfigRuntime>,
    redis: Option<&deadpool_redis::Pool>,
    pending: &mut Pending,
    margin: Duration,
    shutdown: Option<&crate::state::GatewayShutdownHandle>,
) {
    let keys: Vec<_> = pending.keys().cloned().collect();
    for key in keys {
        if shutdown.is_some_and(|s| s.is_requested()) {
            return;
        }
        if !pending[&key].is_current(store) || pending[&key].write_to(local, redis).await {
            pending.remove(&key);
        }
    }
    let targets = store.snapshot().oauth_refresh_targets();
    for key in targets {
        if shutdown.is_some_and(|s| s.is_requested()) {
            return;
        }
        // Each local commit can replace the provider snapshot. Re-pin before the next credential.
        let snapshot = store.snapshot();
        let Some((provider, cred)) = snapshot.oauth_refresh_target(&key.0, &key.1) else {
            continue;
        };
        if !cred.enabled || pending.contains_key(&key) {
            continue;
        }
        let Some(config) = &cred.refresh_config else {
            continue;
        };
        let Some(_guard) = config.try_refresh_lock() else {
            continue;
        };
        let Some((generation, refresh_token)) = config.refresh_snapshot(margin) else {
            continue;
        };
        let previous_access = config
            .runtime_override()
            .0
            .unwrap_or_else(|| cred.payload.api_key.clone());
        let result = transport::exchange(
            http,
            &config.refresh_endpoint,
            &refresh_token,
            &config.client_id,
        )
        .await;
        match result {
            Ok(mut token) => {
                let lifetime = effective_refresh_lifetime_secs(token.expires_in.unwrap_or(21600));
                token.expires_in = Some(lifetime);
                let expires_at = future_rfc3339_after_secs(lifetime);
                let original =
                    persistence::credential(snapshot.document(), &provider.id, &cred.id).cloned();
                let Some(original) = original else {
                    continue;
                };
                let refreshed = PendingRefresh {
                    provider: provider.id.clone(),
                    credential: cred.id.clone(),
                    original,
                    token,
                    expires_at,
                    previous_access,
                    previous_refresh: refresh_token,
                };
                if !refreshed.is_current(store) {
                    continue;
                }
                let applied = config.apply_refresh_if_active(
                    generation,
                    refreshed.token.access_token.clone(),
                    refreshed.token.refresh_token.clone(),
                    lifetime,
                    refreshed.expires_at.clone(),
                );
                // A routing-policy change retires the old owner, but need not discard
                // rotated material for the same authorization. The local commit CAS
                // retains the new policy and rechecks the original credential.
                if applied.is_none() && local.is_none() {
                    continue;
                }
                if !refreshed.write_to(local, redis).await {
                    pending.insert(key, refreshed);
                }
                tracing::info!(provider = %provider.id, cred_id = %cred.id,
                        expires_in_secs = lifetime, "OAuth token refreshed");
            }
            Err(error) => {
                config.record_refresh_failure(generation, error.terminal);
                tracing::warn!(provider = %provider.id, cred_id = %cred.id, code = error.code,
                        reauthorization_required = error.terminal, "OAuth token refresh failed");
            }
        }
    }
}
