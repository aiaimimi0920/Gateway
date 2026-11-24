//! Recheck hard admission before reservation and again through the attempt gate before send.
use super::*;

pub(super) fn record_rejection(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    error: &GatewayError,
) {
    // Budget/cancellation errors may carry the previous sender's error code.
    // They stop this request, not prove a refusal by the current candidate.
    if error.request_budget_stop_reason().is_none() {
        crate::pipeline::stage_route::rejections::record_error(ctx, candidate, error);
    }
}

pub(super) fn record_unsent_next(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    result: &Result<PipelineOutput, AttemptError>,
    before: (u32, Option<Option<u16>>),
) {
    if let Err(AttemptError::Next(error)) = result {
        if ctx.route_attempt_count() == before.0 {
            // Preserve both the legacy default and any previous actual response.
            // Stop/success/sent outcomes retain their own observation.
            ctx.observed_upstream_status = before.1;
            record_rejection(ctx, candidate, error);
        }
    }
}

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

pub(super) fn validate_plan(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    model: &str,
) -> Result<(), GatewayError> {
    // SenseNova's native planner owns media and reasoning-history validation.
    // Reject before begin_attempt so local input errors neither claim a send nor
    // poison credential health. Reuse that planner instead of duplicating its rules.
    if crate::protocol::sensenova::owns_payload(payload)
        || ctx
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
        )
        .map_err(|error| {
            crate::pipeline::stage_route::rejections::record_error(ctx, candidate, &error);
            error
        })?;
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

pub(super) async fn record_running(
    ctx: &PipelineContext,
    state: &AppState,
    candidate: &crate::routing::candidate::RouteCandidate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    model: &str,
) {
    // SAP prepares OAuth later. Its exact identity is published by actual-attempt
    // finalization, never a sticky SQL attribution left by a zero-send rejection.
    if crate::protocol::sap_aicore::owns_payload(payload) {
        return;
    }
    if let (Some(local), Some(audit_id)) = (&state.local_runtime, &ctx.request_audit_id) {
        if let Err(error) = local
            .route_audit(audit_id, &candidate.provider_account_id, model)
            .await
        {
            warn!(code = ?error.code, "Failed to attribute local running request");
        }
    }
}
