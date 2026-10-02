use crate::error::GatewayError;
use crate::state::AppState;
use std::sync::Arc;

use super::*;

pub async fn start_credential_refill_notification_task(state: Arc<AppState>) {
    if !state.credential_pool_automation.refill_queue_enabled() {
        tracing::info!("credential refill queue is disabled");
        return;
    }
    loop {
        if let Err(error) = publish_notification_refill_demands_once(state.as_ref()).await {
            tracing::warn!(
                code = ?error.code,
                "credential refill notification sweep failed"
            );
        }
        tokio::time::sleep(
            state
                .credential_pool_automation
                .refill_notification_interval(),
        )
        .await;
    }
}

pub async fn publish_notification_refill_demands_once(
    state: &AppState,
) -> Result<usize, GatewayError> {
    crate::credential_pool_automation::cycle::settle_refill_cycles(state).await?;
    let demands = list_credential_refill_demands(state).await?;
    let mut published = 0usize;
    for demand in demands
        .into_iter()
        .filter(|demand| demand.notification_enabled && demand.needs_refill)
    {
        let result = create_task_from_demand(
            state,
            &demand,
            CredentialRefillTrigger::Notification,
            demand.deficit.min(MAX_REQUESTED_COUNT),
            // Outstanding-task deduplication reserves capacity. Failed tasks release it;
            // the next timed sweep may retry without a permanent revision-key tombstone.
            None,
        )
        .await?;
        published += usize::from(result.created);
    }
    Ok(published)
}
