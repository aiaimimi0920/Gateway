//! Bounded accounting reads for a validated terminal task; never submit or replace it.
use super::{
    generation,
    transport::{self, Material},
};
use crate::{error::GatewayError, protocol::luma_agents::GenerationRequest};
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;

const RETURN_RESERVE: Duration = Duration::from_millis(250);
const MAX_WAIT: Duration = Duration::from_secs(8);

pub(super) async fn collect(
    client: &rquest::Client,
    material: &Material,
    request: &GenerationRequest,
    id: &str,
    terminal: &Value,
    deadline: Instant,
    total: &mut usize,
) -> Result<Value, GatewayError> {
    let original = generation::usage(terminal)?;
    if !pending(terminal) {
        return Ok(original);
    }
    // Reserve time before the enclosing request deadline so optional accounting cannot
    // replace a terminal result with a timeout. All I/O stays owned by this future.
    let now = Instant::now();
    let budget = deadline
        .saturating_duration_since(now)
        .saturating_sub(RETURN_RESERVE)
        .min(MAX_WAIT);
    if budget.is_zero() {
        return Ok(original);
    }
    let until = now + budget;
    for attempt in 0..3 {
        #[cfg(not(test))]
        let delay = Duration::from_secs(1 << attempt);
        #[cfg(test)]
        let delay = Duration::from_millis(5 << attempt);
        if Instant::now() + delay >= until {
            break;
        }
        tokio::time::sleep(delay).await;
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let response = tokio::time::timeout_at(
            until,
            transport::json(client, material, Some(id), None, remaining, total),
        )
        .await;
        let job = match response {
            Ok(Ok(job)) => job,
            Ok(Err(error)) => {
                // Only controlled codes: no credential, asset URL, response body or task ID.
                tracing::warn!(code = error.code.as_deref().unwrap_or("luma_agents_settlement"),
                    "Luma Agents usage lookup stopped; terminal result retained with unknown credits");
                return Ok(original);
            }
            Err(_) => break,
        };
        if generation::validate(&job, request, id).is_err() || job["state"] != terminal["state"] {
            tracing::warn!(
                code = "luma_agents_generation",
                "Luma Agents usage lookup rejected; terminal result retained with unknown credits"
            );
            return Ok(original);
        }
        // Ignore new outputs/reasons. Accept charges only for the same task and terminal state.
        if !pending(&job) {
            crate::protocol::luma_agents::generation_audit::observe_credits(
                id,
                job["usage"]["credits_charged"].as_number().cloned(),
            );
            return generation::usage(&job);
        }
    }
    tracing::debug!("Luma Agents usage lookup budget ended; credits remain unknown");
    Ok(original)
}

fn pending(job: &Value) -> bool {
    // A null/missing usage means reporting is unavailable, not a delayed settlement.
    job.get("usage").is_some_and(Value::is_object) && job["usage"]["credits_charged"].is_null()
}
