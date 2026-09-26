//! Buffered-response usage, finalization and successful credential affinity.

use super::*;

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

/// Extract normalized upstream usage from a response body.
///
/// Supports both OpenAI (`usage.prompt_tokens / completion_tokens / total_tokens`)
/// and Anthropic (`usage.input_tokens / output_tokens`) shapes.
pub(super) fn extract_usage(body: &Value, provider: &Option<String>) -> Option<TokenUsage> {
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
