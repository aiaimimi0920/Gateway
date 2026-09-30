// ---------------------------------------------------------------------------
// Pipeline stage 4 — upstream dispatch with retry and provider fallback
//
// Iterates the candidate queue, acquires a concurrency permit per candidate,
// then either:
//   - non-streaming: calls execute_with_retry → returns PipelineOutput::Json
//   - streaming:     calls execute_stream     → wraps in TrackedStream
//                                               → returns PipelineOutput::Sse
//
// If a candidate fails, the next one in the queue is tried.  The outer loop
// provides provider-level fallback; the inner retry loop (execute_with_retry)
// handles per-provider transient failures.
// ---------------------------------------------------------------------------

mod tool_stream;

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use futures::StreamExt;
use tracing::{debug, warn};

use crate::concurrency::aimd::FailureKind;
use crate::db;
use crate::error::{classify_upstream_error, FallbackHint, GatewayError};
use crate::keepalive;
use crate::metrics::request::{global_gateway_metrics, GatewayMetrics, ProviderMetricOutcome};
use crate::protocol::accio;
use crate::protocol::anthropic;
use crate::protocol::canonical::{EndpointKind, MessageRole, ProtocolFamily};
use crate::protocol::chatgpt::web_reverse as chatgpt_web;
use crate::protocol::freebuff;
use crate::protocol::grok;
use crate::protocol::kiro;
use crate::protocol::openai;
use crate::protocol::registry::{
    CHATAIBOT_IMAGES_FAMILY, GEMINI_BUSINESS_IMAGES_FAMILY, GEMINI_CANVAS_IMAGES_FAMILY,
    GEMINI_CANVAS_MUSIC_FAMILY, GEMINI_CANVAS_VIDEOS_FAMILY, GEMINI_GENERATE_CONTENT_FAMILY,
    LUMALABS_AUDIO_FAMILY, LUMALABS_IMAGES_FAMILY, LUMALABS_VIDEOS_FAMILY,
    OPENAI_AUDIO_SPEECH_FAMILY, OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY, OPENAI_EMBEDDINGS_FAMILY,
    OPENAI_IMAGES_EDITS_FAMILY, OPENAI_IMAGES_GENERATIONS_FAMILY, OPENAI_MUSIC_GENERATIONS_FAMILY,
    OPENAI_VIDEOS_GENERATIONS_FAMILY, PRODUCER_IMAGES_FAMILY, PRODUCER_MUSIC_FAMILY,
    PRODUCER_VIDEOS_FAMILY, SUNO_IMAGES_FAMILY, SUNO_MUSIC_FAMILY, SUNO_VIDEOS_FAMILY,
    UDIO_IMAGES_FAMILY, UDIO_MUSIC_FAMILY, UDIO_VIDEOS_FAMILY,
};
use crate::protocol::responses;
use crate::protocol::tool_choice::{self, CanonicalToolChoice};
use crate::protocol::tool_inject;
use crate::provider_failure::classify_provider_failure;
use crate::provider_runtime;
use crate::rate_limit::ProviderRateLimitRejections;
use crate::retry::{
    execute_with_retry_after_admission_observed, RetryAttemptObservation, RetryPolicy,
};
use crate::state::AppState;
use crate::upstream::response_types::UpstreamStreamingResponse;
use crate::upstream::stream::{
    snapshot_tapped_archive, snapshot_tapped_completion_semantics, snapshot_tapped_usage,
    tap_sse_completion_semantics, tap_sse_usage, tap_stream_archive, TrackedStream,
};
use crate::upstream::upstream_model_helpers;

use super::{BinaryPipelineResponse, PipelineContext, PipelineOutput};
use tool_stream::{tool_detection_placement, wrap_injected_openai_stream};

use super::stage_rate_limit;
use attempt::{AttemptError, ByteStream, PreparedAttempt, UsageHandle};
mod attempt;
mod buffered;
mod chatgpt_policy;
mod chatgpt_recovery;
mod endpoint_policy;
mod feedback;
mod pack;
mod qwen_recovery;
mod responses_bridge;
mod selection;
mod stream_preflight;
mod stream_translation;
mod streaming;
mod tool_bridge;
use chatgpt_policy::{
    chatgpt_web_browser_fallback_forbidden_error, chatgpt_web_request_time_browser_allowed,
    chatgpt_web_request_time_browser_path_needed, should_escalate_chatgpt_web_to_browser_relay,
    should_refresh_chatgpt_web_after_failure,
};
use chatgpt_recovery::{
    execute_chatgpt_web_nonstream_with_recovery, execute_chatgpt_web_stream_with_recovery,
};
use endpoint_policy::{
    expects_binary_passthrough, expects_json_passthrough, is_conversation_endpoint,
    observe_provider_rate_limit_rejection,
};
use feedback::{
    classify_failure_kind, observe_provider_attempt_metric, observe_provider_failure_metric,
    observe_provider_result_metric, observe_provider_success_metric, retry_policy_for_request,
    should_record_provider_failure, should_try_next_candidate, spawn_record_provider_failure,
    spawn_record_provider_success,
};
use pack::{caller_visible_reply_model, pack_response};
use qwen_recovery::{
    execute_qwen_web_nonstream_with_recovery, execute_qwen_web_stream_with_recovery,
};
use selection::{infer_canonical_completion_semantics, set_selected_candidate};
use tool_bridge::{
    capture_prompt_cache_telemetry, maybe_apply_xml_tool_response_bridge,
    should_force_bridge_tool_injection,
};

/// Execute the upstream call with provider fallback.
///
/// Iterates the candidate queue built by [`stage_route`].  For each candidate:
/// 1. Acquires an AIMD concurrency permit.
/// 2. Dispatches the request (streaming or not).
/// 3. On success, feeds back to the AIMD controller and returns.
/// 4. On failure, determines whether to try the next candidate or abort.
pub async fn run(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    if matches!(
        ctx.canonical_req.endpoint_kind,
        EndpointKind::Embeddings
            | EndpointKind::ImagesGenerations
            | EndpointKind::ImagesEdits
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
            | EndpointKind::AudioTranscriptions
            | EndpointKind::AudioSpeech
            | EndpointKind::Search
            | EndpointKind::Fetch
            | EndpointKind::ResearchCreate
            | EndpointKind::ResearchList
            | EndpointKind::ResearchGet
            | EndpointKind::CreditsBalance
    ) && ctx.canonical_req.stream
    {
        return Err(GatewayError::bad_request(
            "This endpoint does not support streaming responses",
        ));
    }

    if ctx.candidates.is_empty() {
        return Err(GatewayError::server_error(
            "no provider candidates available for this request",
        ));
    }

    let mut last_error: Option<GatewayError> = None;
    let mut provider_rate_limit_rejections = ProviderRateLimitRejections::default();

    // Clone candidates so we can iterate without holding a mutable ref.
    let candidates = ctx.candidates.clone();

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let route_policy_config = ctx.route_policy_config.clone();
        let provider_attempt_gate = super::stage_rate_limit::provider_attempt_gate(
            ctx,
            state,
            &candidate.provider_account_id,
        )?;
        if ctx
            .session
            .as_ref()
            .and_then(|session| session.access_key_kind.as_deref())
            == Some("auto_route")
        {
            if let Some(projected_row) = ctx.projected_access_candidates.get(candidate_index) {
                if ctx.quota_credential_id.as_deref()
                    != Some(projected_row.source_access_key_id.as_str())
                {
                    if let (Some(previous_access_key_id), Some(pg_pool)) =
                        (ctx.quota_credential_id.as_deref(), state.pg_pool.as_ref())
                    {
                        let _ = db::refund_access_key_balance(
                            pg_pool,
                            &state.redis_pool,
                            previous_access_key_id,
                            ctx.quota_pre_deducted_tokens,
                        )
                        .await;
                    }
                    if let Some(pg_pool) = state.pg_pool.as_ref() {
                        let estimated = crate::redis::usage_tracking::estimate_token_count(
                            &ctx.canonical_req.messages_text(),
                        )
                        .max(1);
                        let decision = db::pre_deduct_access_key_balance(
                            pg_pool,
                            &state.redis_pool,
                            &projected_row.source_access_key_id,
                            estimated,
                        )
                        .await?;
                        if !decision.allowed {
                            last_error = Some(
                                GatewayError::quota_exceeded("当前自动路由 key 的候选额度不足")
                                    .with_code(
                                        decision
                                            .reason
                                            .unwrap_or_else(|| "balance_not_allowed".to_string()),
                                    ),
                            );
                            continue;
                        }
                        ctx.quota_credential_id = Some(projected_row.source_access_key_id.clone());
                        ctx.quota_pre_deducted_tokens = decision.pre_deduct_amount;
                    }
                }
            }
        }
        debug!(
            req_id = %ctx.req_id,
            provider = %candidate.provider_account_id,
            label = %candidate.label,
            "trying candidate"
        );

        // Resolve model name.
        // Priority: candidate.upstream_model (alias-resolved) > req.requested_model > payload.default_model
        let model = candidate
            .upstream_model
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                upstream_model_helpers::resolve_model(&candidate.payload, &ctx.canonical_req)
            })
            .to_string();
        let reply_model = caller_visible_reply_model(&ctx.canonical_req, &model);
        if let (Some(local), Some(audit_id)) = (&state.local_runtime, &ctx.request_audit_id) {
            if let Err(error) = local
                .route_audit(audit_id, &candidate.provider_account_id, &model)
                .await
            {
                warn!(code = ?error.code, "Failed to attribute local running request");
            }
        }

        let remote_browser_executor_configured = state
            .upstream_client
            .browser_executor_runtime_health()
            .remote_base_url
            .is_some();
        let browser_session_owns_auth = remote_browser_executor_configured
            && matches!(
                candidate.resolved_execution_mode,
                crate::routing::candidate::ProviderExecutionMode::BrowserBacked
            )
            && matches!(
                candidate.adapter.as_str(),
                "suno_compatible" | "udio_compatible"
            );
        let mut effective_payload = if browser_session_owns_auth {
            candidate.payload.clone()
        } else {
            match keepalive::ensure_payload_ready(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                state.upstream_client.client(),
                &candidate.payload,
                ctx.session
                    .as_ref()
                    .map(|session| session.project_id.as_str()),
                ctx.canonical_req.explicit_session_key.as_deref(),
                ctx.canonical_req.previous_response_id.as_deref(),
                &candidate.provider_account_id,
                &model,
            )
            .await
            {
                Ok(payload) => payload,
                Err(e) => {
                    if should_try_next_candidate(&e) {
                        warn!(
                            req_id = %ctx.req_id,
                            provider = %candidate.provider_account_id,
                            error = %e,
                            "credential keepalive preflight failed; trying next"
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        };

        // Keepalive/session refresh may rewrite runtime auth material, but it
        // must not silently change which adapter/protocol send path is used.
        effective_payload.adapter = candidate.adapter.clone();
        let retry_policy = retry_policy_for_request(&ctx.canonical_req, &effective_payload);

        let original_tools = ctx.canonical_req.tools.clone();
        let original_tool_choice = ctx.canonical_req.tool_choice.clone();
        let original_messages_text = ctx.canonical_req.messages_text();

        // ── XML tool injection for models without native tool support ────
        let text_only_tool_bridge = tool_inject::needs_tool_injection(&model, &candidate.adapter)
            || should_force_bridge_tool_injection(&ctx.canonical_req, &candidate.adapter);
        let has_tool_history = ctx
            .canonical_req
            .messages
            .iter()
            .any(|message| !message.tool_calls.is_empty() || message.role == MessageRole::Tool);
        let tools_were_injected = !ctx.canonical_req.tools.is_empty() && text_only_tool_bridge;

        // Acquire concurrency before admission so a queued/cancelled request does
        // not consume a provider-attempt slot before it can be sent.
        let controller = state
            .concurrency_registry
            .get_or_create(&candidate.provider_account_id);
        let permit = controller.acquire().await;
        if let Err(error) = provider_attempt_gate.admit().await {
            drop(permit);
            if observe_provider_rate_limit_rejection(&mut provider_rate_limit_rejections, &error) {
                warn!(
                    req_id = %ctx.req_id,
                    provider = %candidate.provider_account_id,
                    "provider attempt rejected by route-policy rate limit"
                );
                continue;
            }
            return Err(error);
        }

        if tools_were_injected {
            debug!(
                req_id = %ctx.req_id,
                model = %model,
                adapter = %candidate.adapter,
                "injecting XML tool definitions (model lacks native tool support)"
            );
            tool_inject::inject_tools(&mut ctx.canonical_req);
            ctx.tools_were_injected = true;
        } else if text_only_tool_bridge && has_tool_history {
            debug!(
                req_id = %ctx.req_id,
                model = %model,
                adapter = %candidate.adapter,
                "serializing historical tool transcript for text-only upstream bridge"
            );
            tool_inject::serialize_tool_history(&mut ctx.canonical_req);
        }

        capture_prompt_cache_telemetry(ctx, &candidate.adapter, &model);

        let attempt = PreparedAttempt {
            route_policy_config,
            provider_attempt_gate,
            model,
            reply_model,
            effective_payload,
            retry_policy,
            original_tools,
            original_tool_choice,
            original_messages_text,
            tools_were_injected,
            controller,
            permit,
        };
        let result = if ctx.canonical_req.stream {
            streaming::send(ctx, state, candidate, candidate_index, attempt).await
        } else {
            buffered::send(ctx, state, candidate, candidate_index, attempt).await
        };
        match result {
            Ok(output) => return Ok(output),
            Err(AttemptError::Next(error)) => {
                last_error = Some(error);
                continue;
            }
            Err(AttemptError::Stop(error)) => return Err(error),
        }
    }

    // All candidates exhausted.
    Err(provider_rate_limit_rejections
        .resolve_terminal_error(last_error)
        .unwrap_or_else(|| GatewayError::server_error("all provider candidates exhausted")))
}

#[cfg(test)]
mod tests;
