//! Observe before caller-facing packing can replace a provider name with an alias.
use super::*;
use crate::metrics::model_evidence::ModelObservationMeta;
use crate::protocol::model_declaration::ModelDeclaration;

pub(super) fn metadata(
    ctx: &PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    model: &str,
) -> ModelObservationMeta {
    ModelObservationMeta::new(
        &candidate.provider_account_id,
        ctx.canonical_req.requested_model.as_deref(),
        model,
        ctx.canonical_req.stream,
    )
}

pub(super) fn buffered(
    ctx: &PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    model: &str,
    declaration: Option<&ModelDeclaration>,
) {
    global_gateway_metrics().observe_model_declaration(metadata(ctx, candidate, model).finish(
        declaration,
        if declaration.is_some() {
            "declared"
        } else {
            "missing_or_unobserved"
        },
        "buffered",
    ));
}
