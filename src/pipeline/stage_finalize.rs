// ---------------------------------------------------------------------------
// Pipeline stage 5 — finalization
//
// After a successful non-streaming response:
//   - Enqueues a usage report to Redis
//   - Stores the response in the response cache
//
// Streaming responses record metrics inside the TrackedStream callback, so
// finalize is effectively a no-op for SSE.
//
// Finalization errors are logged but not propagated — the response has already
// been (or is being) returned to the client.
// ---------------------------------------------------------------------------

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::conversation_archive::{
    archive_user_id, is_conversation_archive_endpoint, persist_conversation_archive,
    PersistConversationArchiveInput,
};
use crate::db;
use crate::protocol::registry::{
    ANTHROPIC_MESSAGES_FAMILY, OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY,
    OPENAI_CHAT_FAMILY, OPENAI_EMBEDDINGS_FAMILY, OPENAI_IMAGES_EDITS_FAMILY,
    OPENAI_IMAGES_GENERATIONS_FAMILY, OPENAI_LEGACY_COMPLETIONS_FAMILY,
    OPENAI_MUSIC_GENERATIONS_FAMILY, OPENAI_REALTIME_FAMILY, OPENAI_RESPONSES_FAMILY,
    OPENAI_VIDEOS_GENERATIONS_FAMILY, SEARCH_API_FAMILY,
};
use crate::provider_failure::classify_provider_failure;
use crate::redis::credential_cache::set_credential_affinity;
use crate::redis::usage_tracking::{
    enqueue_usage_report, parse_upstream_usage, refund_quota, settle_quota_after_usage, TokenUsage,
    UsageReport,
};
use crate::state::AppState;

use super::PipelineContext;

#[derive(Debug, Clone)]
pub struct RequestAuditFinalizeSnapshot {
    pub request_id: String,
    pub request_audit_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
    pub platform_access_id: Option<String>,
    pub real_credential_ref: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub resolved_model: Option<String>,
    pub model_alias: Option<String>,
    pub route_selection_strategy: Option<String>,
    pub selected_provider_label: Option<String>,
    pub selected_adapter: Option<String>,
    pub selected_protocol_profile: Option<String>,
    pub selected_execution_mode: Option<String>,
    pub selected_upstream_target_protocol_family: Option<String>,
    pub selected_upstream_target_conversation_family: Option<String>,
    pub canonical_conversation_semantics: Option<String>,
    pub selected_upstream_target_tool_family: Option<String>,
    pub tool_strategy: Option<String>,
    pub canonical_tool_choice_semantics: Option<String>,
    pub canonical_completion_semantics: Option<String>,
    pub selected_routing_score: Option<f64>,
    pub selected_health_weight: Option<f64>,
    pub selected_capacity_weight: Option<f64>,
    pub selected_degraded: Option<bool>,
    pub selected_breaker_open: Option<bool>,
    pub selected_degradation_reasons: Vec<String>,
    pub route_attempt_count: u32,
    pub attempted_provider_ids: Vec<String>,
    pub protocol_family: String,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub request_payload: Value,
    pub response_id: Option<String>,
    pub client_has_cache_control: bool,
    pub auto_cache_applied: bool,
    pub pre_deducted_tokens: u64,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct FailureFinalizeSnapshot {
    pub request_id: String,
    pub credential_id: Option<String>,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub started_at: Instant,
    pub pre_deducted_tokens: u64,
    pub request_audit: RequestAuditFinalizeSnapshot,
}

/// Run post-response finalization.
///
/// For JSON responses: parse usage, enqueue usage report, store in cache.
/// For streaming (SSE) responses: pass `None` — metrics are captured by the
/// TrackedStream callback when the stream ends.
///
/// `json_body` is `Some` for buffered responses, `None` for streaming.
pub async fn run(json_body: Option<&Value>, ctx: &PipelineContext, state: &Arc<AppState>) {
    match json_body {
        None => {
            // Streaming: the TrackedStream callback handles metrics on completion.
            debug!(req_id = %ctx.req_id, "streaming response; finalize is no-op");
        }
        Some(body) => {
            finalize_json(body, ctx, state).await;
        }
    }
}

pub async fn run_non_json_success(ctx: &PipelineContext, state: &Arc<AppState>) {
    finalize_request_audit(
        ctx,
        "completed",
        Some(200),
        None,
        None,
        Some(build_route_trace_from_ctx(ctx, None)),
        None,
        state,
    )
    .await;
}

pub async fn begin_request_audit(ctx: &mut PipelineContext, state: &Arc<AppState>) {
    if ctx.request_audit_id.is_some() {
        return;
    }

    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(session) = ctx.session.as_ref() else {
        return;
    };
    let api_key_id = session.api_key_id.clone();
    let user_credential_id = session.user_credential_id.clone();
    let access_key_id = session.access_key_id.clone();
    if api_key_id.is_none() && user_credential_id.is_none() && access_key_id.is_none() {
        return;
    }

    let input = db::CreateRequestAuditInput {
        project_id: session.project_id.clone(),
        api_key_id,
        user_credential_id,
        access_key_id,
        source_access_key_id: ctx.source_access_key_id.clone(),
        session_id: None,
        route_policy_id: ctx.route_policy_id.clone(),
        provider_account_id: None,
        protocol_family: protocol_family_name(ctx),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: None,
        model_alias: None,
        stream: ctx.stream,
        route_attempt_count: 1,
        response_id: format!("gw-audit-{}", ctx.req_id),
        previous_response_id: ctx.canonical_req.previous_response_id.clone(),
        route_trace: None,
    };

    match db::create_request_audit(pg_pool, input).await {
        Ok(request_audit_id) => {
            ctx.request_audit_id = Some(request_audit_id);
        }
        Err(error) => {
            warn!(
                req_id = %ctx.req_id,
                error = %error,
                "failed to create request audit"
            );
        }
    }
}

pub fn snapshot_request_audit(ctx: &PipelineContext) -> RequestAuditFinalizeSnapshot {
    RequestAuditFinalizeSnapshot {
        request_id: ctx.req_id.to_string(),
        request_audit_id: ctx.request_audit_id.clone(),
        project_id: ctx
            .session
            .as_ref()
            .map(|session| session.project_id.clone()),
        user_id: archive_user_id(
            ctx.neuro_user_id.as_deref(),
            ctx.session
                .as_ref()
                .and_then(|session| session.user_id.as_deref()),
        ),
        session_id: ctx.canonical_req.explicit_session_key.clone(),
        credential_id: ctx
            .quota_credential_id
            .clone()
            .or_else(|| ctx.credential_ref.clone()),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        access_key_id: ctx.requesting_access_key_id.clone(),
        source_access_key_id: ctx.source_access_key_id.clone(),
        platform_access_id: ctx.selected_platform_access_id.clone(),
        real_credential_ref: ctx.selected_real_credential_ref.clone(),
        route_policy_id: ctx.route_policy_id.clone(),
        provider_account_id: ctx.selected_provider_id.clone(),
        resolved_model: ctx.resolved_model.clone(),
        model_alias: ctx.selected_model_alias.clone(),
        route_selection_strategy: ctx.route_selection_strategy.clone(),
        selected_provider_label: ctx.selected_provider_label.clone(),
        selected_adapter: ctx.selected_adapter.clone(),
        selected_protocol_profile: ctx.selected_protocol_profile.clone(),
        selected_execution_mode: ctx.selected_execution_mode.clone(),
        selected_upstream_target_protocol_family: ctx
            .selected_upstream_target_protocol_family
            .clone(),
        selected_upstream_target_conversation_family: ctx
            .selected_upstream_target_conversation_family
            .clone(),
        canonical_conversation_semantics: ctx.canonical_conversation_semantics.clone(),
        selected_upstream_target_tool_family: ctx.selected_upstream_target_tool_family.clone(),
        tool_strategy: ctx.tool_strategy.clone(),
        canonical_tool_choice_semantics: ctx.canonical_tool_choice_semantics.clone(),
        canonical_completion_semantics: ctx.canonical_completion_semantics.clone(),
        selected_routing_score: ctx.selected_routing_score,
        selected_health_weight: ctx.selected_health_weight,
        selected_capacity_weight: ctx.selected_capacity_weight,
        selected_degraded: ctx.selected_degraded,
        selected_breaker_open: ctx.selected_breaker_open,
        selected_degradation_reasons: ctx.selected_degradation_reasons.clone(),
        route_attempt_count: ctx.route_attempt_count(),
        attempted_provider_ids: ctx.attempted_provider_ids(),
        protocol_family: protocol_family_name(ctx),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_id: None,
        client_has_cache_control: ctx.client_has_cache_control,
        auto_cache_applied: ctx.auto_cache_applied,
        pre_deducted_tokens: ctx.quota_pre_deducted_tokens,
        started_at: ctx.started_at,
    }
}

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

        if let Err(report_error) = enqueue_usage_report(&state.redis_pool, &report).await {
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

    if let Some(actual_total_tokens) = usage.as_ref().map(|value| value.total_tokens) {
        settle_pre_deducted_quota_snapshot(&snapshot, state, actual_total_tokens).await;
    }
}

fn apply_stream_completion_semantics(
    snapshot: &mut RequestAuditFinalizeSnapshot,
    canonical_completion_semantics: Option<String>,
) {
    if let Some(canonical_completion_semantics) = canonical_completion_semantics {
        snapshot.canonical_completion_semantics = Some(canonical_completion_semantics);
    }
}

async fn finalize_json(body: &Value, ctx: &PipelineContext, state: &Arc<AppState>) {
    let session = match &ctx.session {
        Some(s) => s,
        None => {
            debug!(req_id = %ctx.req_id, "no session; skipping usage report");
            record_credential_model_success(ctx, state).await;
            return;
        }
    };

    // Extract usage from response body (OpenAI and Anthropic shapes).
    let usage = ctx
        .observed_usage
        .clone()
        .or_else(|| extract_usage(body, &ctx.selected_adapter));
    let prompt_tokens = usage.as_ref().map(|value| value.prompt_tokens).unwrap_or(0);
    let completion_tokens = usage
        .as_ref()
        .map(|value| value.completion_tokens)
        .unwrap_or(0);
    let total_tokens = usage.as_ref().map(|value| value.total_tokens).unwrap_or(0);
    let cache_creation_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_creation_input_tokens);
    let cache_read_input_tokens = usage
        .as_ref()
        .and_then(|value| value.cache_read_input_tokens);

    // Build usage report.
    let latency_ms = ctx.started_at.elapsed().as_millis() as u64;

    // Prefer resolved model from ctx (set by stage_send), fall back to response body.
    let model = ctx
        .resolved_model
        .clone()
        .or_else(|| {
            body.get("model")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Use real provider info from ctx.
    let provider = ctx
        .selected_adapter
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let report = UsageReport {
        request_id: ctx.req_id.to_string(),
        credential_id: ctx
            .credential_ref
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        project_id: session.project_id.clone(),
        user_id: session
            .user_id
            .clone()
            .unwrap_or_else(|| "anonymous".to_string()),
        model,
        provider,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens,
        cache_read_input_tokens,
        request_started_at: format_instant_as_iso(&ctx.started_at),
        request_completed_at: format_now_as_iso(),
        latency_ms,
        success: true,
        error_code: None,
    };

    if let Err(e) = enqueue_usage_report(&state.redis_pool, &report).await {
        warn!(req_id = %ctx.req_id, error = %e, "failed to enqueue usage report");
    } else {
        debug!(req_id = %ctx.req_id, total_tokens, "usage report enqueued");
    }

    finalize_request_audit_success(ctx, body, usage.clone(), state).await;
    persist_conversation_archive_success(ctx, body, usage.clone(), state).await;
    record_credential_model_success(ctx, state).await;

    if let Some(actual_total_tokens) = usage.as_ref().map(|value| value.total_tokens) {
        settle_pre_deducted_quota(ctx, state, actual_total_tokens).await;
    }

    // ── Record credential affinity ──────────────────────────────────────
    // After a successful call, remember which credential was used so the
    // next request in the same conversation/user scope reuses it (upstream
    // prompt cache benefits).
    if let Some(ref provider_id) = ctx.selected_provider_id {
        let model = ctx.resolved_model.as_deref().unwrap_or("unknown");
        if let (
            Some(requesting_access_key_id),
            Some(source_access_key_id),
            Some(platform_access_id),
        ) = (
            ctx.requesting_access_key_id.as_deref(),
            ctx.source_access_key_id.as_deref(),
            ctx.selected_platform_access_id.as_deref(),
        ) {
            let _ = db::record_access_sticky_affinity(
                &state.redis_pool,
                requesting_access_key_id,
                source_access_key_id,
                platform_access_id,
                provider_id,
                ctx.selected_real_credential_ref.as_deref(),
                model,
                ctx.canonical_req.explicit_session_key.as_deref(),
            )
            .await;
        } else {
            // Session-level affinity (conversation stickiness)
            if let Some(ref sk) = ctx.canonical_req.explicit_session_key {
                let scope = format!("session:{}", sk);
                let _ =
                    set_credential_affinity(&state.redis_pool, &scope, model, provider_id).await;
            }
            // User-level affinity (fallback stickiness)
            if let Some(ref uid) = session.user_id {
                let scope = format!("user:{}", uid);
                let _ =
                    set_credential_affinity(&state.redis_pool, &scope, model, provider_id).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn persist_conversation_archive_success(
    ctx: &PipelineContext,
    body: &Value,
    usage: Option<TokenUsage>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint(ctx.canonical_req.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: ctx.request_audit_id.clone(),
        request_id: ctx.req_id.to_string(),
        project_id: ctx
            .session
            .as_ref()
            .map(|session| session.project_id.clone()),
        user_id: archive_user_id(
            ctx.neuro_user_id.as_deref(),
            ctx.session
                .as_ref()
                .and_then(|session| session.user_id.as_deref()),
        ),
        session_id: ctx.canonical_req.explicit_session_key.clone(),
        provider_account_id: ctx.selected_provider_id.clone(),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        protocol_family: protocol_family_name(ctx),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: ctx.resolved_model.clone(),
        status: "completed".to_string(),
        upstream_status: Some(200),
        failure_class: None,
        failure_scope: None,
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_payload: Some(json!({
            "result": body,
            "usage": usage,
            "responseId": extract_response_id(body),
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to persist conversation archive"
        );
    }
}

async fn persist_conversation_archive_failure(
    ctx: &PipelineContext,
    error: &crate::error::GatewayError,
    upstream_status: Option<u16>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint(ctx.canonical_req.endpoint_kind) {
        return;
    }
    let classification = classify_provider_failure(
        upstream_status,
        error.code.as_deref(),
        Some(error.message.as_str()),
    );
    let input = PersistConversationArchiveInput {
        request_audit_id: ctx.request_audit_id.clone(),
        request_id: ctx.req_id.to_string(),
        project_id: ctx
            .session
            .as_ref()
            .map(|session| session.project_id.clone()),
        user_id: archive_user_id(
            ctx.neuro_user_id.as_deref(),
            ctx.session
                .as_ref()
                .and_then(|session| session.user_id.as_deref()),
        ),
        session_id: ctx.canonical_req.explicit_session_key.clone(),
        provider_account_id: ctx.selected_provider_id.clone(),
        provider_credential_ref: ctx
            .selected_provider_credential_id
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone()),
        protocol_family: protocol_family_name(ctx),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        endpoint_kind: endpoint_kind_name(ctx),
        requested_model: ctx.canonical_req.requested_model.clone(),
        resolved_model: ctx.resolved_model.clone(),
        status: "failed".to_string(),
        upstream_status,
        failure_class: Some(classification.class_name().to_string()),
        failure_scope: Some(classification.scope_name().to_string()),
        request_payload: build_archive_request_payload_from_ctx(ctx),
        response_payload: Some(json!({
            "error": {
                "message": error.message.clone(),
                "code": error.code.clone(),
                "kind": format!("{:?}", error.kind),
                "httpStatus": error.http_status,
            }
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to persist failed conversation archive"
        );
    }
}

async fn persist_stream_conversation_archive_success(
    snapshot: &RequestAuditFinalizeSnapshot,
    usage: Option<TokenUsage>,
    raw_response_text: Option<String>,
    response_already_truncated: bool,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint_name(&snapshot.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: snapshot.request_audit_id.clone(),
        request_id: snapshot.request_id.clone(),
        project_id: snapshot.project_id.clone(),
        user_id: snapshot.user_id.clone(),
        session_id: snapshot.session_id.clone(),
        provider_account_id: snapshot.provider_account_id.clone(),
        provider_credential_ref: snapshot.provider_credential_ref.clone(),
        protocol_family: snapshot.protocol_family.clone(),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        endpoint_kind: snapshot.endpoint_kind.clone(),
        requested_model: snapshot.requested_model.clone(),
        resolved_model: snapshot.resolved_model.clone(),
        status: "completed".to_string(),
        upstream_status: Some(200),
        failure_class: None,
        failure_scope: None,
        request_payload: snapshot.request_payload.clone(),
        response_payload: Some(json!({
            "rawSseText": raw_response_text,
            "usage": usage,
            "responseId": snapshot.response_id,
        })),
        request_already_truncated: false,
        response_already_truncated,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to persist stream conversation archive"
        );
    }
}

async fn persist_stream_conversation_archive_failure(
    snapshot: &RequestAuditFinalizeSnapshot,
    failure_class: Option<String>,
    failure_scope: Option<String>,
    error_summary: Option<&str>,
    state: &Arc<AppState>,
) {
    if !is_conversation_archive_endpoint_name(&snapshot.endpoint_kind) {
        return;
    }
    let input = PersistConversationArchiveInput {
        request_audit_id: snapshot.request_audit_id.clone(),
        request_id: snapshot.request_id.clone(),
        project_id: snapshot.project_id.clone(),
        user_id: snapshot.user_id.clone(),
        session_id: snapshot.session_id.clone(),
        provider_account_id: snapshot.provider_account_id.clone(),
        provider_credential_ref: snapshot.provider_credential_ref.clone(),
        protocol_family: snapshot.protocol_family.clone(),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        endpoint_kind: snapshot.endpoint_kind.clone(),
        requested_model: snapshot.requested_model.clone(),
        resolved_model: snapshot.resolved_model.clone(),
        status: "failed".to_string(),
        upstream_status: None,
        failure_class,
        failure_scope,
        request_payload: snapshot.request_payload.clone(),
        response_payload: Some(json!({
            "error": {
                "message": error_summary,
                "code": "stream_failed",
            }
        })),
        request_already_truncated: false,
        response_already_truncated: false,
    };
    if let Err(error) = persist_conversation_archive(state, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to persist failed stream conversation archive"
        );
    }
}

fn build_archive_request_payload_from_ctx(ctx: &PipelineContext) -> Value {
    json!({
        "canonicalRequest": serde_json::to_value(&ctx.canonical_req).unwrap_or(Value::Null),
        "selected": {
            "providerAccountId": ctx.selected_provider_id.clone(),
            "providerCredentialRef": ctx.selected_provider_credential_id.clone(),
            "protocolProfile": ctx.selected_protocol_profile.clone(),
            "executionMode": ctx.selected_execution_mode.clone(),
            "resolvedModel": ctx.resolved_model.clone(),
        },
    })
}

fn is_conversation_archive_endpoint_name(endpoint_kind: &str) -> bool {
    matches!(
        endpoint_kind,
        "chat_completions" | "completions" | "messages" | "responses"
    )
}

/// Extract normalized upstream usage from a response body.
///
/// Supports both OpenAI (`usage.prompt_tokens / completion_tokens / total_tokens`)
/// and Anthropic (`usage.input_tokens / output_tokens`) shapes.
fn extract_usage(body: &Value, provider: &Option<String>) -> Option<TokenUsage> {
    let provider_name = provider
        .as_deref()
        .map(normalize_provider_name)
        .unwrap_or("openai");
    parse_upstream_usage(body, provider_name)
}

fn normalize_provider_name(provider: &str) -> &str {
    if provider.contains("anthropic") {
        "anthropic"
    } else {
        "openai"
    }
}

async fn finalize_request_audit_success(
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

async fn finalize_request_audit_failure(
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

async fn finalize_request_audit_failure_snapshot(
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

async fn record_credential_model_success(ctx: &PipelineContext, state: &Arc<AppState>) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = ctx.selected_provider_id.clone() else {
        return;
    };
    let Some(model) = ctx
        .resolved_model
        .clone()
        .or_else(|| ctx.canonical_req.requested_model.clone())
    else {
        return;
    };

    let input = db::RecordCredentialModelSuccessInput {
        provider_account_id,
        provider_credential_id: ctx.selected_provider_credential_id.clone(),
        provider_credential_ref: ctx
            .selected_real_credential_ref
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone())
            .or_else(|| ctx.selected_provider_credential_id.clone()),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        model,
    };

    if let Err(error) = db::record_provider_credential_model_success(pg_pool, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to record credential-model success"
        );
    }
}

async fn record_credential_model_success_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = snapshot.provider_account_id.clone() else {
        return;
    };
    let Some(model) = snapshot
        .resolved_model
        .clone()
        .or_else(|| snapshot.requested_model.clone())
    else {
        return;
    };

    let input = db::RecordCredentialModelSuccessInput {
        provider_account_id,
        provider_credential_id: snapshot.provider_credential_ref.clone(),
        provider_credential_ref: snapshot
            .real_credential_ref
            .clone()
            .or_else(|| snapshot.provider_credential_ref.clone()),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        model,
    };

    if let Err(error) = db::record_provider_credential_model_success(pg_pool, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to record stream credential-model success"
        );
    }
}

async fn record_credential_model_failure(
    ctx: &PipelineContext,
    error: &crate::error::GatewayError,
    upstream_status: Option<u16>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = ctx.selected_provider_id.clone() else {
        return;
    };
    let Some(model) = ctx
        .resolved_model
        .clone()
        .or_else(|| ctx.canonical_req.requested_model.clone())
    else {
        return;
    };

    let classification = classify_provider_failure(
        upstream_status,
        error.code.as_deref(),
        Some(error.message.as_str()),
    );
    let input = db::RecordCredentialModelFailureInput {
        provider_account_id,
        provider_credential_id: ctx.selected_provider_credential_id.clone(),
        provider_credential_ref: ctx
            .selected_real_credential_ref
            .clone()
            .or_else(|| ctx.selected_platform_access_id.clone())
            .or_else(|| ctx.selected_provider_credential_id.clone()),
        protocol_profile: ctx.selected_protocol_profile.clone(),
        model,
        upstream_status,
        error_message: Some(error.message.clone()),
        classification,
    };

    if let Err(error) = db::record_provider_credential_model_failure(pg_pool, input).await {
        warn!(
            req_id = %ctx.req_id,
            error = %error,
            "failed to record credential-model failure"
        );
    }
}

async fn record_credential_model_failure_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    error_summary: Option<&str>,
    state: &Arc<AppState>,
) {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return;
    };
    let Some(provider_account_id) = snapshot.provider_account_id.clone() else {
        return;
    };
    let Some(model) = snapshot
        .resolved_model
        .clone()
        .or_else(|| snapshot.requested_model.clone())
    else {
        return;
    };

    let classification = classify_provider_failure(None, Some("stream_failed"), error_summary);
    let input = db::RecordCredentialModelFailureInput {
        provider_account_id,
        provider_credential_id: snapshot.provider_credential_ref.clone(),
        provider_credential_ref: snapshot
            .real_credential_ref
            .clone()
            .or_else(|| snapshot.provider_credential_ref.clone()),
        protocol_profile: snapshot.selected_protocol_profile.clone(),
        model,
        upstream_status: None,
        error_message: error_summary.map(str::to_string),
        classification,
    };

    if let Err(error) = db::record_provider_credential_model_failure(pg_pool, input).await {
        warn!(
            request_id = %snapshot.request_id,
            error = %error,
            "failed to record stream credential-model failure"
        );
    }
}

async fn finalize_request_audit(
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
        provider_account_id: ctx.selected_provider_id.clone(),
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

async fn finalize_request_audit_from_snapshot(
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

fn build_route_trace_from_ctx(
    ctx: &PipelineContext,
    error: Option<&crate::error::GatewayError>,
) -> Value {
    let attempted_provider_ids = ctx.attempted_provider_ids();

    let failure_classification = error.map(|value| {
        classify_provider_failure(
            value.http_status,
            value.code.as_deref(),
            Some(value.message.as_str()),
        )
    });

    json!({
        "requestedProtocolFamily": protocol_family_name(ctx),
        "endpointKind": endpoint_kind_name(ctx),
        "routeAttemptCount": ctx.route_attempt_count(),
        "candidateCount": ctx.candidates.len(),
        "attemptedProviderIds": attempted_provider_ids,
        "selectedPipelineMode": ctx.selected_execution_mode.clone(),
        "selectedProtocolProfile": ctx.selected_protocol_profile.clone(),
        "selectionStrategy": ctx.route_selection_strategy.clone(),
        "routeSelectionStrategy": ctx.route_selection_strategy.clone(),
        "accessKeyId": ctx.requesting_access_key_id.clone(),
        "sourceAccessKeyId": ctx.source_access_key_id.clone(),
        "platformAccessId": ctx.selected_platform_access_id.clone(),
        "realCredentialRef": ctx.selected_real_credential_ref.clone(),
        "selectedUpstreamTargetProtocolFamily": ctx
            .selected_upstream_target_protocol_family
            .clone(),
        "selectedUpstreamTargetConversationFamily": ctx
            .selected_upstream_target_conversation_family
            .clone(),
        "canonicalConversationSemantics": ctx.canonical_conversation_semantics.clone(),
        "selectedUpstreamTargetToolFamily": ctx.selected_upstream_target_tool_family.clone(),
        "toolStrategy": ctx.tool_strategy.clone(),
        "canonicalToolChoiceSemantics": ctx.canonical_tool_choice_semantics.clone(),
        "canonicalCompletionSemantics": ctx.canonical_completion_semantics.clone(),
        "selectedCandidate": build_selected_candidate_trace_from_ctx(ctx),
        "fallbackEligible": error.map(is_fallback_eligible),
        "errorCode": error.and_then(|value| value.code.clone()),
        "errorKind": error.map(|value| format!("{:?}", value.kind)),
        "failureClass": failure_classification.as_ref().map(|value| value.class_name()),
        "failureScope": failure_classification.as_ref().map(|value| value.scope_name()),
        "failurePermanent": failure_classification.as_ref().map(|value| value.permanent),
    })
}

fn build_route_trace_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    error_summary: Option<&str>,
) -> Value {
    let failure_classification = error_summary
        .map(|summary| classify_provider_failure(None, Some("stream_failed"), Some(summary)));
    json!({
        "requestedProtocolFamily": snapshot.protocol_family.clone(),
        "endpointKind": snapshot.endpoint_kind.clone(),
        "routeAttemptCount": snapshot.route_attempt_count,
        "attemptedProviderIds": snapshot.attempted_provider_ids.clone(),
        "selectedPipelineMode": snapshot.selected_execution_mode.clone(),
        "selectedProtocolProfile": snapshot.selected_protocol_profile.clone(),
        "selectionStrategy": snapshot.route_selection_strategy.clone(),
        "routeSelectionStrategy": snapshot.route_selection_strategy.clone(),
        "accessKeyId": snapshot.access_key_id.clone(),
        "sourceAccessKeyId": snapshot.source_access_key_id.clone(),
        "platformAccessId": snapshot.platform_access_id.clone(),
        "realCredentialRef": snapshot.real_credential_ref.clone(),
        "selectedUpstreamTargetProtocolFamily": snapshot
            .selected_upstream_target_protocol_family
            .clone(),
        "selectedUpstreamTargetConversationFamily": snapshot
            .selected_upstream_target_conversation_family
            .clone(),
        "canonicalConversationSemantics": snapshot.canonical_conversation_semantics.clone(),
        "selectedUpstreamTargetToolFamily": snapshot
            .selected_upstream_target_tool_family
            .clone(),
        "toolStrategy": snapshot.tool_strategy.clone(),
        "canonicalToolChoiceSemantics": snapshot.canonical_tool_choice_semantics.clone(),
        "canonicalCompletionSemantics": snapshot.canonical_completion_semantics.clone(),
        "selectedCandidate": build_selected_candidate_trace_from_snapshot(snapshot),
        "errorSummary": error_summary,
        "failureClass": failure_classification.as_ref().map(|value| value.class_name()),
        "failureScope": failure_classification.as_ref().map(|value| value.scope_name()),
        "failurePermanent": failure_classification.as_ref().map(|value| value.permanent),
    })
}

fn build_selected_candidate_trace_from_ctx(ctx: &PipelineContext) -> Option<Value> {
    let provider_account_id = ctx.selected_provider_id.as_ref()?;
    let mut trace = serde_json::Map::new();
    trace.insert("providerAccountId".to_string(), json!(provider_account_id));
    trace.insert(
        "platformAccessId".to_string(),
        json!(ctx.selected_platform_access_id.clone()),
    );
    trace.insert(
        "sourceAccessKeyId".to_string(),
        json!(ctx.source_access_key_id.clone()),
    );
    trace.insert(
        "realCredentialRef".to_string(),
        json!(ctx.selected_real_credential_ref.clone()),
    );
    trace.insert(
        "providerLabel".to_string(),
        json!(ctx.selected_provider_label.clone()),
    );
    trace.insert(
        "label".to_string(),
        json!(ctx.selected_provider_label.clone()),
    );
    trace.insert("adapter".to_string(), json!(ctx.selected_adapter.clone()));
    trace.insert(
        "protocolProfile".to_string(),
        json!(ctx.selected_protocol_profile.clone()),
    );
    trace.insert(
        "modelAlias".to_string(),
        json!(ctx.selected_model_alias.clone()),
    );
    trace.insert(
        "resolvedModel".to_string(),
        json!(ctx.resolved_model.clone()),
    );
    trace.insert(
        "resolvedExecutionMode".to_string(),
        json!(ctx.selected_execution_mode.clone()),
    );
    trace.insert(
        "executionMode".to_string(),
        json!(ctx.selected_execution_mode.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetProtocolFamily".to_string(),
        json!(ctx.selected_upstream_target_protocol_family.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetConversationFamily".to_string(),
        json!(ctx.selected_upstream_target_conversation_family.clone()),
    );
    trace.insert(
        "canonicalConversationSemantics".to_string(),
        json!(ctx.canonical_conversation_semantics.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetToolFamily".to_string(),
        json!(ctx.selected_upstream_target_tool_family.clone()),
    );
    trace.insert("toolStrategy".to_string(), json!(ctx.tool_strategy.clone()));
    trace.insert(
        "canonicalToolChoiceSemantics".to_string(),
        json!(ctx.canonical_tool_choice_semantics.clone()),
    );
    trace.insert(
        "canonicalCompletionSemantics".to_string(),
        json!(ctx.canonical_completion_semantics.clone()),
    );
    trace.insert(
        "routingScore".to_string(),
        json!(ctx.selected_routing_score),
    );
    trace.insert(
        "healthWeight".to_string(),
        json!(ctx.selected_health_weight),
    );
    trace.insert(
        "capacityWeight".to_string(),
        json!(ctx.selected_capacity_weight),
    );
    trace.insert("degraded".to_string(), json!(ctx.selected_degraded));
    trace.insert("breakerOpen".to_string(), json!(ctx.selected_breaker_open));
    if !ctx.selected_degradation_reasons.is_empty() {
        trace.insert(
            "degradationReasons".to_string(),
            json!(ctx.selected_degradation_reasons.clone()),
        );
    }
    Some(Value::Object(trace))
}

fn build_selected_candidate_trace_from_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
) -> Option<Value> {
    let provider_account_id = snapshot.provider_account_id.as_ref()?;
    let mut trace = serde_json::Map::new();
    trace.insert("providerAccountId".to_string(), json!(provider_account_id));
    trace.insert(
        "platformAccessId".to_string(),
        json!(snapshot.platform_access_id.clone()),
    );
    trace.insert(
        "sourceAccessKeyId".to_string(),
        json!(snapshot.source_access_key_id.clone()),
    );
    trace.insert(
        "realCredentialRef".to_string(),
        json!(snapshot.real_credential_ref.clone()),
    );
    trace.insert(
        "providerLabel".to_string(),
        json!(snapshot.selected_provider_label.clone()),
    );
    trace.insert(
        "label".to_string(),
        json!(snapshot.selected_provider_label.clone()),
    );
    trace.insert(
        "adapter".to_string(),
        json!(snapshot.selected_adapter.clone()),
    );
    trace.insert(
        "modelAlias".to_string(),
        json!(snapshot.model_alias.clone()),
    );
    trace.insert(
        "resolvedModel".to_string(),
        json!(snapshot.resolved_model.clone()),
    );
    trace.insert(
        "resolvedExecutionMode".to_string(),
        json!(snapshot.selected_execution_mode.clone()),
    );
    trace.insert(
        "executionMode".to_string(),
        json!(snapshot.selected_execution_mode.clone()),
    );
    trace.insert(
        "protocolProfile".to_string(),
        json!(snapshot.selected_protocol_profile.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetProtocolFamily".to_string(),
        json!(snapshot.selected_upstream_target_protocol_family.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetConversationFamily".to_string(),
        json!(snapshot
            .selected_upstream_target_conversation_family
            .clone()),
    );
    trace.insert(
        "canonicalConversationSemantics".to_string(),
        json!(snapshot.canonical_conversation_semantics.clone()),
    );
    trace.insert(
        "selectedUpstreamTargetToolFamily".to_string(),
        json!(snapshot.selected_upstream_target_tool_family.clone()),
    );
    trace.insert(
        "toolStrategy".to_string(),
        json!(snapshot.tool_strategy.clone()),
    );
    trace.insert(
        "canonicalToolChoiceSemantics".to_string(),
        json!(snapshot.canonical_tool_choice_semantics.clone()),
    );
    trace.insert(
        "canonicalCompletionSemantics".to_string(),
        json!(snapshot.canonical_completion_semantics.clone()),
    );
    trace.insert(
        "routingScore".to_string(),
        json!(snapshot.selected_routing_score),
    );
    trace.insert(
        "healthWeight".to_string(),
        json!(snapshot.selected_health_weight),
    );
    trace.insert(
        "capacityWeight".to_string(),
        json!(snapshot.selected_capacity_weight),
    );
    trace.insert("degraded".to_string(), json!(snapshot.selected_degraded));
    trace.insert(
        "breakerOpen".to_string(),
        json!(snapshot.selected_breaker_open),
    );
    if !snapshot.selected_degradation_reasons.is_empty() {
        trace.insert(
            "degradationReasons".to_string(),
            json!(snapshot.selected_degradation_reasons.clone()),
        );
    }
    Some(Value::Object(trace))
}

fn is_fallback_eligible(error: &crate::error::GatewayError) -> bool {
    matches!(
        &error.fallback_hint,
        crate::error::FallbackHint::FallbackProvider { .. }
            | crate::error::FallbackHint::Retry { .. }
    )
}

fn protocol_family_name(ctx: &PipelineContext) -> String {
    match ctx.canonical_req.protocol_family {
        crate::protocol::canonical::ProtocolFamily::OpenAi => match ctx.canonical_req.endpoint_kind
        {
            crate::protocol::canonical::EndpointKind::Responses => OPENAI_RESPONSES_FAMILY,
            crate::protocol::canonical::EndpointKind::Completions => {
                OPENAI_LEGACY_COMPLETIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::Embeddings => OPENAI_EMBEDDINGS_FAMILY,
            crate::protocol::canonical::EndpointKind::ImagesGenerations => {
                OPENAI_IMAGES_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::ImagesEdits => OPENAI_IMAGES_EDITS_FAMILY,
            crate::protocol::canonical::EndpointKind::MusicGenerations => {
                OPENAI_MUSIC_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::VideosGenerations => {
                OPENAI_VIDEOS_GENERATIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::AudioTranscriptions => {
                OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY
            }
            crate::protocol::canonical::EndpointKind::AudioSpeech => OPENAI_AUDIO_SPEECH_FAMILY,
            _ => OPENAI_CHAT_FAMILY,
        },
        crate::protocol::canonical::ProtocolFamily::OpenAiRealtime => OPENAI_REALTIME_FAMILY,
        crate::protocol::canonical::ProtocolFamily::Anthropic => ANTHROPIC_MESSAGES_FAMILY,
        crate::protocol::canonical::ProtocolFamily::GeminiGenerateContent => {
            "gemini_generate_content"
        }
        crate::protocol::canonical::ProtocolFamily::GeminiLive => "gemini_live",
        crate::protocol::canonical::ProtocolFamily::BedrockConverse => "bedrock_converse",
        crate::protocol::canonical::ProtocolFamily::CohereChat => "cohere_chat",
        crate::protocol::canonical::ProtocolFamily::SearchApi => SEARCH_API_FAMILY,
    }
    .to_string()
}

fn endpoint_kind_name(ctx: &PipelineContext) -> String {
    match ctx.canonical_req.endpoint_kind {
        crate::protocol::canonical::EndpointKind::ChatCompletions => "chat_completions",
        crate::protocol::canonical::EndpointKind::Completions => "completions",
        crate::protocol::canonical::EndpointKind::Embeddings => "embeddings",
        crate::protocol::canonical::EndpointKind::ImagesGenerations => "images_generations",
        crate::protocol::canonical::EndpointKind::ImagesEdits => "images_edits",
        crate::protocol::canonical::EndpointKind::MusicGenerations => "music_generations",
        crate::protocol::canonical::EndpointKind::VideosGenerations => "videos_generations",
        crate::protocol::canonical::EndpointKind::AudioTranscriptions => "audio_transcriptions",
        crate::protocol::canonical::EndpointKind::AudioSpeech => "audio_speech",
        crate::protocol::canonical::EndpointKind::Messages => "messages",
        crate::protocol::canonical::EndpointKind::Responses => "responses",
        crate::protocol::canonical::EndpointKind::Search => "search",
        crate::protocol::canonical::EndpointKind::Fetch => "fetch",
        crate::protocol::canonical::EndpointKind::ResearchCreate => "research_create",
        crate::protocol::canonical::EndpointKind::ResearchList => "research_list",
        crate::protocol::canonical::EndpointKind::ResearchGet => "research_get",
        crate::protocol::canonical::EndpointKind::CreditsBalance => "credits_balance",
    }
    .to_string()
}

fn extract_response_id(body: &Value) -> Option<String> {
    body.get("id").and_then(Value::as_str).map(str::to_string)
}

async fn settle_pre_deducted_quota(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    actual_total_tokens: u64,
) {
    let Some(credential_id) = ctx.quota_credential_id.as_deref() else {
        return;
    };
    if ctx.quota_pre_deducted_tokens == 0 {
        return;
    }
    if ctx.requesting_access_key_id.is_some() {
        let Some(pg_pool) = state.pg_pool.as_ref() else {
            return;
        };
        if let Err(error) = db::settle_access_key_balance(
            pg_pool,
            &state.redis_pool,
            credential_id,
            ctx.quota_pre_deducted_tokens,
            actual_total_tokens,
        )
        .await
        {
            warn!(
                req_id = %ctx.req_id,
                credential_id = %credential_id,
                error = %error,
                "failed to settle unified access key balance"
            );
        }
        return;
    }
    if let Err(error) = settle_quota_after_usage(
        &state.redis_pool,
        credential_id,
        ctx.quota_pre_deducted_tokens,
        actual_total_tokens,
    )
    .await
    {
        warn!(
            req_id = %ctx.req_id,
            credential_id = %credential_id,
            error = %error,
            "failed to settle quota after usage"
        );
    }
}

async fn settle_pre_deducted_quota_snapshot(
    snapshot: &RequestAuditFinalizeSnapshot,
    state: &Arc<AppState>,
    actual_total_tokens: u64,
) {
    let Some(credential_id) = snapshot.credential_id.as_deref() else {
        return;
    };
    if snapshot.pre_deducted_tokens == 0 {
        return;
    }
    if snapshot.access_key_id.is_some() {
        let Some(pg_pool) = state.pg_pool.as_ref() else {
            return;
        };
        if let Err(error) = db::settle_access_key_balance(
            pg_pool,
            &state.redis_pool,
            credential_id,
            snapshot.pre_deducted_tokens,
            actual_total_tokens,
        )
        .await
        {
            warn!(
                request_id = %snapshot.request_id,
                credential_id = %credential_id,
                error = %error,
                "failed to settle unified access key balance for stream success"
            );
        }
        return;
    }
    if let Err(error) = settle_quota_after_usage(
        &state.redis_pool,
        credential_id,
        snapshot.pre_deducted_tokens,
        actual_total_tokens,
    )
    .await
    {
        warn!(
            request_id = %snapshot.request_id,
            credential_id = %credential_id,
            error = %error,
            "failed to settle quota after stream usage"
        );
    }
}

async fn refund_pre_deducted_quota(ctx: &PipelineContext, state: &Arc<AppState>) {
    let Some(credential_id) = ctx.quota_credential_id.as_deref() else {
        return;
    };
    if ctx.quota_pre_deducted_tokens == 0 {
        return;
    }
    if ctx.requesting_access_key_id.is_some() {
        let Some(pg_pool) = state.pg_pool.as_ref() else {
            return;
        };
        if let Err(error) = db::refund_access_key_balance(
            pg_pool,
            &state.redis_pool,
            credential_id,
            ctx.quota_pre_deducted_tokens,
        )
        .await
        {
            warn!(
                req_id = %ctx.req_id,
                credential_id = %credential_id,
                error = %error,
                "failed to refund unified access key balance"
            );
        }
        return;
    }
    if let Err(error) = refund_quota(
        &state.redis_pool,
        credential_id,
        ctx.quota_pre_deducted_tokens,
    )
    .await
    {
        warn!(
            req_id = %ctx.req_id,
            credential_id = %credential_id,
            error = %error,
            "failed to refund pre-deducted quota"
        );
    }
}

async fn refund_pre_deducted_quota_snapshot(
    snapshot: &FailureFinalizeSnapshot,
    state: &Arc<AppState>,
) {
    let Some(credential_id) = snapshot.credential_id.as_deref() else {
        return;
    };
    if snapshot.pre_deducted_tokens == 0 {
        return;
    }
    if snapshot.request_audit.access_key_id.is_some() {
        let Some(pg_pool) = state.pg_pool.as_ref() else {
            return;
        };
        if let Err(error) = db::refund_access_key_balance(
            pg_pool,
            &state.redis_pool,
            credential_id,
            snapshot.pre_deducted_tokens,
        )
        .await
        {
            warn!(
                request_id = %snapshot.request_id,
                credential_id = %credential_id,
                error = %error,
                "failed to refund unified access key balance from snapshot"
            );
        }
        return;
    }
    if let Err(error) = refund_quota(
        &state.redis_pool,
        credential_id,
        snapshot.pre_deducted_tokens,
    )
    .await
    {
        warn!(
            request_id = %snapshot.request_id,
            credential_id = %credential_id,
            error = %error,
            "failed to refund stream pre-deducted quota"
        );
    }
}

/// Format an `Instant` as an approximate ISO-8601 UTC string by computing
/// the offset from the current time.
fn format_instant_as_iso(started_at: &std::time::Instant) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let elapsed = started_at.elapsed();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let started_secs = now_secs.saturating_sub(elapsed.as_secs());
    format_unix_secs_as_iso(started_secs)
}

fn format_now_as_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_secs_as_iso(secs)
}

fn format_unix_secs_as_iso(secs: u64) -> String {
    // Minimal UNIX → ISO-8601 converter (no external deps).
    let mut remaining = secs;
    let s = remaining % 60;
    remaining /= 60;
    let mi = remaining % 60;
    remaining /= 60;
    let h = remaining % 24;
    let mut days = (remaining / 24) as i64;

    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, d, h, mi, s
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::PipelineContext;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn make_ctx() -> PipelineContext {
        PipelineContext::new(
            CanonicalRelayRequest {
                protocol_family: ProtocolFamily::OpenAi,
                endpoint_kind: EndpointKind::ChatCompletions,
                requested_model: Some("gpt-4o".to_string()),
                stream: false,
                messages: vec![CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "hi".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                }],
                tools: vec![],
                tool_choice: None,
                reasoning: None,
                metadata: None,
                raw_body: json!({}),
                previous_response_id: None,
                explicit_session_key: None,
                extra: HashMap::new(),
            },
            None,
        )
    }

    #[test]
    fn extract_usage_openai_shape() {
        let body = json!({
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        });
        let usage = extract_usage(&body, &Some("openai_compatible".to_string())).unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
        assert_eq!(usage.cache_creation_input_tokens, None);
        assert_eq!(usage.cache_read_input_tokens, None);
    }

    #[test]
    fn extract_usage_anthropic_shape() {
        let body = json!({
            "usage": {
                "input_tokens": 15,
                "output_tokens": 25,
                "cache_creation_input_tokens": 200,
                "cache_read_input_tokens": 120
            }
        });
        let usage = extract_usage(&body, &Some("anthropic_compatible".to_string())).unwrap();
        assert_eq!(usage.prompt_tokens, 15);
        assert_eq!(usage.completion_tokens, 25);
        assert_eq!(usage.total_tokens, 40);
        assert_eq!(usage.cache_creation_input_tokens, Some(200));
        assert_eq!(usage.cache_read_input_tokens, Some(120));
    }

    #[test]
    fn extract_usage_missing_returns_zeros() {
        let body = json!({ "choices": [] });
        assert!(extract_usage(&body, &Some("openai_compatible".to_string())).is_none());
    }

    #[test]
    fn extract_usage_openai_infers_total() {
        let body = json!({
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 7
            }
        });
        let usage = extract_usage(&body, &Some("openai_compatible".to_string())).unwrap();
        assert_eq!(usage.total_tokens, 12);
    }

    #[test]
    fn format_unix_secs_as_iso_format() {
        // 2026-04-07T00:00:00Z  →  unix = 1744156800 (approximately)
        // We just check the format is correct length and ends with Z.
        let s = format_unix_secs_as_iso(1_744_156_800);
        assert!(s.ends_with('Z'), "expected Z suffix, got: {s}");
        assert_eq!(s.len(), 20, "unexpected format: {s}");
    }

    #[test]
    fn extract_response_id_reads_top_level_id() {
        let body = json!({
            "id": "resp_123",
            "object": "response"
        });
        assert_eq!(extract_response_id(&body).as_deref(), Some("resp_123"));
    }

    #[test]
    fn route_trace_includes_first_class_protocol_semantics() {
        let mut ctx = make_ctx();
        ctx.selected_provider_id = Some("prov-1".to_string());
        ctx.selected_provider_label = Some("Provider".to_string());
        ctx.selected_adapter = Some("openai_compatible".to_string());
        ctx.selected_execution_mode = Some("direct_http".to_string());
        ctx.selected_upstream_target_protocol_family = Some("openai_responses".to_string());
        ctx.selected_upstream_target_conversation_family = Some("openai_responses".to_string());
        ctx.canonical_conversation_semantics = Some("messages_turns".to_string());
        ctx.selected_upstream_target_tool_family = Some("xml_fallback".to_string());
        ctx.tool_strategy = Some("xml_fallback".to_string());
        ctx.canonical_tool_choice_semantics = Some("prompt_only".to_string());
        ctx.canonical_completion_semantics = Some("tool_calls".to_string());

        let trace = build_route_trace_from_ctx(&ctx, None);

        assert_eq!(
            trace["selectedUpstreamTargetProtocolFamily"],
            "openai_responses"
        );
        assert_eq!(
            trace["selectedUpstreamTargetConversationFamily"],
            "openai_responses"
        );
        assert_eq!(trace["canonicalConversationSemantics"], "messages_turns");
        assert_eq!(trace["selectedUpstreamTargetToolFamily"], "xml_fallback");
        assert_eq!(trace["toolStrategy"], "xml_fallback");
        assert_eq!(trace["canonicalToolChoiceSemantics"], "prompt_only");
        assert_eq!(trace["canonicalCompletionSemantics"], "tool_calls");
        assert_eq!(
            trace["selectedCandidate"]["canonicalToolChoiceSemantics"],
            "prompt_only"
        );
    }

    #[test]
    fn route_trace_separates_retry_count_from_actual_provider_ids() {
        let ctx = make_ctx();
        ctx.note_provider_attempt("provider-b");
        ctx.note_provider_attempt("provider-b");

        let trace = build_route_trace_from_ctx(&ctx, None);

        assert_eq!(trace["routeAttemptCount"], 2);
        assert_eq!(trace["attemptedProviderIds"], json!(["provider-b"]));
    }

    #[test]
    fn protocol_family_name_maps_openai_embeddings_to_explicit_family() {
        let mut ctx = make_ctx();
        ctx.canonical_req.endpoint_kind = EndpointKind::Embeddings;
        assert_eq!(protocol_family_name(&ctx), OPENAI_EMBEDDINGS_FAMILY);
    }

    #[test]
    fn apply_stream_completion_semantics_updates_snapshot_route_trace() {
        let ctx = make_ctx();
        let mut snapshot = snapshot_request_audit(&ctx);
        assert_eq!(snapshot.canonical_completion_semantics, None);

        apply_stream_completion_semantics(&mut snapshot, Some("length".to_string()));
        let trace = build_route_trace_from_snapshot(&snapshot, None);

        assert_eq!(trace["canonicalCompletionSemantics"], "length");
    }
}
