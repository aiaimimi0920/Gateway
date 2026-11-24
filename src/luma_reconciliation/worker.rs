//! One bounded GET at a time; stored retry deadlines survive termination and process races.
use super::*;
use std::{sync::Arc, time::Duration};

pub(crate) fn spawn(state: &Arc<AppState>) {
    #[cfg(feature = "line-lumalabs-agents-official-api")]
    if let Some(store) = store::Store::from_state(state) {
        let weak = Arc::downgrade(state);
        let stop = state.shutdown.clone();
        state.lifecycle.background_tasks.spawn(crate::runtime::background_tasks::Worker::LumaReconciliation,
            &state.shutdown, async move {
                loop {
                    tokio::select! {
                        _ = stop.wait() => break,
                        _ = tokio::time::sleep(Duration::from_secs(60)) => {},
                    }
                    let Some(owned) = weak.upgrade() else { break };
                    tokio::select! {
                        _ = stop.wait() => break,
                        result = tick(&owned, &store, now()) => {
                            if let Err(error) = result { tracing::warn!(code = ?error.code, "Luma reconciliation pass failed"); }
                        }
                    }
                }
            });
    }
    #[cfg(not(feature = "line-lumalabs-agents-official-api"))]
    let _ = state;
}

pub(super) async fn tick(
    state: &AppState,
    store: &store::Store,
    time: i64,
) -> Result<(), GatewayError> {
    for (id, previous) in store.due(time).await? {
        let time = time.max(now());
        let mut claimed = previous.clone();
        let attempts = previous["lumaReconciliation"]["attempts"]
            .as_u64()
            .unwrap_or(12);
        claimed["lumaReconciliation"]["nextAt"] = json!(time + 60);
        claimed["lumaReconciliation"]["attempts"] = json!(attempts.saturating_add(1));
        if !store.update(&id, &previous, &claimed).await? {
            continue;
        }
        let mut result = claimed.clone();
        let facts = &previous["lumaAgentsGeneration"];
        if terminal(facts) && facts["usage"]["credits_charged"].is_number() {
            result["lumaReconciliation"]["state"] = json!("settled");
        } else if attempts >= 12 {
            result["lumaReconciliation"]["state"] = json!("exhausted");
        } else {
            match lookup(state, &id, &previous, facts).await {
                Ok(Some(latest))
                    if ["id", "model", "type"]
                        .iter()
                        .all(|key| latest[*key] == facts[*key]) =>
                {
                    result["lumaReconciliation"]["observedState"] = latest["state"].clone();
                    result["lumaReconciliation"]["observedAt"] = json!(time);
                    if terminal(&latest) && latest["usage"]["credits_charged"].is_number() {
                        result["lumaAgentsGeneration"]["usage"]["credits_charged"] =
                            latest["usage"]["credits_charged"].clone();
                        result["lumaReconciliation"]["state"] = json!("settled");
                    }
                }
                Ok(_) => result["lumaReconciliation"]["state"] = json!("blocked"),
                Err(error) if matches!(error.http_status, Some(401 | 403)) => {
                    result["lumaReconciliation"]["state"] = json!("blocked")
                }
                Err(_) => {}
            }
        }
        if result["lumaReconciliation"]["state"] == "pending" {
            if attempts + 1 >= 12 {
                result["lumaReconciliation"]["state"] = json!("exhausted");
            }
            result["lumaReconciliation"]["nextAt"] = json!(time + (60i64 * (1 << attempts.min(6))));
        }
        store.update(&id, &claimed, &result).await?;
    }
    Ok(())
}

fn terminal(facts: &Value) -> bool {
    matches!(facts["state"].as_str(), Some("completed" | "failed"))
}

async fn lookup(
    state: &AppState,
    audit_id: &str,
    trace: &Value,
    facts: &Value,
) -> Result<Option<Value>, GatewayError> {
    let journal = &trace["lumaReconciliation"];
    let binding = &journal["binding"];
    let target = match journal["version"].as_u64() {
        Some(1) => route_target(state, binding),
        Some(2) => match serde_json::from_value::<hosted::Binding>(binding.clone()) {
            Ok(binding) => binding.target(state, audit_id).await?,
            Err(_) => None,
        },
        _ => None,
    };
    let Some(payload) = target else {
        return Ok(None);
    };
    crate::upstream::luma_agents::task_lookup::execute(
        &state.upstream_client.http,
        &payload,
        facts["id"].as_str().unwrap_or(""),
    )
    .await
    .map(Some)
}

fn route_target(state: &AppState, binding: &Value) -> Option<ProviderAccountPayload> {
    let binding: RouteBinding = serde_json::from_value(binding.clone()).ok()?;
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != binding.revision {
        return None;
    }
    snapshot
        .credential_probe_targets_for_provider(&binding.provider_id)?
        .into_iter()
        .find(|target| {
            target.credential_id == binding.credential_id
                && target.enabled
                && target.payload.canonical_adapter() == crate::protocol::luma_agents::ADAPTER
        })
        .map(|target| target.payload)
}
