//! Failure finalization preserves refund, audit and usage-report ordering.

use super::*;

pub async fn run_failure(
    error: &crate::error::GatewayError,
    ctx: &PipelineContext,
    state: &Arc<AppState>,
) {
    refund_pre_deducted_quota(ctx, state).await;
    finalize_request_audit_failure(ctx, error, state).await;

    let session = match &ctx.session {
        Some(session) => session,
        None => return,
    };

    let report = UsageReport {
        request_id: ctx.req_id.to_string(),
        credential_id: ctx
            .quota_credential_id
            .clone()
            .or_else(|| ctx.credential_ref.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        project_id: session.project_id.clone(),
        user_id: session
            .user_id
            .clone()
            .unwrap_or_else(|| "anonymous".to_string()),
        model: ctx
            .resolved_model
            .clone()
            .or_else(|| ctx.canonical_req.requested_model.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        provider: ctx
            .selected_adapter
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        request_started_at: format_instant_as_iso(&ctx.started_at),
        request_completed_at: format_now_as_iso(),
        latency_ms: ctx.started_at.elapsed().as_millis() as u64,
        success: false,
        error_code: error
            .code
            .clone()
            .or_else(|| Some(format!("{:?}", error.kind))),
    };

    if let Err(report_error) = enqueue_usage_report(&state.redis_pool, &report).await {
        warn!(
            req_id = %ctx.req_id,
            error = %report_error,
            "failed to enqueue failure usage report"
        );
    }
}

pub async fn run_stream_failure(snapshot: FailureFinalizeSnapshot, state: &Arc<AppState>) {
    refund_pre_deducted_quota_snapshot(&snapshot, state).await;
    finalize_request_audit_failure_snapshot(
        &snapshot,
        state,
        Some("stream terminated before completion"),
    )
    .await;

    let Some(project_id) = snapshot.project_id else {
        return;
    };

    let report = UsageReport {
        request_id: snapshot.request_id,
        credential_id: snapshot
            .credential_id
            .unwrap_or_else(|| "unknown".to_string()),
        project_id,
        user_id: snapshot.user_id.unwrap_or_else(|| "anonymous".to_string()),
        model: snapshot.model.unwrap_or_else(|| "unknown".to_string()),
        provider: snapshot.provider.unwrap_or_else(|| "unknown".to_string()),
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
        request_started_at: format_instant_as_iso(&snapshot.started_at),
        request_completed_at: format_now_as_iso(),
        latency_ms: snapshot.started_at.elapsed().as_millis() as u64,
        success: false,
        error_code: Some("stream_failed".to_string()),
    };

    if let Err(report_error) = enqueue_usage_report(&state.redis_pool, &report).await {
        warn!(
            request_id = %report.request_id,
            error = %report_error,
            "failed to enqueue stream failure usage report"
        );
    }
}
