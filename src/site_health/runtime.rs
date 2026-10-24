//! Per-runtime bounded observations; weak worker ownership and configuration-fenced publication.
mod worker;
use super::{CompiledPlan, SiteSnapshot};
use crate::{error::GatewayError, state::AppState};
use parking_lot::RwLock;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};
pub(crate) use worker::schedule;

type ObservationKey = (String, Option<String>);

#[derive(Debug, Default)]
pub(crate) struct SiteHealthRuntime {
    started: AtomicBool,
    observations: RwLock<HashMap<ObservationKey, SiteSnapshot>>,
    gate: tokio::sync::Mutex<()>,
    wake: Arc<tokio::sync::Notify>,
}

fn snapshot(state: &AppState, id: &str, profile: Option<&str>) -> Option<SiteSnapshot> {
    state
        .lifecycle
        .site_health
        .observations
        .read()
        .get(&(id.into(), profile.map(str::to_owned)))
        .cloned()
}

pub(crate) async fn refresh(
    state: &AppState,
    id: &str,
    manual: bool,
) -> Result<SiteSnapshot, GatewayError> {
    refresh_plan(state, id, None, manual).await
}

pub(crate) async fn refresh_plan(
    state: &AppState,
    id: &str,
    profile: Option<&str>,
    manual: bool,
) -> Result<SiteSnapshot, GatewayError> {
    let _guard = state
        .lifecycle
        .site_health
        .gate
        .try_lock()
        .map_err(|_| GatewayError::conflict("Site health detection is busy"))?;
    let route = state.route_config.snapshot();
    let provider = route
        .site_health_providers()
        .find(|p| p.id == id)
        .ok_or_else(|| GatewayError::not_found("Site not found"))?;
    let plan = provider
        .site_health
        .as_ref()
        .ok_or_else(|| GatewayError::bad_request("Site health monitoring is disabled"))?
        .plans
        .iter()
        .find(|p| p.id.as_deref() == profile)
        .ok_or_else(|| GatewayError::bad_request("Select a configured site health plan"))?
        .clone();
    let (config, binding) = (&plan.policy, &plan.binding);
    if !config.is_enabled() {
        return Err(GatewayError::bad_request("Site health plan is disabled"));
    }
    let base = provider.payload.base_url.clone();
    // Plan edits must not bypass the request-rate guard for this stable identity.
    if manual
        && snapshot(state, id, profile)
            .is_some_and(|s| super::now().saturating_sub(s.checked_at) < 30)
    {
        return Err(GatewayError::rate_limited(
            "Wait 30 seconds between site health detections",
            30_000,
        ));
    }
    drop(route);
    // Drop the pending network future when this plan is paused/edited, not another plan.
    let discovered = tokio::select! {
        result = super::discovery::discover(id, &base, config) => result,
        _ = wait_plan_changed(state, id, &plan) => return Err(GatewayError::conflict("Site configuration changed during detection")),
    };
    let mut report = discovered.unwrap_or_else(|_| SiteSnapshot {
        provider_id: id.into(),
        profile_id: plan.id.clone(),
        checked_at: super::now(),
        binding: binding.clone(),
        selected_template: None,
        models: Default::default(),
        probes: vec![super::ProbeResult {
            observations: Vec::new(),
            template: config.adapter.id().into(),
            endpoint: config.site_url.clone().unwrap_or(base.clone()),
            status: "network_policy_or_detection_error".into(),
            http_status: None,
            model_count: 0,
            models: Default::default(),
            coverage: None,
        }],
    });
    report.binding = binding.clone();
    report.profile_id = plan.id.clone();
    let current = state.route_config.snapshot();
    if !current.site_health_providers().any(|p| {
        p.id == id
            && p.site_health.as_ref().is_some_and(|c| {
                c.plans
                    .iter()
                    .any(|p| p.id == plan.id && p.binding == *binding)
            })
    }) {
        return Err(GatewayError::conflict(
            "Site configuration changed during detection",
        ));
    }
    let mut observations = state.lifecycle.site_health.observations.write();
    // Manual refreshes also prune retired identities, even if the worker was never
    // scheduled. The cache stays bounded by the 64 actual compiled-plan budget.
    observations.retain(|(id, profile), _| {
        current.site_health_providers().any(|p| {
            p.id == *id
                && p.site_health
                    .as_ref()
                    .is_some_and(|c| c.plans.iter().any(|p| p.id == *profile))
        })
    });
    observations.insert((id.into(), plan.id), report.clone());
    Ok(report)
}

pub(crate) fn subject_allowed(
    state: &AppState,
    route: &crate::routing::config::RouteConfigSnapshot,
    provider_id: &str,
    credential: Option<&str>,
    model: &str,
) -> bool {
    let Some(provider) = route.site_health_providers().find(|p| p.id == provider_id) else {
        return true;
    };
    let Some(config) = &provider.site_health else {
        return true;
    };
    let observations = state.lifecycle.site_health.observations.read();
    let default = format!("{provider_id}::default");
    let subject = credential.unwrap_or(&default);
    // Membership can overlap: all applicable plans must permit the model. An unbound
    // group has no monitor restriction; it never inherits another group's result.
    config
        .plans
        .iter()
        .filter(|p| p.policy.is_enabled() && plan_applies(p, route, subject))
        .all(|p| {
            super::allows(
                &p.policy,
                observations.get(&(provider.id.clone(), p.id.clone())),
                &p.binding,
                model,
                super::now(),
            )
        })
}

fn plan_applies(
    plan: &CompiledPlan,
    route: &crate::routing::config::RouteConfigSnapshot,
    credential: &str,
) -> bool {
    plan.account_group_ids.is_empty()
        || route.effective_account_groups().iter().any(|g| {
            plan.account_group_ids.contains(&g.id)
                && g.provider_credential_ids.iter().any(|id| id == credential)
        })
}

pub(crate) fn entries(state: &AppState) -> serde_json::Value {
    let route = state.route_config.snapshot();
    serde_json::json!({"items":route.site_health_providers().filter_map(|p|p.site_health.as_ref().map(|c|{
        let reports:Vec<_>=c.plans.iter().map(|plan| {
            let report=snapshot(state,&p.id,plan.id.as_deref()).filter(|s|plan.policy.is_enabled() && s.binding==plan.binding);
            serde_json::json!({"profile_id":plan.id,"enabled":plan.policy.is_enabled(),"account_group_ids":plan.account_group_ids,"snapshot":report})
        }).collect();
        let legacy=reports.iter().find(|p|p["profile_id"].is_null()).map(|p|p["snapshot"].clone());
        serde_json::json!({"provider_id":p.id,"config":c.config.redacted(),"snapshot":legacy,"profiles":reports})
    })).collect::<Vec<_>>(), "model_circuits":state.lifecycle.model_circuits.reports()})
}

async fn wait_plan_changed(state: &AppState, id: &str, plan: &CompiledPlan) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let current = state.route_config.snapshot();
        if !current.site_health_providers().any(|p| {
            p.id == id
                && p.site_health.as_ref().is_some_and(|c| {
                    c.plans.iter().any(|p| {
                        p.id == plan.id && p.policy.is_enabled() && p.binding == plan.binding
                    })
                })
        }) {
            return;
        }
    }
}
