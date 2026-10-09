use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::GatewayError;
use crate::state::AppState;

const SCHEDULER_TICK_SECS: u64 = 60;
mod order;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialProbeSweepSummary {
    pub scheduled_count: usize,
    pub due_count: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub unsupported_count: usize,
    pub skipped_count: usize,
}

pub async fn start_provider_credential_probe_scheduler(state: Arc<AppState>) {
    info!(
        interval_secs = SCHEDULER_TICK_SECS,
        "provider credential probe scheduler started"
    );
    let mut interval = tokio::time::interval(Duration::from_secs(SCHEDULER_TICK_SECS));
    loop {
        interval.tick().await;
        match sweep_scheduled_provider_credentials_once(state.as_ref()).await {
            Ok(summary) => debug!(
                scheduled = summary.scheduled_count,
                due = summary.due_count,
                passed = summary.passed_count,
                failed = summary.failed_count,
                unsupported = summary.unsupported_count,
                skipped = summary.skipped_count,
                "provider credential probe sweep completed"
            ),
            Err(error) => warn!(error = %error, "provider credential probe sweep failed"),
        }
    }
}

pub async fn sweep_scheduled_provider_credentials_once(
    state: &AppState,
) -> Result<ProviderCredentialProbeSweepSummary, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let mut targets = snapshot.scheduled_credential_probe_targets();
    let mut order = order::SweepOrder::new(state, targets.len());
    targets.rotate_left(order.offset());
    let mut summary = ProviderCredentialProbeSweepSummary {
        scheduled_count: targets.len(),
        ..ProviderCredentialProbeSweepSummary::default()
    };
    let mut budget =
        crate::provider_runtime::test_execution::RoundBudget::new(snapshot.revision().id());

    for scheduled in targets {
        // Never consume another account's interval after the shared round is exhausted or stale.
        if !budget.available() || !budget.matches(state) || summary.due_count >= 128 {
            break;
        }
        order.advance();
        if !scheduled.target.enabled {
            summary.skipped_count += 1;
            continue;
        }
        if !crate::provider_runtime::test_store::claim(
            state,
            &scheduled.target.provider_id,
            &scheduled.target.credential_id,
            scheduled.interval_minutes,
            scheduled.plan_id.as_deref(),
        )
        .await?
        {
            summary.skipped_count += 1;
            continue;
        }
        summary.due_count += 1;
        let policy = if let Some(id) = scheduled.plan_id.as_deref() {
            snapshot
                .named_test_plan(&scheduled.target.provider_id, id)
                .map(|plan| (plan.policy.clone(), format!("plan:{id}")))
        } else {
            snapshot.credential_test_policy(&scheduled.target)
        };
        if let Some((plan, source)) = policy {
            let mut result = crate::provider_runtime::test_execution::run(
                state,
                &scheduled.target,
                &plan,
                &source,
                "automatic",
                &mut budget,
            )
            .await;
            if let Some(assessment) = result.assessment.as_mut() {
                assessment.plan_id = scheduled.plan_id;
            }
            crate::provider_runtime::test_store::save(state, &result).await?;
            match result.status {
                crate::provider_runtime::ProviderPayloadProbeStatus::Passed => {
                    summary.passed_count += 1
                }
                crate::provider_runtime::ProviderPayloadProbeStatus::Failed => {
                    summary.failed_count += 1
                }
                crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported => {
                    summary.unsupported_count += 1
                }
            }
            continue;
        }
        if !budget.matches(state) || !budget.admit() {
            break;
        }
        let report = crate::provider_runtime::probe_provider_payload_for_console(
            state.upstream_client.client(),
            &scheduled.target.payload,
        )
        .await;
        crate::provider_runtime::record_provider_credential_probe_report(
            state,
            &scheduled.target.provider_id,
            &scheduled.target.credential_id,
            &report,
        )
        .await;
        match report.status {
            crate::provider_runtime::ProviderPayloadProbeStatus::Passed => {
                summary.passed_count += 1;
            }
            crate::provider_runtime::ProviderPayloadProbeStatus::Failed => {
                summary.failed_count += 1;
            }
            crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported => {
                summary.unsupported_count += 1;
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
fn schedule_slot_key(provider_id: &str, credential_id: &str) -> String {
    crate::provider_runtime::test_store::identity("default", provider_id, credential_id)
}

#[cfg(test)]
mod tests {
    use super::schedule_slot_key;

    #[test]
    fn schedule_slot_key_keeps_provider_and_credential_boundaries_unique() {
        assert_ne!(
            schedule_slot_key("abc", "default"),
            schedule_slot_key("xyz", "default")
        );
        assert_ne!(
            schedule_slot_key("provider:a", "b:c"),
            schedule_slot_key("provider", "a:b:c")
        );
    }
}
