//! Explicit final FINISH uses the original run transport, not device-signed completion wire.
use super::execution::sequence::RunTotals;
use super::{run_wire, BusinessError, BusinessRequests};
use serde_json::json;
use std::time::Duration;
pub(super) async fn finish(
    scope: &BusinessRequests<'_>,
    run: &str,
    totals: RunTotals,
    error: Option<BusinessError>,
    budget: Duration,
) -> Result<u16, BusinessError> {
    let status = match error {
        Some(BusinessError::Cancelled) => "cancelled",
        Some(_) => "failed",
        None => "completed",
    };
    let mut body = json!({"action":"FINISH","runId":run,"status":status,
        "totalSteps":totals.total_steps,"directCredits":totals.direct_credits,"totalCredits":totals.total_credits,"steps":[]});
    if let Some(error) = error {
        body["errorMessage"] = json!(error.to_string());
    }
    let (status, _) = run_wire::post(
        scope,
        serde_json::to_vec(&body).map_err(|_| BusinessError::InvalidInput)?,
        budget,
    )
    .await?;
    Ok(status)
}
