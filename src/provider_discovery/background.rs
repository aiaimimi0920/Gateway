//! One durable-intent consumer per runtime; no browser-owned requests or secret queue.
use super::{
    job::{self, DiscoveryJob, DiscoveryJobStatus},
    CredentialDiscovery,
};
use crate::state::AppState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Weak,
};
use tokio::sync::Notify;

#[derive(Debug, Default)]
pub(crate) struct DiscoveryWorker {
    started: AtomicBool,
    wake: Arc<Notify>,
}

// Shared by manual discovery and background jobs. Waiting never holds the save request.
pub(crate) static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub(crate) fn schedule(state: &Arc<AppState>) {
    if state.route_config_runtime.is_none() || state.shutdown.is_requested() {
        return;
    }
    let worker = &state.lifecycle.discovery;
    if !worker.started.load(Ordering::SeqCst)
        && !job::requested(state.route_config.snapshot().document())
    {
        return;
    }
    worker.wake.notify_one();
    if worker.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let weak = Arc::downgrade(state);
    let wake = worker.wake.clone();
    let shutdown = state.shutdown.clone();
    tokio::spawn(run(weak, wake, shutdown));
}

struct Target {
    provider: String,
    credential: String,
    job: DiscoveryJob,
    base: String,
    key: String,
}

fn next(state: &AppState) -> Option<Target> {
    let snapshot = state.route_config.snapshot();
    for provider in &snapshot.document().providers {
        if !job::supported(provider) {
            continue;
        }
        for credential in &provider.credentials {
            let Some(job) = credential
                .discovery_job
                .as_ref()
                .filter(|j| j.status == DiscoveryJobStatus::Pending)
            else {
                continue;
            };
            let Some(id) = &credential.id else {
                continue;
            };
            let Some(target) = snapshot.select_credential_probe_target(id) else {
                continue;
            };
            return Some(Target {
                provider: provider.id.clone(),
                credential: id.clone(),
                job: job.clone(),
                base: credential
                    .base_url
                    .as_ref()
                    .unwrap_or(&provider.base_url)
                    .clone(),
                key: target.payload.api_key,
            });
        }
    }
    None
}

async fn run(
    state: Weak<AppState>,
    wake: Arc<Notify>,
    shutdown: crate::state::GatewayShutdownHandle,
) {
    // One bounded completed-result slot survives transient write failures in this process.
    let mut completed: Option<(Target, Option<CredentialDiscovery>)> = None;
    loop {
        tokio::select! {
            _ = shutdown.wait() => return,
            _ = wake.notified() => {},
        }
        loop {
            let Some(state) = state.upgrade() else {
                return;
            };
            let (target, result) = if let Some(completed) = completed.take() {
                completed
            } else {
                let Some(target) = next(&state) else {
                    break;
                };
                let slot = tokio::select! {
                    _ = shutdown.wait() => return,
                    slot = SLOTS.acquire() => slot,
                };
                let Ok(slot) = slot else {
                    return;
                };
                let result = tokio::select! {
                    _ = shutdown.wait() => return,
                    result = discover_current(&state, &target, &wake) => result,
                };
                drop(slot);
                (target, result)
            };
            if !save(&state, &target, result.as_ref()).await {
                // Retry only writeback on the next wake, never paid I/O for this result.
                completed = Some((target, result));
                tracing::warn!("Background discovery writeback deferred; pending intent retained");
                break;
            }
            // Let an admitted persistence transaction finish before honoring shutdown.
            if shutdown.is_requested() {
                return;
            }
        }
    }
}

fn current(state: &AppState, target: &Target) -> bool {
    let snapshot = state.route_config.snapshot();
    let Some(provider) = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == target.provider)
    else {
        return false;
    };
    let Some(credential) = provider
        .credentials
        .iter()
        .find(|c| c.id.as_deref() == Some(&target.credential))
    else {
        return false;
    };
    let Some(payload) = snapshot.select_credential_probe_target(&target.credential) else {
        return false;
    };
    job::supported(provider)
        && credential.discovery_job.as_ref() == Some(&target.job)
        && super::binding(
            credential.base_url.as_deref().unwrap_or(&provider.base_url),
            &payload.payload.api_key,
        ) == super::binding(&target.base, &target.key)
}

async fn discover_current(
    state: &AppState,
    target: &Target,
    wake: &Notify,
) -> Option<CredentialDiscovery> {
    if !current(state, target) {
        return None;
    }
    let request = super::discover(&target.base, &target.key);
    tokio::pin!(request);
    loop {
        tokio::select! {
            result = &mut request => return result.ok(),
            _ = wake.notified() => if !current(state, target) { return None; },
        }
    }
}

async fn save(state: &AppState, target: &Target, result: Option<&CredentialDiscovery>) -> bool {
    let Some(runtime) = &state.route_config_runtime else {
        return false;
    };
    for _ in 0..3 {
        let snapshot = state.route_config.snapshot();
        let Some(current) = snapshot.select_credential_probe_target(&target.credential) else {
            return true;
        };
        // Raw source address (not discovery-adjusted API base) is authoritative.
        let Some(provider) = snapshot
            .document()
            .providers
            .iter()
            .find(|p| p.id == target.provider)
        else {
            return true;
        };
        let Some(credential) = provider
            .credentials
            .iter()
            .find(|c| c.id.as_deref() == Some(&target.credential))
        else {
            return true;
        };
        let base = credential.base_url.as_deref().unwrap_or(&provider.base_url);
        let unchanged = super::binding(base, &current.payload.api_key)
            == super::binding(&target.base, &target.key);
        let mut document = snapshot.document().clone();
        if !job::merge(
            &mut document,
            &target.provider,
            &target.credential,
            &target.job,
            if unchanged { result } else { None },
        ) {
            return true;
        }
        match runtime
            .commit_automation_document(
                snapshot.revision().id(),
                document,
                Some("Update account protocol discovery".into()),
            )
            .await
        {
            Ok(_) => return true,
            Err(error) if error.code() == "console_revision_conflict" => continue,
            Err(_) => return false,
        }
    }
    false
}
