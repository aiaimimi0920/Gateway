//! Recheck hard admission before reservation and again through the attempt gate before send.
use super::*;

pub(super) async fn prepare(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    request: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<(String, stage_rate_limit::eligibility::EligibilityCheck), GatewayError> {
    let model = candidate
        .upstream_model
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| upstream_model_helpers::resolve_model(&candidate.payload, request))
        .to_string();
    let eligibility =
        stage_rate_limit::eligibility::EligibilityCheck::new(state, candidate, &model);
    eligibility.check().await?;
    Ok((model, eligibility))
}
