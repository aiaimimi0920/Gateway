//! Stream-success finalization from the existing terminal callback snapshot.

use super::*;

pub async fn run_stream_success(
    mut snapshot: RequestAuditFinalizeSnapshot,
    usage: Option<TokenUsage>,
    canonical_completion_semantics: Option<String>,
    raw_response_text: Option<String>,
    response_already_truncated: bool,
    state: &Arc<AppState>,
) {
    apply_stream_completion_semantics(&mut snapshot, canonical_completion_semantics);

    if let Some(project_id) = snapshot.project_id.clone() {
        let usage_ref = usage.as_ref();
        let report = UsageReport {
            request_id: snapshot.request_id.clone(),
            credential_id: snapshot
                .credential_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            project_id,
            user_id: snapshot
                .user_id
                .clone()
                .unwrap_or_else(|| "anonymous".to_string()),
            model: snapshot
                .resolved_model
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            provider: snapshot
                .selected_adapter
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: usage_ref.map(|value| value.prompt_tokens).unwrap_or(0),
            completion_tokens: usage_ref.map(|value| value.completion_tokens).unwrap_or(0),
            total_tokens: usage_ref.map(|value| value.total_tokens).unwrap_or(0),
            cache_creation_input_tokens: usage_ref
                .and_then(|value| value.cache_creation_input_tokens),
            cache_read_input_tokens: usage_ref.and_then(|value| value.cache_read_input_tokens),
            request_started_at: format_instant_as_iso(&snapshot.started_at),
            request_completed_at: format_now_as_iso(),
            latency_ms: snapshot.started_at.elapsed().as_millis() as u64,
            success: true,
            error_code: None,
        };

        if let Err(report_error) = publish_usage(state, &report).await {
            warn!(
                request_id = %report.request_id,
                error = %report_error,
                "failed to enqueue stream success usage report"
            );
        }
    }

    finalize_request_audit_from_snapshot(
        &snapshot,
        "completed",
        Some(200),
        usage.clone(),
        None,
        Some(build_route_trace_from_snapshot(&snapshot, None)),
        state,
    )
    .await;
    persist_stream_conversation_archive_success(
        &snapshot,
        usage.clone(),
        raw_response_text,
        response_already_truncated,
        state,
    )
    .await;
    record_credential_model_success_from_snapshot(&snapshot, state).await;

    let actual_total_tokens = usage
        .as_ref()
        .map(|value| value.total_tokens)
        .unwrap_or(snapshot.pre_deducted_tokens);
    settle_pre_deducted_quota_snapshot(&snapshot, state, actual_total_tokens).await;
}

pub(super) fn apply_stream_completion_semantics(
    snapshot: &mut RequestAuditFinalizeSnapshot,
    canonical_completion_semantics: Option<String>,
) {
    if let Some(canonical_completion_semantics) = canonical_completion_semantics {
        snapshot.canonical_completion_semantics = Some(canonical_completion_semantics);
    }
}
