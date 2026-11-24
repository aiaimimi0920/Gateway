//! Neutral pre-handoff cancellation owns exactly one refund and never updates provider health.
use super::*;

pub async fn run_cancelled(ctx: &PipelineContext, state: &Arc<AppState>) {
    crate::cash_billing::finalization::failure(
        ctx.cash_charge.as_ref(),
        ctx.route_attempt_count(),
        state,
    )
    .await;
    refund_pre_deducted_quota(ctx, state).await;
    let cause = StreamFailureCause::from_observation(ctx.request_budget.stopped_reason(), false);
    let reason = "Request ended before terminal result submission.";
    let mut trace = build_route_trace_from_ctx(ctx, None);
    let response_id = ctx
        .luma_generation_audit
        .as_ref()
        .and_then(|audit| audit.record(&mut trace));
    trace["terminationReason"] = json!(if cause == StreamFailureCause::ConsumerClosed {
        "request_consumer_closed"
    } else {
        cause.label()
    });
    finalize_request_audit(
        ctx,
        cause.status(),
        ctx.observed_upstream_status.flatten(),
        None,
        Some(reason.into()),
        Some(trace),
        response_id,
        state,
    )
    .await;
    let snapshot = snapshot_request_audit(ctx);
    persist_stream_conversation_archive_failure(&snapshot, None, None, Some(reason), state).await;
    let error =
        crate::error::GatewayError::service_unavailable(reason).with_code("request_cancelled");
    failure::report_failure(&error, ctx, state).await;
}
