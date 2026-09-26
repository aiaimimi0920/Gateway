//! Best-effort request audit finalization and failure recording.

use super::*;

pub(super) async fn finalize_request_audit_success(
    ctx: &PipelineContext,
    body: &Value,
    usage: Option<TokenUsage>,
    state: &Arc<AppState>,
) {
    finalize_request_audit(
        ctx,
        "completed",
        Some(200),
        usage,
        None,
        Some(build_route_trace_from_ctx(ctx, None)),
        extract_response_id(body),
        state,
    )
    .await;
}

pub(super) async fn finalize_request_audit_failure(
    ctx: &PipelineContext,
    error: &crate::error::GatewayError,
    state: &Arc<AppState>,
) {
    let upstream_status = if ctx.route_attempt_count() > 0 {
        error
            .http_status
            .and_then(|status| u16::try_from(status).ok())
    } else {
        None
    };

    finalize_request_audit(
        ctx,
        "failed",
        upstream_status,
        None,
        Some(error.message.clone()),
        Some(build_route_trace_from_ctx(ctx, Some(error))),
        None,
        state,
    )
    .await;
    persist_conversation_archive_failure(ctx, error, upstream_status, state).await;
    record_credential_model_failure(ctx, error, upstream_status, state).await;
}

pub(super) async fn finalize_request_audit_failure_snapshot(
    snapshot: &FailureFinalizeSnapshot,
    state: &Arc<AppState>,
    error_summary: Option<&str>,
) {
    finalize_request_audit_from_snapshot(
        &snapshot.request_audit,
        "failed",
        None,
        None,
        error_summary.map(str::to_string),
        Some(build_route_trace_from_snapshot(
            &snapshot.request_audit,
            error_summary,
        )),
        state,
    )
    .await;
    let classification = classify_provider_failure(None, Some("stream_failed"), error_summary);
    persist_stream_conversation_archive_failure(
        &snapshot.request_audit,
        Some(classification.class_name().to_string()),
        Some(classification.scope_name().to_string()),
        error_summary,
        state,
    )
    .await;
    record_credential_model_failure_from_snapshot(&snapshot.request_audit, error_summary, state)
        .await;
}

pub(super) async fn finalize_request_audit(
    ctx: &PipelineContext,
    status: &str,
    upstream_status: Option<u16>,
    usage: Option<TokenUsage>,
    error_summary: Option<String>,
    route_trace: Option<Value>,
    response_id: Option<String>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(request_audit_id) = ctx.request_audit_id.as_deref() else {
        return;
    };

    let prompt_tokens = usage.as_ref().map(|value| value.prompt_tokens);
    let completion_tokens = usage.as_ref().map(|value| value.completion_tokens);
    let total_tokens = usage.as_ref().map(|value| value.total_tokens);
    let cache_creation_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_creation_input_tokens);
    let cache_read_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_read_input_tokens);

    let input = db::FinalizeRequestAuditInput {
        status: status.to_string(),
        upstream_status,
        duration_ms: ctx.started_at.elapsed().as_millis() as u64,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens,
        cache_read_input_tokens,
        client_has_cache_control: ctx.client_has_cache_control,
        auto_cache_applied: ctx.auto_cache_applied,
        error_summary,
        access_key_id: ctx.requesting_access_key_id.clone(),
        source_access_key_id: ctx.source_access_key_id.clone(),
        session_id: None,
        route_policy_id: ctx.route_policy_id.clone(),
        provider_account_id: ctx.audited_provider_id(),
        resolved_model: ctx.resolved_model.clone(),
        model_alias: ctx.selected_model_alias.clone(),
        route_attempt_count: ctx.route_attempt_count(),
        route_trace,
        response_id,
    };

    if let Err(error) = db::finalize_request_audit(pg_pool, request_audit_id, input).await {
        warn!(
            req_id = %ctx.req_id,
            request_audit_id = %request_audit_id,
            error = %error,
            "failed to finalize request audit"
        );
    }
}

pub(super) async fn finalize_request_audit_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    status: &str,
    upstream_status: Option<u16>,
    usage: Option<TokenUsage>,
    error_summary: Option<String>,
    route_trace: Option<Value>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(request_audit_id) = snapshot.request_audit_id.as_deref() else {
        return;
    };

    let prompt_tokens = usage.as_ref().map(|value| value.prompt_tokens);
    let completion_tokens = usage.as_ref().map(|value| value.completion_tokens);
    let total_tokens = usage.as_ref().map(|value| value.total_tokens);
    let cache_creation_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_creation_input_tokens);
    let cache_read_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_read_input_tokens);

    let input = db::FinalizeRequestAuditInput {
        status: status.to_string(),
        upstream_status,
        duration_ms: snapshot.started_at.elapsed().as_millis() as u64,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens,
        cache_read_input_tokens,
        client_has_cache_control: snapshot.client_has_cache_control,
        auto_cache_applied: snapshot.auto_cache_applied,
        error_summary,
        access_key_id: snapshot.access_key_id.clone(),
        source_access_key_id: snapshot.source_access_key_id.clone(),
        session_id: None,
        route_policy_id: snapshot.route_policy_id.clone(),
        provider_account_id: snapshot.provider_account_id.clone(),
        resolved_model: snapshot.resolved_model.clone(),
        model_alias: snapshot.model_alias.clone(),
        route_attempt_count: snapshot.route_attempt_count,
        route_trace,
        response_id: snapshot.response_id.clone(),
    };

    if let Err(error) = db::finalize_request_audit(pg_pool, request_audit_id, input).await {
        warn!(
            request_audit_id = %request_audit_id,
            error = %error,
            "failed to finalize request audit from stream snapshot"
        );
    }
}
