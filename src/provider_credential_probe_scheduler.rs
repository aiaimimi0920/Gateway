use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tracing::{debug, info, warn};

use crate::error::GatewayError;
use crate::state::AppState;

const SCHEDULER_TICK_SECS: u64 = 60;

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
    let targets = state
        .route_config
        .snapshot()
        .scheduled_credential_probe_targets();
    let mut summary = ProviderCredentialProbeSweepSummary {
        scheduled_count: targets.len(),
        ..ProviderCredentialProbeSweepSummary::default()
    };

    for scheduled in targets {
        if !scheduled.target.enabled {
            summary.skipped_count += 1;
            continue;
        }
        if !claim_schedule_slot(
            &state.redis_pool,
            &scheduled.target.provider_id,
            &scheduled.target.credential_id,
            scheduled.interval_minutes,
        )
        .await?
        {
            summary.skipped_count += 1;
            continue;
        }
        summary.due_count += 1;
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

async fn claim_schedule_slot(
    redis_pool: &deadpool_redis::Pool,
    provider_id: &str,
    credential_id: &str,
    interval_minutes: u64,
) -> Result<bool, GatewayError> {
    let mut connection = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let key = schedule_slot_key(provider_id, credential_id);
    let ttl_secs = interval_minutes.clamp(1, 10_080).saturating_mul(60);
    let acquired: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(ttl_secs)
        .query_async(&mut connection)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!(
                "claim scheduled provider credential probe slot: {error}"
            ))
        })?;
    Ok(acquired.is_some())
}

fn schedule_slot_key(provider_id: &str, credential_id: &str) -> String {
    format!(
        "gw:scheduled-credential-probe:{}:{provider_id}:{}:{credential_id}",
        provider_id.len(),
        credential_id.len()
    )
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
