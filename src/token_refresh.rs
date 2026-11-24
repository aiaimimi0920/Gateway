//! Expiry-driven OAuth maintenance, independent of model probes and inference traffic.
mod accio;
mod cache_target;
mod dim;
mod grant;
mod grok;
mod kiro;
mod lobsterai;
mod nous;
mod persistence;
mod projection;
mod quick;
mod received;
pub(crate) mod server_cache;
#[cfg(test)]
mod tests;
mod transport;
mod vertex;

use crate::console::RouteConfigRuntime;
use crate::routing::config::{
    effective_refresh_lifetime_secs, future_rfc3339_after_secs, RouteConfigStore,
};
use crate::state::AppState;
pub(crate) use persistence::PendingRefresh;
pub(crate) use projection::is_refresh_projection;
use received::PendingWrite;
use std::{collections::HashMap, sync::Arc, time::Duration};

type Pending = HashMap<(String, String), PendingWrite>;

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
            state.route_config_runtime.as_deref(),
            state.local_runtime.is_none().then_some(&state.redis_pool),
            &mut pending,
            Duration::from_secs(margin_secs),
            Some(&state.shutdown),
        )
        .await;
        #[cfg(feature = "line-lumalabs-web-reverse-api")]
        crate::luma_web_session::maintenance::check(&state).await;
        tokio::select! {
            _ = state.shutdown.wait() => break,
            _ = state.lifecycle.oauth_refresh_wake.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(check_secs.max(1))) => {}
        }
    }
    // Normal shutdown waits for the in-flight exchange/commit, then one persistence-only flush.
    if let Some(store) = state.upstream_client.luma_web_sessions.get() {
        store.flush_pending().await;
    }
    if let Some(runtime) = state
        .local_runtime
        .as_ref()
        .and(state.route_config_runtime.as_deref())
    {
        for refresh in pending.values_mut() {
            if !refresh.write_to(Some(runtime), None).await {
                let (provider, credential) = refresh.identity();
                tracing::error!(
                    provider,
                    cred_id = credential,
                    "OAuth refreshed material could not be persisted before shutdown"
                );
            }
        }
    } else if state.local_runtime.is_none() {
        let runtime = state.route_config_runtime.as_deref();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        for refresh in pending.values_mut() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if !matches!(
                tokio::time::timeout(
                    remaining,
                    refresh.write_to(runtime, Some(&state.redis_pool))
                )
                .await,
                Ok(true)
            ) {
                tracing::error!(
                    cred_id = refresh.identity().1,
                    "OAuth cache material not persisted before shutdown"
                );
            }
        }
    }
}

async fn refresh_once(
    store: &RouteConfigStore,
    http: &rquest::Client,
    runtime: Option<&RouteConfigRuntime>,
    redis: Option<&deadpool_redis::Pool>,
    pending: &mut Pending,
    margin: Duration,
    shutdown: Option<&crate::state::GatewayShutdownHandle>,
) {
    if shutdown.is_some_and(|handle| handle.is_requested()) {
        return;
    }
    // Successful production startup supplies this owner in every storage/runtime role.
    let Some(runtime) = runtime else { return };
    let keys: Vec<_> = pending.keys().cloned().collect();
    for key in keys {
        if shutdown.is_some_and(|s| s.is_requested()) {
            return;
        }
        if pending
            .get_mut(&key)
            .unwrap()
            .write_to(Some(runtime), redis)
            .await
        {
            pending.remove(&key);
        }
    }
    if runtime.coordinator().is_none() {
        // Result storage obligations are flushed even while authority recovery is unavailable.
        if !persistence::prepare_worker_authority(runtime).await {
            return;
        }
    }
    if redis.is_some() {
        if !matches!(
            tokio::time::timeout(
                persistence::authority_budget(),
                runtime.repair_refresh_cache()
            )
            .await,
            Ok(Ok(()))
        ) {
            tracing::warn!("OAuth cache publication recovery pending; new exchanges deferred");
            return;
        }
    }
    let targets = store.snapshot().oauth_refresh_targets();
    for key in targets {
        if shutdown.is_some_and(|s| s.is_requested()) {
            return;
        }
        // Each authority commit can replace the provider snapshot. Re-pin before the next credential.
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
        if config.lobster_dispatch_generation().is_some() {
            // The current Server cache owner requires an expiry; never consume a Lobster grant there.
            if redis.is_some()
                || !lobsterai::recover(runtime, &snapshot, &provider.id, &cred.id, config).await
            {
                continue;
            }
        }

        let Some(attempt) = config.begin_refresh(margin, &cred.payload.api_key) else {
            continue;
        };
        let prepared = match grant::Prepared::new(http, &attempt, &cred.payload).await {
            Ok(prepared) => prepared,
            Err(error) => {
                attempt.record_failure(error.terminal);
                tracing::warn!(provider = %provider.id, cred_id = %cred.id, code = error.code,
                    "OAuth refresh preparation failed before grant reservation");
                continue;
            }
        };
        let Some(original) =
            persistence::credential(snapshot.document(), &provider.id, &cred.id).cloned()
        else {
            continue;
        };
        let Some((request, reservation)) =
            grant::reserve(runtime, &snapshot, &provider.id, &cred.id, &attempt).await
        else {
            continue;
        };
        use crate::console::refresh_grants::GrantReservation;
        let (record, result, saved_expiry) = match reservation {
            GrantReservation::Reserved(record) => (record, prepared.execute(http).await, None),
            GrantReservation::Existing(record) => {
                if let Some(tokens) = lobsterai::replay(&record, &request) {
                    (record, Ok(grant::Outcome::Lobster(tokens)), None)
                } else if let Some((token, expiry)) = grant::replay(&record, &request) {
                    (record, Ok(grant::Outcome::Token(token)), Some(expiry))
                } else if let Some(crate::console::refresh_grants::GrantReceipt::NousPortal {
                    tokens,
                }) = record
                    .receipt_for(&request)
                    .filter(|_| attempt.nous_portal.is_some())
                {
                    let tokens = tokens.clone();
                    (record, Ok(grant::Outcome::Nous(tokens)), None)
                } else {
                    tracing::warn!(provider = %provider.id, cred_id = %cred.id,
                        "OAuth grant has permanent evidence; automatic re-exchange is blocked");
                    if attempt.lobsterai.is_some() {
                        attempt.record_failure(true);
                    }
                    continue;
                }
            }
            GrantReservation::Blocked | GrantReservation::Stale | GrantReservation::Contended => {
                continue;
            }
        };
        match result {
            Ok(grant::Outcome::Lobster(tokens)) => {
                let mut received = PendingWrite::received_lobster(
                    config.clone(),
                    attempt,
                    original,
                    request,
                    record,
                    tokens,
                );
                if !received.write_to(Some(runtime), redis).await {
                    pending.insert(key, received);
                }
            }
            Ok(grant::Outcome::Nous(tokens)) => {
                let mut received = PendingWrite::received(
                    config.clone(),
                    attempt,
                    original,
                    request,
                    record,
                    tokens,
                );
                if !received.write_to(Some(runtime), redis).await {
                    pending.insert(key, received);
                }
            }
            Ok(grant::Outcome::Token(mut token)) => {
                let lifetime = effective_refresh_lifetime_secs(token.expires_in.unwrap_or(21600));
                token.expires_in = Some(lifetime);
                let expires_at = saved_expiry
                    .or_else(|| token.absolute_expiry.clone())
                    .unwrap_or_else(|| future_rfc3339_after_secs(lifetime));
                let mut refreshed = PendingWrite::Result(Box::new(PendingRefresh::received(
                    config.clone(),
                    attempt,
                    original,
                    request,
                    record,
                    token,
                    expires_at,
                )));
                if !refreshed.write_to(Some(runtime), redis).await {
                    pending.insert(key, refreshed);
                }
                tracing::info!(provider = %provider.id, cred_id = %cred.id,
                        expires_in_secs = lifetime, "OAuth refresh result accepted");
            }
            Err(error) => {
                attempt.record_failure(error.terminal);
                tracing::warn!(provider = %provider.id, cred_id = %cred.id, code = error.code,
                        reauthorization_required = error.terminal, grant_replay_blocked = true,
                        "OAuth token refresh failed");
            }
        }
    }
}
