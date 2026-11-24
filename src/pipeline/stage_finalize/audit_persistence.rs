//! Best-effort request audit finalization and failure recording.

use super::*;

/// The receipt owns settlement. Audits must not publish usage or invoke ordinary finalizers.
pub(in crate::pipeline) async fn finish_stored_response_audit(
    ctx: &PipelineContext,
    result: &Result<Value, crate::error::GatewayError>,
    operation: &str,
    disconnected: bool,
    receipt_id: Option<&str>,
    state: &Arc<AppState>,
) {
    let error = result.as_ref().err();
    let mut trace = build_route_trace_from_ctx(ctx, error);
    trace["storedResponseOperation"] = json!(operation);
    trace["storedResponseId"] =
        json!(receipt_id.or_else(|| result.as_ref().ok().and_then(|body| body["id"].as_str())));
    trace["callerDisconnected"] = json!(disconnected);
    let usage = if operation == "create" {
        result
            .as_ref()
            .ok()
            .and_then(|body| parse_upstream_usage(body, "sap_aicore"))
    } else {
        None
    };
    finalize_request_audit(
        ctx,
        if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
        ctx.observed_upstream_status
            .unwrap_or_else(|| result.is_ok().then_some(200)),
        usage,
        error.map(|err| err.message.clone()),
        Some(trace),
        None, // One receipt ID can have many operation audits.
        state,
    )
    .await;
}

pub(super) async fn finalize_request_audit_success(
    ctx: &PipelineContext,
    body: &Value,
    usage: Option<TokenUsage>,
    state: &Arc<AppState>,
) {
    let mut trace = build_route_trace_from_ctx(ctx, None);
    let response_id =
        super::luma_agents_audit::success(ctx.selected_adapter.as_deref(), body, &mut trace)
            .or_else(|| extract_response_id(body));
    finalize_request_audit(
        ctx,
        "completed",
        ctx.observed_upstream_status.unwrap_or(Some(200)),
        usage,
        None,
        Some(trace),
        response_id,
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
        ctx.observed_upstream_status
            .unwrap_or_else(|| error.provider_response_status())
    } else {
        None
    };

    let mut trace = build_route_trace_from_ctx(ctx, Some(error));
    let response_id = crate::protocol::luma_agents::record_failure_audit(error, &mut trace)
        .or_else(|| ctx.luma_generation_audit.as_ref()?.record(&mut trace));
    finalize_request_audit(
        ctx,
        StreamFailureCause::from_observation(ctx.request_budget.stopped_reason(), true).status(),
        upstream_status,
        None,
        Some(error.message.clone()),
        Some(trace),
        response_id,
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
    cause: StreamFailureCause,
) {
    let mut trace = build_route_trace_from_snapshot(&snapshot.request_audit, error_summary);
    trace["terminationReason"] = json!(cause.label());
    if cause != StreamFailureCause::Upstream {
        for field in ["failureClass", "failureScope", "failurePermanent"] {
            trace[field] = Value::Null;
        }
    }
    finalize_request_audit_from_snapshot(
        &snapshot.request_audit,
        cause.status(),
        snapshot.request_audit.observed_upstream_status.flatten(),
        None,
        error_summary.map(str::to_string),
        Some(trace),
        cause == StreamFailureCause::ClientDisconnected,
        state,
    )
    .await;
    let classification = classify_provider_failure(None, Some("stream_failed"), error_summary);
    persist_stream_conversation_archive_failure(
        &snapshot.request_audit,
        (cause == StreamFailureCause::Upstream).then(|| classification.class_name().to_string()),
        (cause == StreamFailureCause::Upstream).then(|| classification.scope_name().to_string()),
        error_summary,
        state,
    )
    .await;
    if cause == StreamFailureCause::Upstream {
        record_credential_model_failure_from_snapshot(
            &snapshot.request_audit,
            error_summary,
            state,
        )
        .await;
    }
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
        client_disconnected: ctx.request_budget.stopped_reason() == Some("caller_cancelled"),
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
        resolved_model: ctx.audited_resolved_model(),
        model_alias: ctx.selected_model_alias.clone(),
        route_attempt_count: ctx.route_attempt_count(),
        route_trace,
        response_id,
    };

    if let Err(error) = crate::local_runtime::audits::finalize(state, request_audit_id, input).await
    {
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
    client_disconnected: bool,
    state: &Arc<AppState>,
) {
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
        client_disconnected,
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

    if let Err(error) = crate::local_runtime::audits::finalize(state, request_audit_id, input).await
    {
        warn!(
            request_audit_id = %request_audit_id,
            error = %error,
            "failed to finalize request audit from stream snapshot"
        );
    }
}
