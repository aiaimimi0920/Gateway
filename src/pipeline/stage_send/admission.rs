//! Recheck hard admission before reservation and again through the attempt gate before send.
use super::*;
/// Authorization has already filtered this candidate; privacy must latch before any skip.
pub(super) fn preflight(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Result<(), GatewayError> {
    crate::protocol::commandcode::zdr::latch(&mut ctx.request_headers, &candidate.payload)?;
    crate::protocol::bedrock_turn::validate_request(
        crate::protocol::call_flow::prepare(&candidate.payload)?
            .payload
            .as_ref(),
        &ctx.canonical_req,
    )
}

pub(super) fn validate_continuation(
    ctx: &PipelineContext,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    model: &str,
) -> Result<(), GatewayError> {
    if ctx
        .canonical_req
        .raw_body
        .get(crate::protocol::bedrock_turn::HISTORY_FIELD)
        .is_some()
    {
        crate::upstream::client::UpstreamClient::build_request_plan(
            payload,
            &ctx.canonical_req,
            model,
            ctx.canonical_req.stream,
        )?;
    }
    Ok(())
}

pub(super) async fn prepare(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    request: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Result<(String, stage_rate_limit::eligibility::EligibilityCheck), GatewayError> {
    crate::routing::call_flow::validate_prepared_candidate(candidate)?;
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
