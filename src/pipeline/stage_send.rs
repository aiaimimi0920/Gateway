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

        let mut effective_payload = match keepalive::ensure_payload_ready(
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

        if ctx.canonical_req.stream {
            // ── Streaming path ────────────────────────────────────────────
            // For tool-injected streaming, the byte stream is wrapped with
            // a tool call detector (see wrap_streaming_tool_detection) that
            // accumulates text, checks for XML tool calls on stream end,
            // and emits tool_call deltas + [DONE] if found.

            let stream_attempt_started_at = Instant::now();
            let stream_result = if candidate.adapter == "freebuff_compatible" {
                freebuff::execute_stream(
                    state.upstream_client.client(),
                    &effective_payload,
                    &ctx.canonical_req,
                    &model,
                    Some(&ctx.request_headers),
                )
                .await
                .map(|execution| {
                    (
                        UpstreamStreamingResponse::Http(execution.response),
                        Some(execution.lease_handle),
                        stream_attempt_started_at,
                    )
                })
            } else if candidate.adapter == "qwen_web_compatible" {
                execute_qwen_web_stream_with_recovery(
                    state,
                    candidate,
                    &provider_attempt_gate,
                    &effective_payload,
                    &ctx.canonical_req,
                    &model,
                    &ctx.request_headers,
                )
                .await
                .map(|(response, started_at)| (response, None, started_at))
            } else if candidate.adapter == "chatgpt_web_reverse_compatible" {
                execute_chatgpt_web_stream_with_recovery(
                    state,
                    candidate,
                    &provider_attempt_gate,
                    &effective_payload,
                    &ctx.canonical_req,
                    &model,
                    &ctx.request_headers,
                )
                .await
                .map(|(response, started_at)| (response, None, started_at))
            } else {
                state
                    .upstream_client
                    .execute_stream(
                        &effective_payload,
                        &ctx.canonical_req,
                        &model,
                        Some(&ctx.request_headers),
                    )
                    .await
                    .map(|response| (response, None, stream_attempt_started_at))
            };

            match stream_result {
                Ok((stream_response, freebuff_lease_handle, stream_started_at)) => {
                    let byte_stream: std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                    > = match stream_response {
                        UpstreamStreamingResponse::Bytes(stream) => stream,
                        UpstreamStreamingResponse::Http(response)
                            if matches!(
                                candidate.adapter.as_str(),
                                "accio_compatible"
                                    | "gemini_api_compatible"
                                    | "bedrock_converse_compatible"
                                    | "cohere_compatible"
                            ) =>
                        {
                            let mut upstream = Box::pin(response.bytes_stream())
                                as std::pin::Pin<
                                    Box<
                                        dyn futures::Stream<
                                                Item = Result<bytes::Bytes, rquest::Error>,
                                            > + Send,
                                    >,
                                >;

                            let first = upstream.next().await;
                            let first_chunk = match first {
                                Some(Ok(chunk)) => chunk,
                                Some(Err(err)) => {
                                    let error = crate::error::classify_network_error(
                                        &err,
                                        Some(&candidate.adapter),
                                    );
                                    let failure_kind = classify_failure_kind(&error);
                                    controller.on_failure(failure_kind);
                                    observe_provider_failure_metric(
                                        global_gateway_metrics().as_ref(),
                                        &candidate.provider_account_id,
                                        &model,
                                        stream_started_at.elapsed().as_millis() as u64,
                                        &error.message,
                                    );
                                    spawn_record_provider_failure(
                                        Arc::clone(state),
                                        candidate.provider_account_id.clone(),
                                        candidate.provider_credential_id.clone(),
                                        route_policy_config.clone(),
                                        error.message.clone(),
                                    );
                                    drop(permit);

                                    if should_try_next_candidate(&error) {
                                        warn!(
                                            req_id = %ctx.req_id,
                                            provider = %candidate.provider_account_id,
                                            error = %error,
                                            "candidate failed during generic tool-stream preflight; trying next"
                                        );
                                        last_error = Some(error);
                                        continue;
                                    } else {
                                        return Err(error);
                                    }
                                }
                                None => {
                                    let error = GatewayError::server_error(
                                        "Streaming adapter ended before any events were received",
                                    )
                                    .with_provider(candidate.adapter.as_str());
                                    let failure_kind = classify_failure_kind(&error);
                                    controller.on_failure(failure_kind);
                                    observe_provider_failure_metric(
                                        global_gateway_metrics().as_ref(),
                                        &candidate.provider_account_id,
                                        &model,
                                        stream_started_at.elapsed().as_millis() as u64,
                                        &error.message,
                                    );
                                    spawn_record_provider_failure(
                                        Arc::clone(state),
                                        candidate.provider_account_id.clone(),
                                        candidate.provider_credential_id.clone(),
                                        route_policy_config.clone(),
                                        error.message.clone(),
                                    );
                                    drop(permit);

                                    if should_try_next_candidate(&error) {
                                        warn!(
                                            req_id = %ctx.req_id,
                                            provider = %candidate.provider_account_id,
                                            error = %error,
                                            "candidate returned an empty tool stream; trying next"
                                        );
                                        last_error = Some(error);
                                        continue;
                                    } else {
                                        return Err(error);
                                    }
                                }
                            };

                            if let Some(error) =
                                accio::detect_accio_provider_error(first_chunk.as_ref())
                            {
                                let failure_kind = classify_failure_kind(&error);
                                controller.on_failure(failure_kind);
                                observe_provider_failure_metric(
                                    global_gateway_metrics().as_ref(),
                                    &candidate.provider_account_id,
                                    &model,
                                    stream_started_at.elapsed().as_millis() as u64,
                                    &error.message,
                                );
                                spawn_record_provider_failure(
                                    Arc::clone(state),
                                    candidate.provider_account_id.clone(),
                                    candidate.provider_credential_id.clone(),
                                    route_policy_config.clone(),
                                    error.message.clone(),
                                );
                                drop(permit);

                                if should_try_next_candidate(&error) {
                                    warn!(
                                        req_id = %ctx.req_id,
                                        provider = %candidate.provider_account_id,
                                        error = %error,
                                        "streaming tool-adapter preflight failed; trying next"
                                    );
                                    last_error = Some(error);
                                    continue;
                                } else {
                                    return Err(error);
                                }
                            }

                            let replay = futures::stream::once(async move {
                                Ok::<bytes::Bytes, rquest::Error>(first_chunk)
                            })
                            .chain(upstream);

                            Box::pin(accio::translate_anthropic_like_stream_to_openai(
                                replay,
                                model.clone(),
                            ))
                        }
                        UpstreamStreamingResponse::Http(response)
                            if candidate.adapter == "grok_compatible" =>
                        {
                            Box::pin(grok::translate_grok_stream(
                                response.bytes_stream(),
                                model.clone(),
                            ))
                        }
                        UpstreamStreamingResponse::Http(response) => {
                            Box::pin(response.bytes_stream())
                        }
                    };

                    let (byte_stream, stream_usage_handle) =
                        if candidate.adapter == "kiro_compatible" {
                            (byte_stream, None)
                        } else {
                            let (tapped_stream, usage_handle) = tap_sse_usage(byte_stream);
                            (
                                Box::pin(tapped_stream)
                                    as std::pin::Pin<
                                        Box<
                                            dyn futures::Stream<
                                                    Item = Result<bytes::Bytes, rquest::Error>,
                                                > + Send,
                                        >,
                                    >,
                                Some(usage_handle),
                            )
                        };

                    // If tools were injected, wrap the stream to detect and
                    // translate XML tool calls in the streamed response.
                    let byte_stream: std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                    > = if tools_were_injected && candidate.adapter != "anthropic_compatible" {
                        let resp_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
                        debug!(
                            req_id = %ctx.req_id,
                            "wrapping stream with XML tool call detector"
                        );
                        tool_inject::wrap_streaming_tool_detection(
                            byte_stream,
                            reply_model.clone(),
                            resp_id,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        )
                    } else {
                        byte_stream
                    };

                    // If the client called /v1/messages (Anthropic native) but
                    // the upstream is NOT anthropic_compatible, the stream is in
                    // OpenAI SSE format and must be translated to Anthropic SSE.
                    let uses_openai_responses_bridge = candidate
                        .payload
                        .bridges_openai_text_endpoint_to_responses(ctx.canonical_req.endpoint_kind);

                    let byte_stream: std::pin::Pin<
                        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                    > = if candidate.adapter == "kiro_compatible" {
                        if ctx.canonical_req.endpoint_kind == EndpointKind::Messages {
                            debug!(
                                req_id = %ctx.req_id,
                                "translating Kiro event stream to Anthropic SSE format"
                            );
                            Box::pin(kiro::translate_kiro_event_stream_to_anthropic_sse(
                                byte_stream,
                                reply_model.clone(),
                                ctx.canonical_req.clone(),
                            ))
                        } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses {
                            debug!(
                                req_id = %ctx.req_id,
                                "translating Kiro event stream to OpenAI Responses SSE format"
                            );
                            Box::pin(responses::translate_openai_sse_to_responses(
                                kiro::translate_kiro_event_stream_to_openai_sse(
                                    byte_stream,
                                    reply_model.clone(),
                                    ctx.canonical_req.clone(),
                                ),
                                reply_model.clone(),
                            ))
                        } else {
                            debug!(
                                req_id = %ctx.req_id,
                                "translating Kiro event stream to OpenAI SSE format"
                            );
                            Box::pin(kiro::translate_kiro_event_stream_to_openai_sse(
                                byte_stream,
                                reply_model.clone(),
                                ctx.canonical_req.clone(),
                            ))
                        }
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::ChatCompletions
                        && uses_openai_responses_bridge
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating OpenAI Responses SSE stream to OpenAI chat SSE format"
                        );
                        Box::pin(responses::translate_responses_sse_to_openai_chat(
                            byte_stream,
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::ChatCompletions
                        && candidate.adapter == "anthropic_compatible"
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating Anthropic SSE stream to OpenAI SSE format"
                        );
                        let openai_stream = accio::translate_anthropic_like_stream_to_openai(
                            byte_stream,
                            reply_model.clone(),
                        );
                        wrap_injected_openai_stream(
                            Box::pin(openai_stream),
                            tools_were_injected,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        )
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses
                        && candidate.adapter == "anthropic_compatible"
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating Anthropic SSE stream to OpenAI Responses SSE format"
                        );
                        let openai_stream = accio::translate_anthropic_like_stream_to_openai(
                            byte_stream,
                            reply_model.clone(),
                        );
                        let openai_stream = wrap_injected_openai_stream(
                            Box::pin(openai_stream),
                            tools_were_injected,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        );
                        Box::pin(responses::translate_openai_sse_to_responses(
                            openai_stream,
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Messages
                        && uses_openai_responses_bridge
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating OpenAI Responses SSE stream to Anthropic SSE format"
                        );
                        let openai_stream = responses::translate_responses_sse_to_openai_chat(
                            byte_stream,
                            reply_model.clone(),
                        );
                        let openai_stream = wrap_injected_openai_stream(
                            Box::pin(openai_stream),
                            tools_were_injected,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        );
                        Box::pin(anthropic::translate_openai_sse_to_anthropic(
                            openai_stream,
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Messages
                        && candidate.adapter != "anthropic_compatible"
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating OpenAI SSE stream to Anthropic SSE format"
                        );
                        let openai_stream = wrap_injected_openai_stream(
                            byte_stream,
                            tools_were_injected,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        );
                        Box::pin(anthropic::translate_openai_sse_to_anthropic(
                            openai_stream,
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions
                        && uses_openai_responses_bridge
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating OpenAI Responses SSE stream to legacy completions SSE format"
                        );
                        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
                            responses::translate_responses_sse_to_openai_chat(
                                byte_stream,
                                reply_model.clone(),
                            ),
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions
                        && candidate.adapter == "anthropic_compatible"
                    {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating Anthropic SSE stream to legacy completions SSE format"
                        );
                        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
                            accio::translate_anthropic_like_stream_to_openai(
                                byte_stream,
                                reply_model.clone(),
                            ),
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Completions {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating OpenAI chat SSE stream to legacy completions SSE format"
                        );
                        Box::pin(openai::translate_openai_chat_sse_to_legacy_completions(
                            byte_stream,
                            reply_model.clone(),
                        ))
                    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses {
                        debug!(
                            req_id = %ctx.req_id,
                            adapter = %candidate.adapter,
                            "translating stream to OpenAI Responses SSE format"
                        );
                        let openai_stream = if uses_openai_responses_bridge || tools_were_injected {
                            Box::pin(responses::translate_responses_sse_to_openai_chat(
                                byte_stream,
                                reply_model.clone(),
                            ))
                                as std::pin::Pin<
                                    Box<
                                        dyn futures::Stream<
                                                Item = Result<bytes::Bytes, rquest::Error>,
                                            > + Send,
                                    >,
                                >
                        } else {
                            byte_stream
                        };
                        let openai_stream = wrap_injected_openai_stream(
                            openai_stream,
                            tools_were_injected,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        );
                        Box::pin(responses::translate_openai_sse_to_responses(
                            openai_stream,
                            reply_model.clone(),
                        ))
                    } else {
                        byte_stream
                    };

                    let (byte_stream, stream_usage_handle) =
                        if candidate.adapter == "kiro_compatible" {
                            let (tapped_stream, usage_handle) = tap_sse_usage(byte_stream);
                            (
                                Box::pin(tapped_stream)
                                    as std::pin::Pin<
                                        Box<
                                            dyn futures::Stream<
                                                    Item = Result<bytes::Bytes, rquest::Error>,
                                                > + Send,
                                        >,
                                    >,
                                Some(usage_handle),
                            )
                        } else {
                            (byte_stream, stream_usage_handle)
                        };

                    let (byte_stream, stream_completion_semantics_handle) =
                        if is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
                            let (tapped_stream, completion_semantics_handle) =
                                tap_sse_completion_semantics(byte_stream);
                            (
                                Box::pin(tapped_stream)
                                    as std::pin::Pin<
                                        Box<
                                            dyn futures::Stream<
                                                    Item = Result<bytes::Bytes, rquest::Error>,
                                                > + Send,
                                        >,
                                    >,
                                Some(completion_semantics_handle),
                            )
                        } else {
                            (byte_stream, None)
                        };

                    let (byte_stream, stream_archive_handle) =
                        if is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
                            let (tapped_stream, archive_handle) = tap_stream_archive(
                            byte_stream,
                            crate::conversation_archive::CONVERSATION_ARCHIVE_MAX_RESPONSE_BYTES,
                        );
                            (
                                Box::pin(tapped_stream)
                                    as std::pin::Pin<
                                        Box<
                                            dyn futures::Stream<
                                                    Item = Result<bytes::Bytes, rquest::Error>,
                                                > + Send,
                                        >,
                                    >,
                                Some(archive_handle),
                            )
                        } else {
                            (byte_stream, None)
                        };

                    // Save winning candidate info to ctx.
                    let projected_access = ctx
                        .projected_access_candidates
                        .get(candidate_index)
                        .cloned();
                    set_selected_candidate(
                        ctx,
                        candidate,
                        projected_access.as_ref(),
                        &model,
                        tools_were_injected,
                    );

                    // Do NOT call controller.on_success() here — the
                    // TrackedStream callback will call it when the stream
                    // finishes successfully, avoiding a double-success report.
                    drop(permit);

                    // Clone the controller Arc so the callback can notify on
                    // stream completion.
                    let ctrl_for_cb = Arc::clone(&controller);
                    let provider_id = candidate.provider_account_id.clone();
                    let state_for_cb = Arc::clone(state);
                    let route_policy_for_cb = route_policy_config.clone();
                    let resolved_model_for_cb = model.clone();
                    let audit_snapshot =
                        crate::pipeline::stage_finalize::snapshot_request_audit(ctx);
                    let failure_finalize_snapshot =
                        crate::pipeline::stage_finalize::FailureFinalizeSnapshot {
                            request_id: ctx.req_id.to_string(),
                            credential_id: ctx
                                .quota_credential_id
                                .clone()
                                .or_else(|| ctx.credential_ref.clone()),
                            project_id: ctx
                                .session
                                .as_ref()
                                .map(|session| session.project_id.clone()),
                            user_id: ctx.neuro_user_id.clone().or_else(|| {
                                ctx.session
                                    .as_ref()
                                    .and_then(|session| session.user_id.clone())
                            }),
                            model: Some(model.clone()),
                            provider: Some(candidate.adapter.clone()),
                            started_at: ctx.started_at,
                            pre_deducted_tokens: ctx.quota_pre_deducted_tokens,
                            request_audit: audit_snapshot.clone(),
                        };

                    let provider_credential_id_for_cb = candidate.provider_credential_id.clone();
                    let tracked = TrackedStream::new_with_started_at(
                        byte_stream,
                        stream_started_at,
                        move |metrics, success| {
                            if let Some(lease_handle) = freebuff_lease_handle {
                                tokio::spawn(async move {
                                    if success {
                                        lease_handle.release().await;
                                    } else {
                                        lease_handle
                                            .invalidate(
                                                "freebuff stream terminated before completion",
                                            )
                                            .await;
                                    }
                                });
                            }
                            if success {
                                ctrl_for_cb.on_success();
                                observe_provider_success_metric(
                                    global_gateway_metrics().as_ref(),
                                    &provider_id,
                                    &resolved_model_for_cb,
                                    metrics.total_duration_ms,
                                );
                                spawn_record_provider_success(
                                    Arc::clone(&state_for_cb),
                                    provider_id.clone(),
                                    provider_credential_id_for_cb.clone(),
                                );
                                let state_for_audit = Arc::clone(&state_for_cb);
                                let audit_snapshot = audit_snapshot.clone();
                                let usage =
                                    stream_usage_handle.as_ref().and_then(snapshot_tapped_usage);
                                let canonical_completion_semantics =
                                    stream_completion_semantics_handle
                                        .as_ref()
                                        .and_then(snapshot_tapped_completion_semantics);
                                let archive_snapshot =
                                    stream_archive_handle.as_ref().map(snapshot_tapped_archive);
                                tokio::spawn(async move {
                                    crate::pipeline::stage_finalize::run_stream_success(
                                        audit_snapshot,
                                        usage,
                                        canonical_completion_semantics,
                                        archive_snapshot
                                            .as_ref()
                                            .map(|snapshot| snapshot.text.clone()),
                                        archive_snapshot
                                            .as_ref()
                                            .map(|snapshot| snapshot.truncated)
                                            .unwrap_or(false),
                                        &state_for_audit,
                                    )
                                    .await;
                                });
                            } else {
                                ctrl_for_cb.on_failure(FailureKind::General);
                                observe_provider_failure_metric(
                                    global_gateway_metrics().as_ref(),
                                    &provider_id,
                                    &resolved_model_for_cb,
                                    metrics.total_duration_ms,
                                    "stream terminated before completion",
                                );
                                spawn_record_provider_failure(
                                    Arc::clone(&state_for_cb),
                                    provider_id.clone(),
                                    provider_credential_id_for_cb.clone(),
                                    route_policy_for_cb.clone(),
                                    "stream terminated before completion".to_string(),
                                );
                                let state_for_finalize = Arc::clone(&state_for_cb);
                                tokio::spawn(async move {
                                    crate::pipeline::stage_finalize::run_stream_failure(
                                        failure_finalize_snapshot,
                                        &state_for_finalize,
                                    )
                                    .await;
                                });
                            }
                            tracing::debug!(
                                provider = %provider_id,
                                chunks = metrics.chunk_count,
                                bytes = metrics.total_bytes,
                                duration_ms = metrics.total_duration_ms,
                                success,
                                "stream completed"
                            );
                        },
                    );

                    return Ok(PipelineOutput::Sse(tracked));
                }
                Err(e) => {
                    if should_record_provider_failure(&e) {
                        let failure_kind = classify_failure_kind(&e);
                        controller.on_failure(failure_kind);
                        if !matches!(
                            candidate.adapter.as_str(),
                            "qwen_web_compatible" | "chatgpt_web_reverse_compatible"
                        ) {
                            observe_provider_failure_metric(
                                global_gateway_metrics().as_ref(),
                                &candidate.provider_account_id,
                                &model,
                                stream_attempt_started_at.elapsed().as_millis() as u64,
                                &e.message,
                            );
                        }
                        spawn_record_provider_failure(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                            route_policy_config.clone(),
                            e.message.clone(),
                        );
                    }
                    drop(permit);

                    if should_try_next_candidate(&e) {
                        warn!(
                            req_id = %ctx.req_id,
                            provider = %candidate.provider_account_id,
                            error = %e,
                            "candidate failed; trying next"
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            // ── Non-streaming path ────────────────────────────────────────

            if tools_were_injected
                && ctx.canonical_req.endpoint_kind == EndpointKind::Responses
                && candidate.adapter == "anthropic_compatible"
            {
                let bridge_attempt_started_at = Instant::now();
                match state
                    .upstream_client
                    .execute_stream(
                        &effective_payload,
                        &ctx.canonical_req,
                        &model,
                        Some(&ctx.request_headers),
                    )
                    .await
                {
                    Ok(UpstreamStreamingResponse::Http(response)) => {
                        let status = response.status().as_u16();
                        if !response.status().is_success() {
                            let failure = classify_upstream_error(
                                status,
                                "upstream streaming bridge failed",
                                Some(&candidate.adapter),
                            );
                            let failure_kind = classify_failure_kind(&failure);
                            controller.on_failure(failure_kind);
                            observe_provider_failure_metric(
                                global_gateway_metrics().as_ref(),
                                &candidate.provider_account_id,
                                &model,
                                bridge_attempt_started_at.elapsed().as_millis() as u64,
                                &failure.message,
                            );
                            spawn_record_provider_failure(
                                Arc::clone(state),
                                candidate.provider_account_id.clone(),
                                candidate.provider_credential_id.clone(),
                                route_policy_config.clone(),
                                failure.message.clone(),
                            );
                            drop(permit);

                            if should_try_next_candidate(&failure) {
                                last_error = Some(failure);
                                continue;
                            } else {
                                return Err(failure);
                            }
                        }

                        let anthropic_stream = Box::pin(response.bytes_stream())
                            as std::pin::Pin<
                                Box<
                                    dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>>
                                        + Send,
                                >,
                            >;
                        let openai_stream =
                            Box::pin(accio::translate_anthropic_like_stream_to_openai(
                                anthropic_stream,
                                reply_model.clone(),
                            ));
                        let openai_stream = wrap_injected_openai_stream(
                            openai_stream,
                            true,
                            &ctx.req_id,
                            &reply_model,
                            original_tools.clone(),
                            original_tool_choice.clone(),
                            Some(original_messages_text.clone()),
                        );
                        let responses_stream =
                            Box::pin(responses::translate_openai_sse_to_responses(
                                openai_stream,
                                reply_model.clone(),
                            ));

                        match responses::accumulate_responses_sse_stream(
                            responses_stream,
                            &reply_model,
                        )
                        .await
                        {
                            Ok(canonical_resp) => {
                                controller.on_success();
                                observe_provider_success_metric(
                                    global_gateway_metrics().as_ref(),
                                    &candidate.provider_account_id,
                                    &model,
                                    bridge_attempt_started_at.elapsed().as_millis() as u64,
                                );
                                spawn_record_provider_success(
                                    Arc::clone(state),
                                    candidate.provider_account_id.clone(),
                                    candidate.provider_credential_id.clone(),
                                );
                                drop(permit);

                                let projected_access = ctx
                                    .projected_access_candidates
                                    .get(candidate_index)
                                    .cloned();
                                set_selected_candidate(
                                    ctx,
                                    candidate,
                                    projected_access.as_ref(),
                                    &model,
                                    tools_were_injected,
                                );
                                ctx.canonical_completion_semantics =
                                    infer_canonical_completion_semantics(
                                        &ctx.canonical_req,
                                        &canonical_resp,
                                    )
                                    .map(str::to_string);
                                ctx.observed_usage = canonical_resp.usage.clone();

                                let json_resp = pack_response(&ctx.canonical_req, &canonical_resp);
                                return Ok(PipelineOutput::Json(json_resp));
                            }
                            Err(error) => {
                                let failure_kind = classify_failure_kind(&error);
                                controller.on_failure(failure_kind);
                                observe_provider_failure_metric(
                                    global_gateway_metrics().as_ref(),
                                    &candidate.provider_account_id,
                                    &model,
                                    bridge_attempt_started_at.elapsed().as_millis() as u64,
                                    &error.message,
                                );
                                spawn_record_provider_failure(
                                    Arc::clone(state),
                                    candidate.provider_account_id.clone(),
                                    candidate.provider_credential_id.clone(),
                                    route_policy_config.clone(),
                                    error.message.clone(),
                                );
                                drop(permit);

                                if should_try_next_candidate(&error) {
                                    last_error = Some(error);
                                    continue;
                                } else {
                                    return Err(error);
                                }
                            }
                        }
                    }
                    Ok(UpstreamStreamingResponse::Bytes(_)) => {
                        let error = GatewayError::server_error(
                            "unexpected byte-stream upstream for anthropic responses bridge",
                        )
                        .with_code("unexpected_non_http_stream_bridge")
                        .with_provider(candidate.adapter.as_str());
                        let failure_kind = classify_failure_kind(&error);
                        controller.on_failure(failure_kind);
                        observe_provider_failure_metric(
                            global_gateway_metrics().as_ref(),
                            &candidate.provider_account_id,
                            &model,
                            bridge_attempt_started_at.elapsed().as_millis() as u64,
                            &error.message,
                        );
                        spawn_record_provider_failure(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                            route_policy_config.clone(),
                            error.message.clone(),
                        );
                        drop(permit);

                        if should_try_next_candidate(&error) {
                            last_error = Some(error);
                            continue;
                        } else {
                            return Err(error);
                        }
                    }
                    Err(error) => {
                        let failure_kind = classify_failure_kind(&error);
                        controller.on_failure(failure_kind);
                        observe_provider_failure_metric(
                            global_gateway_metrics().as_ref(),
                            &candidate.provider_account_id,
                            &model,
                            bridge_attempt_started_at.elapsed().as_millis() as u64,
                            &error.message,
                        );
                        spawn_record_provider_failure(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                            route_policy_config.clone(),
                            error.message.clone(),
                        );
                        drop(permit);

                        if should_try_next_candidate(&error) {
                            last_error = Some(error);
                            continue;
                        } else {
                            return Err(error);
                        }
                    }
                }
            }

            // Capture immutable refs needed inside the closure.
            let payload = effective_payload.clone();
            let canonical_req = ctx.canonical_req.clone();
            let extra_hdrs = ctx.request_headers.clone();
            let provider_account_id = candidate.provider_account_id.clone();
            let client = &state.upstream_client;

            if expects_binary_passthrough(&ctx.canonical_req) {
                let metric_provider = candidate.provider_account_id.clone();
                let metric_model = model.clone();
                let result = execute_with_retry_after_admission_observed(
                    || {
                        let payload = payload.clone();
                        let canonical_req = canonical_req.clone();
                        let model = model.clone();
                        let extra_hdrs = extra_hdrs.clone();
                        let provider_account_id = provider_account_id.clone();
                        async move {
                            client
                                .execute_binary_passthrough_with_provider_account_id(
                                    &provider_account_id,
                                    &payload,
                                    &canonical_req,
                                    &model,
                                    Some(&extra_hdrs),
                                )
                                .await
                        }
                    },
                    || provider_attempt_gate.admit(),
                    move |observation| {
                        observe_provider_attempt_metric(
                            global_gateway_metrics().as_ref(),
                            &metric_provider,
                            &metric_model,
                            observation,
                        );
                    },
                    &retry_policy,
                )
                .await;

                match result {
                    Ok(binary_resp) => {
                        controller.on_success();
                        spawn_record_provider_success(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                        );
                        drop(permit);

                        let projected_access = ctx
                            .projected_access_candidates
                            .get(candidate_index)
                            .cloned();
                        set_selected_candidate(
                            ctx,
                            candidate,
                            projected_access.as_ref(),
                            &model,
                            tools_were_injected,
                        );

                        return Ok(PipelineOutput::Binary(BinaryPipelineResponse {
                            body: binary_resp.body,
                            content_type: binary_resp.content_type,
                            extra_headers: binary_resp.extra_headers,
                        }));
                    }
                    Err(e) => {
                        if should_record_provider_failure(&e) {
                            let failure_kind = classify_failure_kind(&e);
                            controller.on_failure(failure_kind);
                            spawn_record_provider_failure(
                                Arc::clone(state),
                                candidate.provider_account_id.clone(),
                                candidate.provider_credential_id.clone(),
                                route_policy_config.clone(),
                                e.message.clone(),
                            );
                        }
                        drop(permit);

                        if should_try_next_candidate(&e) {
                            warn!(
                                req_id = %ctx.req_id,
                                provider = %candidate.provider_account_id,
                                error = %e,
                                "candidate failed; trying next"
                            );
                            last_error = Some(e);
                            continue;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }

            if expects_json_passthrough(&ctx.canonical_req) {
                let metric_provider = candidate.provider_account_id.clone();
                let metric_model = model.clone();
                let result = execute_with_retry_after_admission_observed(
                    || {
                        let payload = payload.clone();
                        let canonical_req = canonical_req.clone();
                        let model = model.clone();
                        let extra_hdrs = extra_hdrs.clone();
                        async move {
                            client
                                .execute_json_passthrough(
                                    &candidate.provider_account_id,
                                    candidate.resolved_execution_mode,
                                    &payload,
                                    &canonical_req,
                                    &model,
                                    Some(&extra_hdrs),
                                )
                                .await
                        }
                    },
                    || provider_attempt_gate.admit(),
                    move |observation| {
                        observe_provider_attempt_metric(
                            global_gateway_metrics().as_ref(),
                            &metric_provider,
                            &metric_model,
                            observation,
                        );
                    },
                    &retry_policy,
                )
                .await;

                match result {
                    Ok(json_resp) => {
                        controller.on_success();
                        spawn_record_provider_success(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                        );
                        drop(permit);

                        let projected_access = ctx
                            .projected_access_candidates
                            .get(candidate_index)
                            .cloned();
                        set_selected_candidate(
                            ctx,
                            candidate,
                            projected_access.as_ref(),
                            &model,
                            tools_were_injected,
                        );

                        return Ok(PipelineOutput::Json(json_resp));
                    }
                    Err(e) => {
                        if should_record_provider_failure(&e) {
                            let failure_kind = classify_failure_kind(&e);
                            controller.on_failure(failure_kind);
                            spawn_record_provider_failure(
                                Arc::clone(state),
                                candidate.provider_account_id.clone(),
                                candidate.provider_credential_id.clone(),
                                route_policy_config.clone(),
                                e.message.clone(),
                            );
                        }
                        drop(permit);

                        if should_try_next_candidate(&e) {
                            warn!(
                                req_id = %ctx.req_id,
                                provider = %candidate.provider_account_id,
                                error = %e,
                                "candidate failed; trying next"
                            );
                            last_error = Some(e);
                            continue;
                        } else {
                            return Err(e);
                        }
                    }
                }
            }

            let result = if candidate.adapter == "qwen_web_compatible" {
                execute_qwen_web_nonstream_with_recovery(
                    state,
                    candidate,
                    &provider_attempt_gate,
                    &payload,
                    &canonical_req,
                    &model,
                    &extra_hdrs,
                    &retry_policy,
                )
                .await
            } else if candidate.adapter == "chatgpt_web_reverse_compatible" {
                execute_chatgpt_web_nonstream_with_recovery(
                    state,
                    candidate,
                    &provider_attempt_gate,
                    &payload,
                    &canonical_req,
                    &model,
                    &extra_hdrs,
                    &retry_policy,
                )
                .await
            } else {
                let metric_provider = candidate.provider_account_id.clone();
                let metric_model = model.clone();
                execute_with_retry_after_admission_observed(
                    || {
                        let payload = payload.clone();
                        let canonical_req = canonical_req.clone();
                        let model = model.clone();
                        let extra_hdrs = extra_hdrs.clone();
                        let provider_account_id = provider_account_id.clone();
                        async move {
                            if candidate.adapter == "freebuff_compatible" {
                                freebuff::execute(
                                    client.client(),
                                    &payload,
                                    &canonical_req,
                                    &model,
                                    Some(&extra_hdrs),
                                )
                                .await
                            } else {
                                client
                                    .execute_with_provider_account_id(
                                        &provider_account_id,
                                        &payload,
                                        &canonical_req,
                                        &model,
                                        Some(&extra_hdrs),
                                    )
                                    .await
                            }
                        }
                    },
                    || provider_attempt_gate.admit(),
                    move |observation| {
                        observe_provider_attempt_metric(
                            global_gateway_metrics().as_ref(),
                            &metric_provider,
                            &metric_model,
                            observation,
                        );
                    },
                    &retry_policy,
                )
                .await
            };

            match result {
                Ok(mut canonical_resp) => {
                    controller.on_success();
                    spawn_record_provider_success(
                        Arc::clone(state),
                        candidate.provider_account_id.clone(),
                        candidate.provider_credential_id.clone(),
                    );
                    drop(permit);

                    // If we injected tools, parse XML tool calls from the
                    // response text and convert to standard tool_calls format.
                    if !original_tools.is_empty() {
                        maybe_apply_xml_tool_response_bridge(
                            &ctx.req_id,
                            &mut canonical_resp,
                            &original_tools,
                            original_tool_choice.as_ref(),
                            Some(original_messages_text.as_str()),
                            tools_were_injected,
                        );
                    }

                    // Save winning candidate info to ctx.
                    let projected_access = ctx
                        .projected_access_candidates
                        .get(candidate_index)
                        .cloned();
                    set_selected_candidate(
                        ctx,
                        candidate,
                        projected_access.as_ref(),
                        &model,
                        tools_were_injected,
                    );
                    ctx.canonical_completion_semantics =
                        infer_canonical_completion_semantics(&ctx.canonical_req, &canonical_resp)
                            .map(str::to_string);
                    ctx.observed_usage = canonical_resp.usage.clone();

                    // Pack the response back into the wire format matching
                    // the original protocol family.
                    let json_resp = pack_response(&ctx.canonical_req, &canonical_resp);
                    return Ok(PipelineOutput::Json(json_resp));
                }
                Err(e) => {
                    if should_record_provider_failure(&e) {
                        let failure_kind = classify_failure_kind(&e);
                        controller.on_failure(failure_kind);
                        spawn_record_provider_failure(
                            Arc::clone(state),
                            candidate.provider_account_id.clone(),
                            candidate.provider_credential_id.clone(),
                            route_policy_config,
                            e.message.clone(),
                        );
                    }
                    drop(permit);

                    if should_try_next_candidate(&e) {
                        warn!(
                            req_id = %ctx.req_id,
                            provider = %candidate.provider_account_id,
                            error = %e,
                            "candidate failed; trying next"
                        );
                        last_error = Some(e);
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    // All candidates exhausted.
    Err(provider_rate_limit_rejections
        .resolve_terminal_error(last_error)
        .unwrap_or_else(|| GatewayError::server_error("all provider candidates exhausted")))
}

fn observe_provider_rate_limit_rejection(
    rejections: &mut ProviderRateLimitRejections,
    error: &GatewayError,
) -> bool {
    if error.code.as_deref() != Some("rate_limit_exceeded") {
        return false;
    }
    let FallbackHint::Retry { delay_ms, .. } = &error.fallback_hint else {
        return false;
    };
    rejections.observe(*delay_ms);
    true
}

fn expects_json_passthrough(req: &crate::protocol::canonical::CanonicalRelayRequest) -> bool {
    match req.endpoint_kind {
        EndpointKind::Embeddings
        | EndpointKind::ImagesGenerations
        | EndpointKind::ImagesEdits
        | EndpointKind::MusicGenerations
        | EndpointKind::VideosGenerations
        | EndpointKind::Search
        | EndpointKind::Fetch
        | EndpointKind::ResearchCreate
        | EndpointKind::ResearchList
        | EndpointKind::ResearchGet
        | EndpointKind::CreditsBalance => true,
        EndpointKind::AudioTranscriptions => !matches!(
            req.raw_body
                .get("response_format")
                .and_then(|value| value.as_str())
                .map(|value| value.to_ascii_lowercase()),
            Some(format) if matches!(format.as_str(), "text" | "srt" | "vtt")
        ),
        _ => false,
    }
}

fn expects_binary_passthrough(req: &crate::protocol::canonical::CanonicalRelayRequest) -> bool {
    matches!(req.endpoint_kind, EndpointKind::AudioSpeech)
        || (req.endpoint_kind == EndpointKind::AudioTranscriptions
            && !expects_json_passthrough(req))
}

fn is_conversation_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    )
}

fn selected_openai_target_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> &'static str {
    if candidate
        .payload
        .bridges_openai_text_endpoint_to_responses(req.endpoint_kind)
    {
        "openai_responses"
    } else if candidate
        .payload
        .bridges_openai_responses_to_chat_completions(req.endpoint_kind)
    {
        "openai_chat"
    } else if req.endpoint_kind == EndpointKind::Embeddings {
        OPENAI_EMBEDDINGS_FAMILY
    } else if req.endpoint_kind == EndpointKind::AudioTranscriptions {
        OPENAI_AUDIO_TRANSCRIPTIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::AudioSpeech {
        OPENAI_AUDIO_SPEECH_FAMILY
    } else if req.endpoint_kind == EndpointKind::ImagesGenerations {
        OPENAI_IMAGES_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::ImagesEdits {
        OPENAI_IMAGES_EDITS_FAMILY
    } else if req.endpoint_kind == EndpointKind::MusicGenerations {
        OPENAI_MUSIC_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::VideosGenerations {
        OPENAI_VIDEOS_GENERATIONS_FAMILY
    } else if req.endpoint_kind == EndpointKind::Responses {
        "openai_responses"
    } else {
        "openai_chat"
    }
}

fn infer_selected_upstream_target_protocol_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Option<String> {
    let family = match candidate.protocol_family.as_str() {
        "openai" => selected_openai_target_family(req, candidate).to_string(),
        "gemini_business" => GEMINI_BUSINESS_IMAGES_FAMILY.to_string(),
        "chataibot" => CHATAIBOT_IMAGES_FAMILY.to_string(),
        "lumalabs" => match req.endpoint_kind {
            EndpointKind::MusicGenerations => LUMALABS_AUDIO_FAMILY.to_string(),
            EndpointKind::VideosGenerations => LUMALABS_VIDEOS_FAMILY.to_string(),
            _ => LUMALABS_IMAGES_FAMILY.to_string(),
        },
        "gemini_canvas" => match req.endpoint_kind {
            EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
            | EndpointKind::AudioSpeech => GEMINI_GENERATE_CONTENT_FAMILY.to_string(),
            EndpointKind::MusicGenerations => GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
            EndpointKind::VideosGenerations => GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
            _ => GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
        },
        "producer" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => PRODUCER_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => PRODUCER_VIDEOS_FAMILY.to_string(),
            _ => PRODUCER_MUSIC_FAMILY.to_string(),
        },
        "suno" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => SUNO_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => SUNO_VIDEOS_FAMILY.to_string(),
            _ => SUNO_MUSIC_FAMILY.to_string(),
        },
        "udio" => match req.endpoint_kind {
            EndpointKind::ImagesGenerations => UDIO_IMAGES_FAMILY.to_string(),
            EndpointKind::VideosGenerations => UDIO_VIDEOS_FAMILY.to_string(),
            _ => UDIO_MUSIC_FAMILY.to_string(),
        },
        _ => candidate.protocol_family.clone(),
    };
    Some(family)
}

fn infer_selected_upstream_target_conversation_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    match candidate.protocol_family.as_str() {
        "openai" => Some(selected_openai_target_family(req, candidate).to_string()),
        "anthropic" => Some("anthropic_messages".to_string()),
        "gemini_generate_content"
        | "gemini_live"
        | "bedrock_converse"
        | "cohere_chat"
        | "openai_chat"
        | "openai_legacy_completions"
        | "openai_responses"
        | "openai_realtime"
        | "xfyun_websocket" => Some(candidate.protocol_family.clone()),
        _ => Some(candidate.protocol_family.clone()),
    }
}

fn infer_canonical_conversation_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }

    match req.protocol_family {
        ProtocolFamily::OpenAi => match req.endpoint_kind {
            EndpointKind::Responses => Some("responses_input_output"),
            EndpointKind::Completions => Some("prompt_completion"),
            _ => Some("messages_turns"),
        },
        ProtocolFamily::OpenAiRealtime | ProtocolFamily::GeminiLive => Some("live_session_events"),
        ProtocolFamily::Anthropic | ProtocolFamily::BedrockConverse => Some("messages_blocks"),
        ProtocolFamily::GeminiGenerateContent => Some("parts_turns"),
        ProtocolFamily::CohereChat => Some("messages_turns"),
        ProtocolFamily::SearchApi => None,
    }
}

fn infer_selected_upstream_target_tool_family(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    candidate: &crate::routing::candidate::RouteCandidate,
    tools_were_injected: bool,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some("xml_fallback".to_string());
    }

    infer_selected_upstream_target_conversation_family(req, candidate)
}

fn infer_tool_strategy(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    tools_were_injected: bool,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some("xml_fallback");
    }
    if req.tools.is_empty() {
        return None;
    }
    Some("native")
}

fn infer_canonical_tool_choice_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    tools_were_injected: bool,
) -> Option<String> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }
    if tools_were_injected {
        return Some(
            CanonicalToolChoice::PromptOnly
                .semantics_label()
                .to_string(),
        );
    }
    if req.tools.is_empty() {
        return None;
    }

    let choice = tool_choice::parse_tool_choice(req.tool_choice.as_ref())
        .unwrap_or(CanonicalToolChoice::Auto);
    Some(choice.semantics_label().to_string())
}

fn infer_canonical_completion_semantics(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    resp: &crate::protocol::canonical::CanonicalRelayResponse,
) -> Option<&'static str> {
    if !is_conversation_endpoint(req.endpoint_kind) {
        return None;
    }

    match resp
        .finish_reason
        .as_deref()
        .unwrap_or(if resp.tool_calls.is_empty() {
            "stop"
        } else {
            "tool_calls"
        }) {
        "stop" | "end_turn" | "stop_sequence" | "complete" => Some("stop"),
        "tool_calls" | "function_call" => Some("tool_calls"),
        "length" => Some("length"),
        "content_filter" => Some("content_filter"),
        _ => Some("other_provider_reason"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn set_selected_candidate(
    ctx: &mut PipelineContext,
    candidate: &crate::routing::candidate::RouteCandidate,
    projected_access: Option<&crate::db::ProjectedPlatformAccessRow>,
    model: &str,
    tools_were_injected: bool,
) {
    ctx.selected_provider_id = Some(candidate.provider_account_id.clone());
    ctx.selected_provider_credential_id = candidate.provider_credential_id.clone();
    ctx.selected_provider_label = Some(candidate.label.clone());
    ctx.selected_adapter = Some(candidate.adapter.clone());
    ctx.selected_protocol_profile = Some(candidate.protocol_profile.clone());
    if let Some(line) =
        crate::implementation_lines::line_for_payload(&candidate.payload).or_else(|| {
            crate::implementation_lines::line_for_protocol_profile(&candidate.protocol_profile)
        })
    {
        let line_id = line
            .feature_name()
            .strip_prefix("line-")
            .unwrap_or(line.feature_name());
        crate::http::route_proof::record_route_proof(crate::http::route_proof::RouteProof::new(
            line_id,
        ));
    }
    ctx.resolved_model = Some(model.to_string());
    ctx.selected_model_alias = candidate.model_alias.clone();
    ctx.selected_execution_mode = Some(
        match candidate.resolved_execution_mode {
            crate::routing::candidate::ProviderExecutionMode::DirectHttp => "direct_http",
            crate::routing::candidate::ProviderExecutionMode::BrowserBacked => "browser_backed",
        }
        .to_string(),
    );
    ctx.selected_upstream_target_protocol_family =
        infer_selected_upstream_target_protocol_family(&ctx.canonical_req, candidate);
    ctx.selected_upstream_target_conversation_family =
        infer_selected_upstream_target_conversation_family(&ctx.canonical_req, candidate);
    ctx.canonical_conversation_semantics =
        infer_canonical_conversation_semantics(&ctx.canonical_req).map(str::to_string);
    ctx.selected_upstream_target_tool_family = infer_selected_upstream_target_tool_family(
        &ctx.canonical_req,
        candidate,
        tools_were_injected,
    )
    .map(|value| value.to_string());
    ctx.tool_strategy =
        infer_tool_strategy(&ctx.canonical_req, tools_were_injected).map(str::to_string);
    ctx.canonical_tool_choice_semantics =
        infer_canonical_tool_choice_semantics(&ctx.canonical_req, tools_were_injected);
    ctx.selected_routing_score = candidate.routing_score;
    ctx.selected_health_weight = candidate.routing_health_weight;
    ctx.selected_capacity_weight = candidate.routing_capacity_weight;
    ctx.selected_degraded = candidate.routing_degraded;
    ctx.selected_breaker_open = candidate.routing_breaker_open;
    ctx.selected_degradation_reasons = candidate.routing_degradation_reasons.clone();
    if let Some(projected_access) = projected_access {
        ctx.source_access_key_id = Some(projected_access.source_access_key_id.clone());
        ctx.selected_platform_access_id = Some(projected_access.platform_access_id.clone());
    }
    ctx.selected_real_credential_ref = candidate.payload.credential_id.clone();
}

fn capture_prompt_cache_telemetry(ctx: &mut PipelineContext, adapter: &str, model: &str) {
    let telemetry = match adapter {
        "anthropic_compatible" => {
            anthropic::inspect_prompt_cache_telemetry(&ctx.canonical_req, model, ctx.stream)
        }
        "accio_compatible" => accio::inspect_prompt_cache_telemetry(&ctx.canonical_req, model),
        _ => {
            ctx.client_has_cache_control = false;
            ctx.auto_cache_applied = false;
            return;
        }
    };

    ctx.client_has_cache_control = telemetry.client_has_cache_control;
    ctx.auto_cache_applied = telemetry.auto_cache_applied;
}

fn has_strict_tool_choice(req: &crate::protocol::canonical::CanonicalRelayRequest) -> bool {
    matches!(
        tool_choice::parse_tool_choice(req.tool_choice.as_ref()),
        Some(CanonicalToolChoice::Required | CanonicalToolChoice::Specific(_))
    )
}

fn maybe_apply_xml_tool_response_bridge(
    req_id: &uuid::Uuid,
    canonical_resp: &mut crate::protocol::canonical::CanonicalRelayResponse,
    original_tools: &[crate::protocol::canonical::CanonicalTool],
    original_tool_choice: Option<&serde_json::Value>,
    original_messages_text: Option<&str>,
    tools_were_injected: bool,
) -> bool {
    if original_tools.is_empty() || !canonical_resp.tool_calls.is_empty() {
        return false;
    }

    let parse_result = tool_inject::parse_tool_calls_from_text_with_context(
        &canonical_resp.text,
        original_tools,
        original_tool_choice,
        original_messages_text,
    );
    if !parse_result.had_tool_calls {
        return false;
    }

    debug!(
        req_id = %req_id,
        count = parse_result.tool_calls.len(),
        tools_were_injected,
        "promoted XML tool calls from upstream text response"
    );
    canonical_resp.text = parse_result.clean_text;
    canonical_resp.tool_calls = parse_result.tool_calls;
    canonical_resp.finish_reason = Some("tool_calls".to_string());
    true
}

fn should_force_bridge_tool_injection(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    adapter: &str,
) -> bool {
    if req.tools.is_empty() || adapter != "anthropic_compatible" || !has_strict_tool_choice(req) {
        return false;
    }

    matches!(
        (req.protocol_family, req.endpoint_kind),
        (
            ProtocolFamily::OpenAiRealtime,
            EndpointKind::ChatCompletions
        ) | (ProtocolFamily::OpenAi, EndpointKind::Responses)
    )
}

fn wrap_injected_openai_stream(
    byte_stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
    >,
    tools_were_injected: bool,
    req_id: &uuid::Uuid,
    model: &str,
    original_tools: Vec<crate::protocol::canonical::CanonicalTool>,
    original_tool_choice: Option<serde_json::Value>,
    original_messages_text: Option<String>,
) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>> {
    if !tools_were_injected {
        return byte_stream;
    }

    let resp_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    debug!(
        req_id = %req_id,
        "wrapping translated OpenAI stream with XML tool call detector"
    );
    tool_inject::wrap_streaming_tool_detection(
        byte_stream,
        model.to_string(),
        resp_id,
        original_tools,
        original_tool_choice,
        original_messages_text,
    )
}

fn should_refresh_qwen_web_after_failure(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            crate::protocol::qwen_web::QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE
                | crate::protocol::qwen_web::QWEN_WEB_SESSION_INVALID_CODE
        )
    )
}

async fn execute_qwen_web_nonstream_with_recovery(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    provider_attempt_gate: &super::stage_rate_limit::ProviderAttemptGate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: &std::collections::HashMap<String, String>,
    retry_policy: &RetryPolicy,
) -> Result<crate::protocol::canonical::CanonicalRelayResponse, GatewayError> {
    let provider_account_id = candidate.provider_account_id.clone();
    let first_metric_provider = provider_account_id.clone();
    let first_metric_model = model.to_string();
    let first_attempt = execute_with_retry_after_admission_observed(
        || {
            let payload = payload.clone();
            let req = req.clone();
            let model = model.to_string();
            let extra_headers = extra_headers.clone();
            let provider_account_id = provider_account_id.clone();
            async move {
                state
                    .upstream_client
                    .execute_with_provider_account_id(
                        &provider_account_id,
                        &payload,
                        &req,
                        &model,
                        Some(&extra_headers),
                    )
                    .await
            }
        },
        || provider_attempt_gate.admit(),
        move |observation| {
            observe_provider_attempt_metric(
                global_gateway_metrics().as_ref(),
                &first_metric_provider,
                &first_metric_model,
                observation,
            );
        },
        retry_policy,
    )
    .await;

    match first_attempt {
        Ok(response) => Ok(response),
        Err(error) if should_refresh_qwen_web_after_failure(&error) => {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                "Qwen Web direct replay failed; forcing browser-session refresh before retry"
            );
            let refreshed_payload = keepalive::refresh_qwen_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                model,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            let refreshed_metric_provider = provider_account_id.clone();
            let refreshed_metric_model = model.to_string();
            execute_with_retry_after_admission_observed(
                || {
                    let payload = refreshed_payload.clone();
                    let req = req.clone();
                    let model = model.to_string();
                    let extra_headers = extra_headers.clone();
                    let provider_account_id = provider_account_id.clone();
                    async move {
                        state
                            .upstream_client
                            .execute_with_provider_account_id(
                                &provider_account_id,
                                &payload,
                                &req,
                                &model,
                                Some(&extra_headers),
                            )
                            .await
                    }
                },
                || provider_attempt_gate.admit(),
                move |observation| {
                    observe_provider_attempt_metric(
                        global_gateway_metrics().as_ref(),
                        &refreshed_metric_provider,
                        &refreshed_metric_model,
                        observation,
                    );
                },
                retry_policy,
            )
            .await
        }
        Err(error) => Err(error),
    }
}

async fn execute_qwen_web_stream_with_recovery(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    provider_attempt_gate: &super::stage_rate_limit::ProviderAttemptGate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: &std::collections::HashMap<String, String>,
) -> Result<(UpstreamStreamingResponse, Instant), GatewayError> {
    let first_attempt_started_at = Instant::now();
    let first_attempt = state
        .upstream_client
        .execute_stream_with_provider_account_id(
            &candidate.provider_account_id,
            payload,
            req,
            model,
            Some(extra_headers),
        )
        .await;
    if first_attempt.is_err() {
        observe_provider_result_metric(
            global_gateway_metrics().as_ref(),
            &candidate.provider_account_id,
            model,
            &first_attempt,
            first_attempt_started_at.elapsed().as_millis() as u64,
        );
    }
    match first_attempt {
        Ok(response) => Ok((response, first_attempt_started_at)),
        Err(error) if should_refresh_qwen_web_after_failure(&error) => {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                "Qwen Web stream start failed; forcing browser-session refresh before retry"
            );
            let refreshed_payload = keepalive::refresh_qwen_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                model,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            let refreshed_attempt_started_at = Instant::now();
            let refreshed_attempt = state
                .upstream_client
                .execute_stream_with_provider_account_id(
                    &candidate.provider_account_id,
                    &refreshed_payload,
                    req,
                    model,
                    Some(extra_headers),
                )
                .await;
            if refreshed_attempt.is_err() {
                observe_provider_result_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    model,
                    &refreshed_attempt,
                    refreshed_attempt_started_at.elapsed().as_millis() as u64,
                );
            }
            match refreshed_attempt {
                Ok(response) => Ok((response, refreshed_attempt_started_at)),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn should_refresh_chatgpt_web_after_failure(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE
                | crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_SESSION_INVALID_CODE
        )
    )
}

fn should_escalate_chatgpt_web_to_browser_relay(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    error: &GatewayError,
) -> bool {
    if should_refresh_chatgpt_web_after_failure(error) {
        return true;
    }
    let has_browser_recovery_seed = payload
        .extra_body
        .as_ref()
        .map(|body| {
            [
                "authSeed",
                "chatgptAuthUrl",
                "mailboxRef",
                "mailboxSessionId",
            ]
            .iter()
            .any(|key| body.get(*key).is_some())
        })
        .unwrap_or(false);
    let has_runtime_bearer = !payload.api_key.trim().is_empty()
        || payload
            .auth_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
    let has_runtime_cookie = payload
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("cookie"))
        .map(|(_, value)| value.trim())
        .is_some_and(|value| !value.is_empty());
    if !(has_browser_recovery_seed || has_runtime_bearer || has_runtime_cookie) {
        return false;
    }
    let message = error.message.as_str();
    matches!(
        error.kind,
        crate::error::ErrorKind::ServerError | crate::error::ErrorKind::ServiceUnavailable
    ) && (message.contains("Internal Server Error") || message.contains("\"detail\""))
}

fn chatgpt_web_request_time_browser_allowed(
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> bool {
    if let Ok(value) = std::env::var("CHATGPT_WEB_REVERSE_REQUEST_TIME_BROWSER") {
        let normalized = value.trim().to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "0" | "false" | "no" | "off" | "disabled" | "never" | "pure_http_only"
        ) {
            return false;
        }
    }

    let Some(extra_body) = payload.extra_body.as_ref() else {
        return true;
    };
    if extra_body
        .get("requestTimeBrowserAllowed")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|allowed| !allowed)
    {
        return false;
    }
    let mode = extra_body
        .get("requestTimeBrowserMode")
        .or_else(|| extra_body.get("browserFallbackMode"))
        .or_else(|| extra_body.get("fallbackMode"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .map(str::to_ascii_lowercase);
    if let Some(mode) = mode {
        if matches!(
            mode.as_str(),
            "disabled" | "never" | "off" | "pure_http_only" | "browserless" | "fail_fast"
        ) {
            return false;
        }
    }
    true
}

fn chatgpt_web_request_time_browser_path_needed(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    error: &GatewayError,
) -> bool {
    should_refresh_chatgpt_web_after_failure(error)
        || should_escalate_chatgpt_web_to_browser_relay(payload, error)
}

fn chatgpt_web_browser_fallback_forbidden_error(error: &GatewayError) -> GatewayError {
    GatewayError::service_unavailable(format!(
        "ChatGPT Web reverse pure HTTP replay failed and request-time browser fallback is disabled. original_code={} original_message={}",
        error.code.as_deref().unwrap_or("none"),
        error.message
    ))
    .with_provider("chatgpt_web_reverse_compatible")
    .with_code("chatgpt_web_request_time_browser_forbidden")
}

async fn execute_chatgpt_web_nonstream_with_recovery(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    provider_attempt_gate: &super::stage_rate_limit::ProviderAttemptGate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: &std::collections::HashMap<String, String>,
    retry_policy: &RetryPolicy,
) -> Result<crate::protocol::canonical::CanonicalRelayResponse, GatewayError> {
    let provider_account_id = candidate.provider_account_id.clone();
    let first_metric_provider = provider_account_id.clone();
    let first_metric_model = model.to_string();
    let first_attempt = execute_with_retry_after_admission_observed(
        || {
            let payload = payload.clone();
            let req = req.clone();
            let model = model.to_string();
            let extra_headers = extra_headers.clone();
            let provider_account_id = provider_account_id.clone();
            async move {
                state
                    .upstream_client
                    .execute_with_provider_account_id(
                        &provider_account_id,
                        &payload,
                        &req,
                        &model,
                        Some(&extra_headers),
                    )
                    .await
            }
        },
        || provider_attempt_gate.admit(),
        move |observation| {
            observe_provider_attempt_metric(
                global_gateway_metrics().as_ref(),
                &first_metric_provider,
                &first_metric_model,
                observation,
            );
        },
        retry_policy,
    )
    .await;

    match first_attempt {
        Ok(response) => Ok(response),
        Err(error)
            if !chatgpt_web_request_time_browser_allowed(payload)
                && chatgpt_web_request_time_browser_path_needed(payload, &error) =>
        {
            Err(chatgpt_web_browser_fallback_forbidden_error(&error))
        }
        Err(error)
            if chatgpt_web_request_time_browser_allowed(payload)
                && should_refresh_chatgpt_web_after_failure(&error) =>
        {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                "ChatGPT Web reverse direct replay failed; forcing browser-session refresh before retry"
            );
            let refreshed_payload = keepalive::refresh_chatgpt_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            let refreshed_metric_provider = provider_account_id.clone();
            let refreshed_metric_model = model.to_string();
            match execute_with_retry_after_admission_observed(
                || {
                    let payload = refreshed_payload.clone();
                    let req = req.clone();
                    let model = model.to_string();
                    let extra_headers = extra_headers.clone();
                    let provider_account_id = provider_account_id.clone();
                    async move {
                        state
                            .upstream_client
                            .execute_with_provider_account_id(
                                &provider_account_id,
                                &payload,
                                &req,
                                &model,
                                Some(&extra_headers),
                            )
                            .await
                    }
                },
                || provider_attempt_gate.admit(),
                move |observation| {
                    observe_provider_attempt_metric(
                        global_gateway_metrics().as_ref(),
                        &refreshed_metric_provider,
                        &refreshed_metric_model,
                        observation,
                    );
                },
                retry_policy,
            )
            .await
            {
                Ok(response) => Ok(response),
                Err(error)
                    if !chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && chatgpt_web_request_time_browser_path_needed(
                            &refreshed_payload,
                            &error,
                        ) =>
                {
                    Err(chatgpt_web_browser_fallback_forbidden_error(&error))
                }
                Err(error)
                    if chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && should_refresh_chatgpt_web_after_failure(&error) =>
                {
                    debug!(
                        provider = %candidate.provider_account_id,
                        credential_id = ?candidate.provider_credential_id,
                        code = ?error.code,
                        "ChatGPT Web direct replay still challenged after refresh; escalating to browser relay"
                    );
                    provider_attempt_gate.admit().await?;
                    let relay_started_at = Instant::now();
                    let relay_result = async {
                        let relay = keepalive::execute_chatgpt_web_browser_relay(
                            &state.redis_pool,
                            state.pg_pool.as_ref(),
                            &refreshed_payload,
                            req,
                            model,
                            false,
                        )
                        .await?;
                        chatgpt_web::accumulate_response(&relay.body_text, model)
                            .map_err(|error| error.with_provider("chatgpt_web_reverse_compatible"))
                    }
                    .await;
                    observe_provider_result_metric(
                        global_gateway_metrics().as_ref(),
                        &provider_account_id,
                        model,
                        &relay_result,
                        relay_started_at.elapsed().as_millis() as u64,
                    );
                    relay_result
                }
                Err(error)
                    if chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && should_escalate_chatgpt_web_to_browser_relay(
                            &refreshed_payload,
                            &error,
                        ) =>
                {
                    debug!(
                        provider = %candidate.provider_account_id,
                        credential_id = ?candidate.provider_credential_id,
                        code = ?error.code,
                        message = %error.message,
                        "ChatGPT Web direct replay still returned a stable server-side failure after refresh; escalating to browser relay"
                    );
                    provider_attempt_gate.admit().await?;
                    let relay_started_at = Instant::now();
                    let relay_result = async {
                        let relay = keepalive::execute_chatgpt_web_browser_relay(
                            &state.redis_pool,
                            state.pg_pool.as_ref(),
                            &refreshed_payload,
                            req,
                            model,
                            false,
                        )
                        .await?;
                        chatgpt_web::accumulate_response(&relay.body_text, model)
                            .map_err(|error| error.with_provider("chatgpt_web_reverse_compatible"))
                    }
                    .await;
                    observe_provider_result_metric(
                        global_gateway_metrics().as_ref(),
                        &provider_account_id,
                        model,
                        &relay_result,
                        relay_started_at.elapsed().as_millis() as u64,
                    );
                    relay_result
                }
                Err(error) => Err(error),
            }
        }
        Err(error)
            if chatgpt_web_request_time_browser_allowed(payload)
                && should_escalate_chatgpt_web_to_browser_relay(payload, &error) =>
        {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                message = %error.message,
                "ChatGPT Web reverse direct replay returned a stable server-side failure; escalating to browser relay"
            );
            provider_attempt_gate.admit().await?;
            let relay_started_at = Instant::now();
            let relay_result = async {
                let relay = keepalive::execute_chatgpt_web_browser_relay(
                    &state.redis_pool,
                    state.pg_pool.as_ref(),
                    payload,
                    req,
                    model,
                    false,
                )
                .await?;
                chatgpt_web::accumulate_response(&relay.body_text, model)
                    .map_err(|error| error.with_provider("chatgpt_web_reverse_compatible"))
            }
            .await;
            observe_provider_result_metric(
                global_gateway_metrics().as_ref(),
                &provider_account_id,
                model,
                &relay_result,
                relay_started_at.elapsed().as_millis() as u64,
            );
            relay_result
        }
        Err(error) => Err(error),
    }
}

async fn execute_chatgpt_web_stream_with_recovery(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    provider_attempt_gate: &super::stage_rate_limit::ProviderAttemptGate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: &std::collections::HashMap<String, String>,
) -> Result<(UpstreamStreamingResponse, Instant), GatewayError> {
    let first_attempt_started_at = Instant::now();
    let first_attempt = state
        .upstream_client
        .execute_stream_with_provider_account_id(
            &candidate.provider_account_id,
            payload,
            req,
            model,
            Some(extra_headers),
        )
        .await;
    if first_attempt.is_err() {
        observe_provider_result_metric(
            global_gateway_metrics().as_ref(),
            &candidate.provider_account_id,
            model,
            &first_attempt,
            first_attempt_started_at.elapsed().as_millis() as u64,
        );
    }
    match first_attempt {
        Ok(response) => Ok((response, first_attempt_started_at)),
        Err(error)
            if !chatgpt_web_request_time_browser_allowed(payload)
                && chatgpt_web_request_time_browser_path_needed(payload, &error) =>
        {
            Err(chatgpt_web_browser_fallback_forbidden_error(&error))
        }
        Err(error)
            if chatgpt_web_request_time_browser_allowed(payload)
                && should_refresh_chatgpt_web_after_failure(&error) =>
        {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                "ChatGPT Web reverse stream start failed; forcing browser-session refresh before retry"
            );
            let refreshed_payload = keepalive::refresh_chatgpt_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            let refreshed_attempt_started_at = Instant::now();
            let refreshed_attempt = state
                .upstream_client
                .execute_stream_with_provider_account_id(
                    &candidate.provider_account_id,
                    &refreshed_payload,
                    req,
                    model,
                    Some(extra_headers),
                )
                .await;
            if refreshed_attempt.is_err() {
                observe_provider_result_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    model,
                    &refreshed_attempt,
                    refreshed_attempt_started_at.elapsed().as_millis() as u64,
                );
            }
            match refreshed_attempt {
                Ok(response) => Ok((response, refreshed_attempt_started_at)),
                Err(error)
                    if !chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && chatgpt_web_request_time_browser_path_needed(
                            &refreshed_payload,
                            &error,
                        ) =>
                {
                    Err(chatgpt_web_browser_fallback_forbidden_error(&error))
                }
                Err(error)
                    if chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && should_refresh_chatgpt_web_after_failure(&error) =>
                {
                    debug!(
                        provider = %candidate.provider_account_id,
                        credential_id = ?candidate.provider_credential_id,
                        code = ?error.code,
                        "ChatGPT Web stream start still challenged after refresh; escalating to browser relay"
                    );
                    provider_attempt_gate.admit().await?;
                    let relay_started_at = Instant::now();
                    let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                        &state.redis_pool,
                        state.pg_pool.as_ref(),
                        &refreshed_payload,
                        req,
                        model,
                        true,
                    )
                    .await;
                    if relay_result.is_err() {
                        observe_provider_result_metric(
                            global_gateway_metrics().as_ref(),
                            &candidate.provider_account_id,
                            model,
                            &relay_result,
                            relay_started_at.elapsed().as_millis() as u64,
                        );
                    }
                    let relay = relay_result?;
                    let upstream = futures::stream::once(async move {
                        Ok::<Bytes, rquest::Error>(Bytes::from(relay.body_text))
                    });
                    let translated =
                        chatgpt_web::translate_chatgpt_web_stream(upstream, model.to_string());
                    Ok((
                        UpstreamStreamingResponse::Bytes(Box::pin(translated)),
                        relay_started_at,
                    ))
                }
                Err(error)
                    if chatgpt_web_request_time_browser_allowed(&refreshed_payload)
                        && should_escalate_chatgpt_web_to_browser_relay(
                            &refreshed_payload,
                            &error,
                        ) =>
                {
                    debug!(
                        provider = %candidate.provider_account_id,
                        credential_id = ?candidate.provider_credential_id,
                        code = ?error.code,
                        message = %error.message,
                        "ChatGPT Web stream start still returned a stable server-side failure after refresh; escalating to browser relay"
                    );
                    provider_attempt_gate.admit().await?;
                    let relay_started_at = Instant::now();
                    let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                        &state.redis_pool,
                        state.pg_pool.as_ref(),
                        &refreshed_payload,
                        req,
                        model,
                        true,
                    )
                    .await;
                    if relay_result.is_err() {
                        observe_provider_result_metric(
                            global_gateway_metrics().as_ref(),
                            &candidate.provider_account_id,
                            model,
                            &relay_result,
                            relay_started_at.elapsed().as_millis() as u64,
                        );
                    }
                    let relay = relay_result?;
                    let upstream = futures::stream::once(async move {
                        Ok::<Bytes, rquest::Error>(Bytes::from(relay.body_text))
                    });
                    let translated =
                        chatgpt_web::translate_chatgpt_web_stream(upstream, model.to_string());
                    Ok((
                        UpstreamStreamingResponse::Bytes(Box::pin(translated)),
                        relay_started_at,
                    ))
                }
                Err(error) => Err(error),
            }
        }
        Err(error)
            if chatgpt_web_request_time_browser_allowed(payload)
                && should_escalate_chatgpt_web_to_browser_relay(payload, &error) =>
        {
            debug!(
                provider = %candidate.provider_account_id,
                credential_id = ?candidate.provider_credential_id,
                code = ?error.code,
                message = %error.message,
                "ChatGPT Web reverse stream start returned a stable server-side failure; escalating to browser relay"
            );
            provider_attempt_gate.admit().await?;
            let relay_started_at = Instant::now();
            let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                req,
                model,
                true,
            )
            .await;
            if relay_result.is_err() {
                observe_provider_result_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    model,
                    &relay_result,
                    relay_started_at.elapsed().as_millis() as u64,
                );
            }
            let relay = relay_result?;
            let upstream = futures::stream::once(async move {
                Ok::<Bytes, rquest::Error>(Bytes::from(relay.body_text))
            });
            let translated = chatgpt_web::translate_chatgpt_web_stream(upstream, model.to_string());
            Ok((
                UpstreamStreamingResponse::Bytes(Box::pin(translated)),
                relay_started_at,
            ))
        }
        Err(error) => Err(error),
    }
}

/// Determine the AIMD failure kind from the error.
fn classify_failure_kind(e: &GatewayError) -> FailureKind {
    if e.kind == crate::error::ErrorKind::RateLimit {
        FailureKind::RateLimited
    } else {
        FailureKind::General
    }
}

/// Return `true` when the error hint suggests we should try the next candidate.
fn should_try_next_candidate(e: &GatewayError) -> bool {
    if e.code.as_deref() == Some("rate_limit_admission_indeterminate") {
        return false;
    }
    match &e.fallback_hint {
        FallbackHint::FallbackProvider { .. } => true,
        FallbackHint::Retry { .. } => e.retryable,
        FallbackHint::Abort { .. } => false,
        FallbackHint::DowngradeModel { .. } => false,
    }
}

fn should_record_provider_failure(error: &GatewayError) -> bool {
    error.code.as_deref() != Some("rate_limit_admission_indeterminate")
}

fn is_gemini_canvas_video_quota_passthrough_adapter(adapter: &str) -> bool {
    matches!(
        adapter,
        "gemini_canvas_compatible"
            | "gemini_canvas_web_reverse_compatible"
            | "gemini_canvas_program_web_reverse_compatible"
    )
}

fn retry_policy_for_request(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> RetryPolicy {
    let mut policy = RetryPolicy::default();
    if payload.adapter == "gemini_canvas_compatible"
        && matches!(req.endpoint_kind, EndpointKind::ImagesEdits)
    {
        policy.max_retries = 0;
    }
    if is_gemini_canvas_video_quota_passthrough_adapter(payload.adapter.as_str())
        && matches!(req.endpoint_kind, EndpointKind::VideosGenerations)
    {
        policy.retryable_statuses.retain(|status| *status != 429);
    }
    if payload.adapter == "udio_compatible"
        && matches!(
            req.endpoint_kind,
            EndpointKind::ImagesGenerations
                | EndpointKind::MusicGenerations
                | EndpointKind::VideosGenerations
        )
    {
        policy.retryable_statuses.retain(|status| *status != 429);
    }
    policy
}

fn spawn_record_provider_success(
    state: Arc<AppState>,
    provider_account_id: String,
    provider_credential_id: Option<String>,
) {
    tokio::spawn(async move {
        if let Err(error) = provider_runtime::record_provider_candidate_success(
            &state,
            &provider_account_id,
            provider_credential_id.as_deref(),
        )
        .await
        {
            warn!(
                provider = %provider_account_id,
                error = %error,
                "failed to persist provider success"
            );
        }
    });
}

fn spawn_record_provider_failure(
    state: Arc<AppState>,
    provider_account_id: String,
    provider_credential_id: Option<String>,
    route_policy_config: Option<crate::db::GatewayRoutePolicyConfig>,
    message: String,
) {
    tokio::spawn(async move {
        if let Err(error) = provider_runtime::record_provider_candidate_failure(
            &state,
            &provider_account_id,
            provider_credential_id.as_deref(),
            route_policy_config.as_ref(),
            &message,
        )
        .await
        {
            warn!(
                provider = %provider_account_id,
                error = %error,
                "failed to persist provider failure"
            );
        }
    });
}

fn observe_provider_attempt_metric(
    metrics: &GatewayMetrics,
    provider: &str,
    model: &str,
    observation: RetryAttemptObservation<'_>,
) {
    let (success, failure_class) = if observation.succeeded {
        (true, None)
    } else {
        let failure_class = observation
            .error
            .map(|error| {
                classify_provider_failure(
                    error.http_status,
                    error.code.as_deref(),
                    Some(error.message.as_str()),
                )
                .class_name()
            })
            .or(Some("unknown"));
        (false, failure_class)
    };
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider,
        model: Some(model),
        success,
        latency_ms: observation.latency_ms,
        failure_class,
    });
}

fn observe_provider_result_metric<T>(
    metrics: &GatewayMetrics,
    provider: &str,
    model: &str,
    result: &Result<T, GatewayError>,
    latency_ms: u64,
) {
    observe_provider_attempt_metric(
        metrics,
        provider,
        model,
        RetryAttemptObservation {
            succeeded: result.is_ok(),
            latency_ms,
            error: result.as_ref().err(),
        },
    );
}

fn observe_provider_success_metric(
    metrics: &GatewayMetrics,
    provider: &str,
    model: &str,
    latency_ms: u64,
) {
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider,
        model: Some(model),
        success: true,
        latency_ms,
        failure_class: None,
    });
}

fn observe_provider_failure_metric(
    metrics: &GatewayMetrics,
    provider: &str,
    model: &str,
    latency_ms: u64,
    message: &str,
) {
    let classification = classify_provider_failure(None, None, Some(message));
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider,
        model: Some(model),
        success: false,
        latency_ms,
        failure_class: Some(classification.class_name()),
    });
}

/// Re-pack a [`CanonicalRelayResponse`] into the wire format that the caller
/// expects, based on the protocol family of the original request.
fn pack_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    resp: &crate::protocol::canonical::CanonicalRelayResponse,
) -> serde_json::Value {
    use crate::protocol::canonical::{EndpointKind, ProtocolFamily};

    let resp_id = format!("relay-{}", uuid::Uuid::new_v4());
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let reply_model = caller_visible_reply_model(req, &resp.model);

    match (req.protocol_family, req.endpoint_kind) {
        (ProtocolFamily::Anthropic, EndpointKind::Messages) => {
            crate::protocol::anthropic::build_messages_success(
                &resp_id,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
        (_, EndpointKind::Responses) => {
            // Responses API — use the proper Responses API shape.
            crate::protocol::responses::build_responses_success(
                &resp_id,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
        (_, EndpointKind::Completions) => {
            crate::protocol::openai::build_legacy_completions_success(
                &resp_id,
                created_at,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                resp.finish_reason.as_deref(),
            )
        }
        _ => {
            // OpenAI chat completions (default).
            crate::protocol::openai::build_chat_completions_success(
                &resp_id,
                created_at,
                reply_model.as_str(),
                &resp.text,
                resp.usage.as_ref(),
                &resp.tool_calls,
                resp.finish_reason.as_deref(),
            )
        }
    }
}

fn caller_visible_reply_model(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    upstream_model: &str,
) -> String {
    req.requested_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(upstream_model)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::CanonicalRelayRequest;
    use crate::protocol::canonical::{
        CanonicalRelayResponse, CanonicalTool, EndpointKind, ProtocolFamily, TokenUsage,
    };
    use crate::routing::candidate::{
        ProviderAccountPayload, ProviderExecutionMode, RouteCandidate,
    };
    use std::collections::HashMap;

    fn make_req(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole};
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
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
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn make_resp() -> CanonicalRelayResponse {
        CanonicalRelayResponse {
            model: "gpt-4o".to_string(),
            text: "Hello!".to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: 5,
                completion_tokens: 3,
                total_tokens: 8,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            tool_calls: vec![],
            upstream_status: Some(200),
            finish_reason: Some("stop".to_string()),
        }
    }

    fn make_candidate(adapter: &str) -> RouteCandidate {
        RouteCandidate {
            provider_account_id: "prov-1".to_string(),
            provider_credential_id: Some("cred-1".to_string()),
            label: "Provider".to_string(),
            payload: ProviderAccountPayload {
                adapter: adapter.to_string(),
                base_url: "https://api.example.com".to_string(),
                api_key: "sk-test".to_string(),
                credential_id: Some("cred-ref".to_string()),
                expires_at: None,
                runtime_state_object_key: None,
                account_name: None,
                execution_mode: None,
                endpoint_execution_modes: None,
                default_model: None,
                headers: HashMap::new(),
                auth_mode: None,
                anthropic_version: None,
                beta_headers: None,
                auth_header_name: None,
                auth_token: None,
                responses_path: None,
                chat_completions_path: None,
                completions_path: None,
                embeddings_path: None,
                audio_transcriptions_path: None,
                audio_speech_path: None,
                messages_path: None,
                search_path: None,
                fetch_path: None,
                research_path: None,
                balance_path: None,
                search_query_field: None,
                fetch_urls_field: None,
                extra_body: None,
                session_auth: None,
                keepalive: None,
            },
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            supported_protocol_families: vec!["openai_chat".to_string()],
            adapter: adapter.to_string(),
            model_alias: Some("gpt-4o".to_string()),
            upstream_model: Some("gpt-4o".to_string()),
            resolved_execution_mode: ProviderExecutionMode::DirectHttp,
            priority: 100,
            weight: 1,
            failure_count: 0,
            cooldown_until: None,
            routing_score: Some(0.95),
            routing_health_weight: Some(0.9),
            routing_capacity_weight: Some(1.0),
            routing_degraded: Some(false),
            routing_breaker_open: Some(false),
            routing_degradation_reasons: Vec::new(),
        }
    }

    #[test]
    fn pack_response_openai_chat_completions() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let resp = make_resp();
        let json = pack_response(&req, &resp);
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["choices"][0]["message"]["content"], "Hello!");
        assert_eq!(json["model"], "gpt-4o");
    }

    #[test]
    fn pack_response_anthropic_messages() {
        let req = make_req(ProtocolFamily::Anthropic, EndpointKind::Messages);
        let resp = make_resp();
        let json = pack_response(&req, &resp);
        assert_eq!(json["type"], "message");
        assert_eq!(json["content"][0]["text"], "Hello!");
    }

    #[test]
    fn pack_response_legacy_completions() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::Completions);
        let resp = make_resp();
        let json = pack_response(&req, &resp);
        assert_eq!(json["object"], "text_completion");
        assert_eq!(json["choices"][0]["text"], "Hello!");
        assert_eq!(json["model"], "gpt-4o");
    }

    #[test]
    fn pack_response_prefers_requested_model_over_upstream_model() {
        let req = make_req(ProtocolFamily::Anthropic, EndpointKind::Messages);
        let mut resp = make_resp();
        resp.model = "astron-code-latest".to_string();
        let json = pack_response(&req, &resp);
        assert_eq!(json["model"], "gpt-4o");
    }

    #[test]
    fn classify_failure_kind_rate_limit() {
        let e = GatewayError::rate_limited("too fast", 1000);
        assert_eq!(classify_failure_kind(&e), FailureKind::RateLimited);
    }

    #[test]
    fn classify_failure_kind_server_error() {
        let e = GatewayError::server_error("boom");
        assert_eq!(classify_failure_kind(&e), FailureKind::General);
    }

    #[test]
    fn should_try_next_on_server_error() {
        let e = GatewayError::server_error("upstream failed");
        assert!(should_try_next_candidate(&e));
    }

    #[test]
    fn should_not_try_next_on_auth_error() {
        let e = GatewayError::unauthorized("bad creds");
        assert!(!should_try_next_candidate(&e));
    }

    #[test]
    fn should_not_try_next_on_bad_request() {
        let e = GatewayError::bad_request("malformed");
        assert!(!should_try_next_candidate(&e));
    }

    #[test]
    fn should_not_try_next_on_indeterminate_rate_limit_admission() {
        let error =
            GatewayError::service_unavailable("rate-limit admission result is indeterminate")
                .with_code("rate_limit_admission_indeterminate");

        assert!(!should_try_next_candidate(&error));
    }

    #[test]
    fn indeterminate_rate_limit_admission_is_not_provider_failure_feedback() {
        let error =
            GatewayError::service_unavailable("rate-limit admission result is indeterminate")
                .with_code("rate_limit_admission_indeterminate");

        assert!(!should_record_provider_failure(&error));
        assert!(should_record_provider_failure(&GatewayError::server_error(
            "upstream unavailable",
        )));
    }

    #[test]
    fn chatgpt_web_escalates_stable_server_failures_with_access_token_only() {
        let candidate = make_candidate("chatgpt_web_reverse_compatible");
        let error = GatewayError::server_error("Internal Server Error");

        assert!(should_escalate_chatgpt_web_to_browser_relay(
            &candidate.payload,
            &error,
        ));
    }

    #[test]
    fn chatgpt_web_request_time_browser_is_allowed_by_default() {
        let candidate = make_candidate("chatgpt_web_reverse_compatible");

        assert!(chatgpt_web_request_time_browser_allowed(&candidate.payload));
    }

    #[test]
    fn chatgpt_web_request_time_browser_can_be_disabled_by_bool_payload_flag() {
        let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "requestTimeBrowserAllowed".to_string(),
            serde_json::json!(false),
        );
        candidate.payload.extra_body = Some(extra_body);

        assert!(!chatgpt_web_request_time_browser_allowed(
            &candidate.payload
        ));
    }

    #[test]
    fn chatgpt_web_request_time_browser_can_be_disabled_by_mode_payload_flag() {
        let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "requestTimeBrowserMode".to_string(),
            serde_json::json!("pure_http_only"),
        );
        candidate.payload.extra_body = Some(extra_body);

        assert!(!chatgpt_web_request_time_browser_allowed(
            &candidate.payload
        ));
    }

    #[test]
    fn chatgpt_web_browser_fallback_forbidden_error_is_explicit() {
        let error = chatgpt_web_browser_fallback_forbidden_error(&GatewayError::server_error(
            "Internal Server Error",
        ));

        assert_eq!(
            error.code.as_deref(),
            Some("chatgpt_web_request_time_browser_forbidden")
        );
        assert!(error.message.contains("pure HTTP"));
    }

    #[test]
    fn chatgpt_web_escalates_stable_server_failures_with_cookie_only_runtime() {
        let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
        candidate.payload.api_key = String::new();
        candidate.payload.headers.insert(
            "Cookie".to_string(),
            "__Secure-next-auth.session-token=abc; cf_clearance=def".to_string(),
        );
        let error = GatewayError::server_error("Internal Server Error");

        assert!(should_escalate_chatgpt_web_to_browser_relay(
            &candidate.payload,
            &error,
        ));
    }

    #[test]
    fn chatgpt_web_does_not_escalate_stable_server_failures_without_runtime_seed() {
        let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
        candidate.payload.api_key = String::new();
        let error = GatewayError::server_error("Internal Server Error");

        assert!(!should_escalate_chatgpt_web_to_browser_relay(
            &candidate.payload,
            &error,
        ));
    }

    #[test]
    fn retry_policy_for_udio_media_excludes_rate_limit_retries() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        let candidate = make_candidate("udio_compatible");

        let policy = retry_policy_for_request(&req, &candidate.payload);

        assert!(!policy.retryable_statuses.contains(&429));
        assert!(policy.retryable_statuses.contains(&500));
    }

    #[test]
    fn retry_policy_for_standard_chat_keeps_rate_limit_retries() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let candidate = make_candidate("openai_compatible");

        let policy = retry_policy_for_request(&req, &candidate.payload);

        assert!(policy.retryable_statuses.contains(&429));
    }

    #[test]
    fn retry_policy_for_gemini_canvas_image_edits_disables_provider_retries() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
        let candidate = make_candidate("gemini_canvas_compatible");

        let policy = retry_policy_for_request(&req, &candidate.payload);

        assert_eq!(policy.max_retries, 0);
    }

    #[test]
    fn retry_policy_for_gemini_canvas_program_video_excludes_rate_limit_retries() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let candidate = make_candidate("gemini_canvas_program_web_reverse_compatible");

        let policy = retry_policy_for_request(&req, &candidate.payload);

        assert!(!policy.retryable_statuses.contains(&429));
        assert!(policy.retryable_statuses.contains(&500));
    }

    #[test]
    fn retry_policy_for_gemini_canvas_browser_video_excludes_rate_limit_retries() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let candidate = make_candidate("gemini_canvas_web_reverse_compatible");

        let policy = retry_policy_for_request(&req, &candidate.payload);

        assert!(!policy.retryable_statuses.contains(&429));
    }

    #[test]
    fn selected_candidate_tracks_openai_responses_bridge_semantics() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("openai_compatible");
        candidate.payload.responses_path = Some("/v1/responses".to_string());

        set_selected_candidate(&mut ctx, &candidate, None, "gpt-4o", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some("openai_responses")
        );
        assert_eq!(
            ctx.selected_upstream_target_conversation_family.as_deref(),
            Some("openai_responses")
        );
        assert_eq!(
            ctx.canonical_conversation_semantics.as_deref(),
            Some("messages_turns")
        );
    }

    #[test]
    fn selected_candidate_tracks_native_modality_family() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("producer_compatible");
        candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();

        set_selected_candidate(&mut ctx, &candidate, None, "producer-base", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some(PRODUCER_MUSIC_FAMILY)
        );
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
        assert!(ctx.selected_upstream_target_tool_family.is_none());
    }

    #[test]
    fn selected_candidate_tracks_producer_image_family() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("producer_compatible");
        candidate.protocol_family = "producer".to_string();
        candidate.protocol_profile = "producer".to_string();

        set_selected_candidate(&mut ctx, &candidate, None, "producer:image", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some(PRODUCER_IMAGES_FAMILY)
        );
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
    }

    #[test]
    fn selected_candidate_tracks_lumalabs_video_family() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("lumalabs_compatible");
        candidate.protocol_family = "lumalabs".to_string();
        candidate.protocol_profile = "lumalabs".to_string();

        set_selected_candidate(&mut ctx, &candidate, None, "ray-2", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some(LUMALABS_VIDEOS_FAMILY)
        );
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
    }

    #[test]
    fn selected_candidate_tracks_suno_video_family() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("suno_compatible");
        candidate.protocol_family = "suno".to_string();
        candidate.protocol_profile = "suno".to_string();

        set_selected_candidate(&mut ctx, &candidate, None, "chirp-v3-5", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some(SUNO_VIDEOS_FAMILY)
        );
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
    }

    #[test]
    fn selected_candidate_tracks_udio_video_family() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let mut candidate = make_candidate("udio_compatible");
        candidate.protocol_family = "udio".to_string();
        candidate.protocol_profile = "udio".to_string();

        set_selected_candidate(&mut ctx, &candidate, None, "udio-video", false);

        assert_eq!(
            ctx.selected_upstream_target_protocol_family.as_deref(),
            Some(UDIO_VIDEOS_FAMILY)
        );
        assert!(ctx.selected_upstream_target_conversation_family.is_none());
    }

    #[test]
    fn selected_candidate_tracks_prompt_only_when_tools_are_injected() {
        let mut req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        req.tools.push(CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: None,
            input_schema: Some(serde_json::json!({"type":"object"})),
            raw: HashMap::new(),
        });
        req.tool_choice = Some(serde_json::json!("required"));
        let mut ctx = crate::pipeline::PipelineContext::new(req, None);
        let candidate = make_candidate("openai_compatible");

        set_selected_candidate(&mut ctx, &candidate, None, "gpt-4o", true);

        assert_eq!(
            ctx.selected_upstream_target_tool_family.as_deref(),
            Some("xml_fallback")
        );
        assert_eq!(ctx.tool_strategy.as_deref(), Some("xml_fallback"));
        assert_eq!(
            ctx.canonical_tool_choice_semantics.as_deref(),
            Some("prompt_only")
        );
    }

    #[test]
    fn xml_tool_response_bridge_promotes_upstream_xml_when_tools_requested() {
        let mut canonical_resp = CanonicalRelayResponse {
            model: "xop35qwen2b".to_string(),
            text: "<tool_calls>\n<tool_call>\n<tool_name>weather</tool_name>\n<parameters>{\"city\":\"Hangzhou\"}</parameters>\n</tool_call>\n</tool_calls>".to_string(),
            usage: None,
            tool_calls: Vec::new(),
            upstream_status: Some(200),
            finish_reason: Some("stop".to_string()),
        };
        let tools = vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: Some("Return weather".to_string()),
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"],
            })),
            raw: HashMap::new(),
        }];

        let changed = maybe_apply_xml_tool_response_bridge(
            &uuid::Uuid::nil(),
            &mut canonical_resp,
            &tools,
            Some(&serde_json::json!("required")),
            Some("Use only the weather tool for Hangzhou."),
            false,
        );

        assert!(changed);
        assert!(canonical_resp.text.is_empty());
        assert_eq!(canonical_resp.tool_calls.len(), 1);
        assert_eq!(
            canonical_resp.tool_calls[0].name.as_deref(),
            Some("weather")
        );
        assert_eq!(canonical_resp.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn xml_tool_response_bridge_skips_when_native_tool_calls_already_present() {
        let mut canonical_resp = CanonicalRelayResponse {
            model: "xop35qwen2b".to_string(),
            text: "<tool_calls><tool_call><tool_name>weather</tool_name></tool_call></tool_calls>"
                .to_string(),
            usage: None,
            tool_calls: vec![crate::protocol::canonical::CanonicalToolCall {
                id: Some("call_weather".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: HashMap::new(),
            }],
            upstream_status: Some(200),
            finish_reason: Some("tool_calls".to_string()),
        };
        let tools = vec![CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: None,
            input_schema: Some(serde_json::json!({"type": "object"})),
            raw: HashMap::new(),
        }];

        let changed = maybe_apply_xml_tool_response_bridge(
            &uuid::Uuid::nil(),
            &mut canonical_resp,
            &tools,
            None,
            None,
            true,
        );

        assert!(!changed);
        assert_eq!(canonical_resp.tool_calls.len(), 1);
    }

    #[test]
    fn canonical_completion_semantics_maps_content_filter() {
        let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let resp = CanonicalRelayResponse {
            finish_reason: Some("content_filter".to_string()),
            ..make_resp()
        };

        assert_eq!(
            infer_canonical_completion_semantics(&req, &resp),
            Some("content_filter")
        );
    }

    #[tokio::test]
    async fn empty_candidates_returns_error() {
        use crate::concurrency::aimd::AimdConfig;
        use crate::concurrency::registry::ConcurrencyRegistry;
        use crate::config::Config;
        use crate::pipeline::PipelineContext;
        use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole};
        use crate::upstream::client::UpstreamClient;

        let state = Arc::new(AppState {
            config: Config {
                console: Default::default(),
                runtime_role: crate::config::GatewayRuntimeRole::Standalone,
                port: 4200,
                redis_url: "redis://localhost".to_string(),
                database_url: None,
                upstream_timeout_secs: 30,
                max_request_body_bytes: 1024 * 1024,
                max_body_chat_completions_bytes: 1024 * 1024,
                max_body_completions_bytes: 1024 * 1024,
                max_body_messages_bytes: 1024 * 1024,
                max_body_responses_bytes: 1024 * 1024,
                max_body_embeddings_bytes: 1024 * 1024,
                max_body_audio_transcriptions_bytes: 8 * 1024 * 1024,
                max_body_audio_speech_bytes: 1024 * 1024,
                max_body_search_bytes: 512 * 1024,
                max_body_fetch_bytes: 512 * 1024,
                max_body_research_bytes: 512 * 1024,
                max_body_images_generations_bytes: 1024 * 1024,
                max_body_images_edits_bytes: 1024 * 1024,
                max_body_music_bytes: 1024 * 1024,
                max_body_videos_bytes: 1024 * 1024,
                response_cache_ttl_secs: 300,
                response_cache_max_size_bytes: 512 * 1024,
                quota_pre_deduct_estimate_ratio: 1.2,
                usage_report_batch_size: 100,
                provider_probe_interval_secs: 30,
                log_level: "info".to_string(),
                gateway_api_key: None,
                gateway_api_key_secret: None,
                gateway_management_token: None,
                gateway_keepalive_bearer_token: None,
                default_project_id: "platform-default-project".to_string(),
                gateway_inbound_api_key_header_aliases: Vec::new(),
                provider_credential_folder_sync_enabled: false,
                provider_credential_folder_sync_root_dir: None,
                provider_credential_folder_sync_interval_secs: 30,
                provider_credential_folder_sync_import_enabled: true,
                provider_credential_folder_sync_export_enabled: true,
                provider_credential_folder_sync_watch_enabled: true,
                provider_credential_folder_sync_watch_debounce_millis: 1500,
                provider_credential_folder_sync_delete_missing: false,
                provider_credential_refresh_enabled: true,
                provider_credential_refresh_interval_secs: 3600,
                provider_credential_refresh_before_secs: 86_400,
                provider_credential_refresh_batch_limit: 100,
                provider_credential_refresh_lock_ttl_secs: 300,
                credential_stock_monitor_enabled: true,
                credential_stock_monitor_interval_secs: 60,
                splitter_worker_executable_path: None,
                splitter_initial_worker_port: 4201,
                splitter_ready_timeout_secs: 120,
                splitter_ready_poll_interval_millis: 500,
                splitter_reload_shutdown_timeout_secs: 600,
            },
            redis_pool: deadpool_redis::Config::from_url("redis://localhost:6379")
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .expect("pool"),
            pg_pool: None,
            upstream_client: UpstreamClient::new(30),
            concurrency_registry: ConcurrencyRegistry::new(AimdConfig::default()),
            auth_adapters: vec![],
            filter_config: None,
            route_config: Arc::new(crate::routing::config::RouteConfigStore::new()),
            route_config_runtime: None,
            credential_cache: crate::credential_store::CredentialMemoryCache::new(30),
            lifecycle: crate::state::GatewayLifecycleState::default(),
            shutdown: crate::state::GatewayShutdownHandle::default(),
            provider_credential_folder_sync: crate::state::ProviderCredentialFolderSyncRuntime::new(
                false,
            ),
        });

        let req = CanonicalRelayRequest {
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
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let mut ctx = PipelineContext::new(req, None);
        // candidates is empty by default
        let result = run(&mut ctx, &state).await;
        assert!(result.is_err());
    }

    #[test]
    fn provider_metric_helpers_record_success_and_classified_failure() {
        let metrics = crate::metrics::request::GatewayMetrics::new();
        observe_provider_success_metric(&metrics, "openai", "gpt-4o", 25);
        observe_provider_failure_metric(
            &metrics,
            "openai",
            "gpt-4o",
            120,
            "rate limit exceeded with too many requests",
        );

        let output = metrics.render_prometheus();
        assert!(output
            .contains("gateway_provider_requests_total{provider=\"openai\",model=\"gpt-4o\"} 2"));
        assert!(output.contains("failure_class=\"rate_limited\""));
    }

    #[test]
    fn provider_attempt_metric_observer_uses_resolved_model_and_elapsed_attempt_latency() {
        let metrics = crate::metrics::request::GatewayMetrics::new();
        observe_provider_attempt_metric(
            &metrics,
            "provider-account-1",
            "gpt-5.3-codex",
            RetryAttemptObservation {
                succeeded: true,
                latency_ms: 37,
                error: None,
            },
        );
        let error = GatewayError::server_error("upstream unavailable");
        observe_provider_attempt_metric(
            &metrics,
            "provider-account-1",
            "gpt-5.3-codex",
            RetryAttemptObservation {
                succeeded: false,
                latency_ms: 91,
                error: Some(&error),
            },
        );

        let output = metrics.render_prometheus();
        assert!(output.contains(
            "gateway_provider_requests_total{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 2"
        ));
        assert!(output.contains(
            "gateway_provider_latency_ms_sum{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 128"
        ));
        assert!(!output.contains("unknown_model"));
        assert!(!output
            .contains("latency_ms_sum{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 0"));
    }
}
