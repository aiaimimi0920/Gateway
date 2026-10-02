//! Settle durable refill hysteresis after health verification reaches the goal.
use crate::{error::GatewayError, state::AppState};

pub(crate) async fn settle_refill_cycles(state: &AppState) -> Result<(), GatewayError> {
    let snapshot = state.route_config.snapshot();
    let mut document = snapshot.document().clone();
    let mut changed = false;
    for provider in &mut document.providers {
        if !provider.pool_refill_in_progress {
            continue;
        }
        let availability = super::availability::pool_availability(state, provider).await?;
        if !provider.auto_refill_enabled
            || super::capacity::pool_min_size(provider) == 0
            || availability.available >= super::capacity::pool_max_size(provider)
        {
            provider.pool_refill_in_progress = false;
            changed = true;
        }
    }
    if changed {
        let runtime = state
            .route_config_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::service_unavailable("Route runtime is unavailable"))?;
        match runtime
            .commit_automation_document(
                snapshot.revision().id(),
                document,
                Some("credential refill cycle reached its available target".to_string()),
            )
            .await
        {
            Ok(_) => {}
            // A concurrent edit wins; the next worker sweep re-evaluates fresh state.
            Err(error) if error.code() == "console_revision_conflict" => {}
            Err(error) => return Err(GatewayError::server_error(error.to_string())),
        }
    }
    Ok(())
}
