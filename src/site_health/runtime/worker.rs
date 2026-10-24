//! A single weakly-owned worker schedules actual plans, not each bound credential group.
use super::*;
use std::{sync::atomic::Ordering, time::Duration};

pub(crate) fn schedule(state: &Arc<AppState>) {
    let worker = &state.lifecycle.site_health;
    if state.shutdown.is_requested() {
        return;
    }
    worker.wake.notify_one();
    if worker.started.swap(true, Ordering::SeqCst) {
        return;
    }
    let weak = Arc::downgrade(state);
    let shutdown = state.shutdown.clone();
    let wake = worker.wake.clone();
    tokio::spawn(async move {
        loop {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let route = state.route_config.snapshot();
            let targets: Vec<_> = route
                .site_health_providers()
                .flat_map(|p| {
                    p.site_health
                        .iter()
                        .flat_map(|c| c.plans.iter())
                        .map(|plan| {
                            (
                                (p.id.clone(), plan.id.clone()),
                                plan.binding.clone(),
                                plan.policy.refresh_interval_secs,
                                plan.policy.is_enabled(),
                            )
                        })
                })
                .collect();
            drop(route);
            state
                .lifecycle
                .site_health
                .observations
                .write()
                .retain(|key, _| targets.iter().any(|(k, _, _, _)| key == k));
            for ((id, profile), binding, interval, enabled) in targets {
                // Keep the last timestamp for paused identities: pause/resume must not
                // bypass manual-refresh throttling. Entries hide paused snapshots.
                if !enabled {
                    continue;
                }
                let due = snapshot(&state, &id, profile.as_deref()).is_none_or(|s| {
                    s.binding != binding
                        || super::super::now().saturating_sub(s.checked_at) >= interval
                });
                if due {
                    tokio::select! { _=shutdown.wait()=>return, _=refresh_plan(&state, &id, profile.as_deref(), false)=>{} }
                }
            }
            drop(state);
            tokio::select! { _=shutdown.wait()=>return, _=wake.notified()=>{}, _=tokio::time::sleep(Duration::from_secs(15))=>{} }
        }
    });
}
