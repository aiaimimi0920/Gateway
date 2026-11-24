//! Request-owned rejection facts; no credentials, outbound calls, or winner attribution.
use super::*;
use crate::routing::config::RejectedCandidate;

pub(crate) fn record(ctx: &mut PipelineContext, candidate: &RouteCandidate, reason: &str) {
    let rejection = RejectedCandidate::from_candidate(candidate, reason);
    if ctx.rejected_candidates.len() < 100
        && !ctx.rejected_candidates.iter().any(|previous| {
            previous.provider_account_id == rejection.provider_account_id
                && previous.real_credential_ref == rejection.real_credential_ref
                && previous.reason == rejection.reason
        })
    {
        ctx.rejected_candidates.push(rejection);
    }
}

pub(crate) fn record_error(
    ctx: &mut PipelineContext,
    candidate: &RouteCandidate,
    error: &GatewayError,
) {
    record(
        ctx,
        candidate,
        error.code.as_deref().unwrap_or("admission_rejected"),
    );
}

// Preserve projection alignment and existing all-blocked recovery behavior. The send gate
// remains authoritative: retaining a candidate neither grants eligibility nor records a send.
pub(super) fn retain_queueable_pairs<T>(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidates: Vec<RouteCandidate>,
    rows: Vec<T>,
    requested_model: &str,
) -> (Vec<RouteCandidate>, Vec<T>) {
    let mut queueable_candidates = Vec::new();
    let mut queueable_rows = Vec::new();
    let mut blocked_candidates = Vec::new();
    let mut blocked_rows = Vec::new();
    for (candidate, row) in candidates.into_iter().zip(rows) {
        let model = candidate
            .upstream_model
            .as_deref()
            .unwrap_or(requested_model);
        if crate::routing::admission::candidate_queueable(state, &candidate, model) {
            queueable_candidates.push(candidate);
            queueable_rows.push(row);
        } else {
            let reason = crate::routing::admission::check_candidate(state, &candidate, model)
                .err()
                .and_then(|error| error.code)
                .unwrap_or_else(|| "temporarily_unavailable".into());
            record(ctx, &candidate, &reason);
            blocked_candidates.push(candidate);
            blocked_rows.push(row);
        }
    }
    if queueable_candidates.is_empty() && !blocked_candidates.is_empty() {
        debug!(
            req_id = %ctx.req_id,
            model = requested_model,
            blocked_count = blocked_candidates.len(),
            "all queueable candidates blocked; retaining temporarily blocked candidates for send-stage recovery"
        );
        (blocked_candidates, blocked_rows)
    } else {
        (queueable_candidates, queueable_rows)
    }
}

pub(super) fn retain_queueable(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidates: &mut Vec<RouteCandidate>,
) {
    candidates.retain(|candidate| {
        let model = candidate.upstream_model.as_deref().unwrap_or_else(|| {
            crate::upstream::upstream_model_helpers::resolve_model(
                &candidate.payload,
                &ctx.canonical_req,
            )
        });
        let queueable = crate::routing::admission::candidate_queueable(state, candidate, model);
        if !queueable {
            let reason = crate::routing::admission::check_candidate(state, candidate, model)
                .err()
                .and_then(|error| error.code)
                .unwrap_or_else(|| "temporarily_unavailable".into());
            record(ctx, candidate, &reason);
        }
        queueable
    });
}

pub(super) fn capture(candidates: &[RouteCandidate]) -> Vec<RejectedCandidate> {
    candidates
        .iter()
        .take(100)
        .map(|candidate| {
            RejectedCandidate::from_candidate(candidate, "credential_model_unavailable")
        })
        .collect()
}

pub(super) fn record_removed(
    ctx: &mut PipelineContext,
    candidates: &[RouteCandidate],
    before: Vec<RejectedCandidate>,
) {
    for rejected in before {
        if ctx.rejected_candidates.len() >= 100 {
            break;
        }
        if !candidates.iter().any(|candidate| {
            candidate.provider_account_id == rejected.provider_account_id
                && candidate
                    .provider_credential_id
                    .as_ref()
                    .or(candidate.payload.credential_id.as_ref())
                    == rejected.real_credential_ref.as_ref()
        }) {
            ctx.rejected_candidates.push(rejected);
        }
    }
}
