//! Per-runtime bounded observations; weak worker ownership and configuration-fenced publication.
use super::SiteSnapshot;
use crate::{error::GatewayError, routing::candidate::RouteCandidate, state::AppState};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Debug, Default)]
pub(crate) struct SiteHealthRuntime {
    started: AtomicBool,
    observations: RwLock<HashMap<String, SiteSnapshot>>,
    gate: tokio::sync::Mutex<()>,
    wake: Arc<tokio::sync::Notify>,
}

pub(crate) fn schedule(state: &Arc<AppState>) {
    let worker = &state.lifecycle.site_health;
    if state.shutdown.is_requested() {
        return;
    }
    worker.wake.notify_one();
    // Start even without configured sites so additions through any configuration API are observed.
    if worker.started.load(Ordering::SeqCst) {
        return;
    }
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
            let targets: Vec<_> = state
                .route_config
                .snapshot()
                .site_health_providers()
                .filter_map(|p| {
                    p.site_health
                        .as_ref()
                        .map(|(c, b)| (p.id.clone(), b.clone(), c.refresh_interval_secs))
                })
                .collect();
            state
                .lifecycle
                .site_health
                .observations
                .write()
                .retain(|id, _| targets.iter().any(|(p, _, _)| id == p));
            for (id, binding, interval) in targets {
                let due = state
                    .lifecycle
                    .site_health
                    .observations
                    .read()
                    .get(&id)
                    .is_none_or(|s| {
                        s.binding != binding
                            || super::now().saturating_sub(s.checked_at) >= interval
                    });
                if due {
                    tokio::select! {_ = shutdown.wait()=>return, _=refresh(&state,&id,false)=>{}}
                }
            }
            drop(state);
            tokio::select! {_ = shutdown.wait()=>return, _=wake.notified()=>{}, _=tokio::time::sleep(Duration::from_secs(15))=>{}}
        }
    });
}

pub(crate) fn snapshot(state: &AppState, id: &str) -> Option<SiteSnapshot> {
    state
        .lifecycle
        .site_health
        .observations
        .read()
        .get(id)
        .cloned()
}

pub(crate) async fn refresh(
    state: &AppState,
    id: &str,
    manual: bool,
) -> Result<SiteSnapshot, GatewayError> {
    let _guard = state
        .lifecycle
        .site_health
        .gate
        .try_lock()
        .map_err(|_| GatewayError::conflict("Site health detection is busy"))?;
    let snapshot = state.route_config.snapshot();
    let provider = snapshot
        .site_health_providers()
        .find(|p| p.id == id)
        .ok_or_else(|| GatewayError::not_found("Site not found"))?;
    let (config, binding) = provider
        .site_health
        .as_ref()
        .ok_or_else(|| GatewayError::bad_request("Site health monitoring is disabled"))?
        .clone();
    let base = provider.payload.base_url.clone();
    if manual && snapshot_existing_recent(state, id, &binding) {
        return Err(GatewayError::rate_limited(
            "Wait 30 seconds between site health detections",
            30_000,
        ));
    }
    drop(snapshot);
    let report = super::discovery::discover(id, &base, &config)
        .await
        .unwrap_or_else(|_| SiteSnapshot {
            provider_id: id.into(),
            checked_at: super::now(),
            binding: binding.clone(),
            selected_template: None,
            models: Default::default(),
            probes: vec![super::ProbeResult {
                template: "auto".into(),
                endpoint: config.site_url.clone().unwrap_or(base.clone()),
                status: "network_policy_or_detection_error".into(),
                http_status: None,
                model_count: 0,
                models: Default::default(),
            }],
        });
    let current = state.route_config.snapshot();
    if !current
        .site_health_providers()
        .any(|p| p.id == id && p.site_health.as_ref().is_some_and(|(_, b)| *b == binding))
    {
        return Err(GatewayError::conflict(
            "Site configuration changed during detection",
        ));
    }
    state
        .lifecycle
        .site_health
        .observations
        .write()
        .insert(id.into(), report.clone());
    Ok(report)
}

fn snapshot_existing_recent(state: &AppState, id: &str, binding: &str) -> bool {
    state
        .lifecycle
        .site_health
        .observations
        .read()
        .get(id)
        .is_some_and(|s| s.binding == binding && super::now().saturating_sub(s.checked_at) < 30)
}

pub(crate) fn candidate_allowed(state: &AppState, candidate: &RouteCandidate, model: &str) -> bool {
    let route = state.route_config.snapshot();
    let Some(provider) = route
        .site_health_providers()
        .find(|p| p.id == candidate.provider_account_id)
    else {
        return true;
    };
    let Some((config, binding)) = &provider.site_health else {
        return true;
    };
    let observations = state.lifecycle.site_health.observations.read();
    super::allows(
        config,
        observations.get(&provider.id),
        binding,
        model,
        super::now(),
    )
}

pub(crate) fn entries(state: &AppState) -> serde_json::Value {
    let route = state.route_config.snapshot();
    serde_json::json!({"items":route.site_health_providers().filter_map(|p|p.site_health.as_ref().map(|(config,binding)|{
        let report=snapshot(state,&p.id).filter(|s|s.binding==*binding);
        serde_json::json!({"provider_id":p.id,"config":config,"snapshot":report})
    })).collect::<Vec<_>>()})
}
