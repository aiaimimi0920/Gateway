//! Streaming dispatch, selected-state capture and terminal callback ownership.
use super::*;
#[path = "stream_terminal.rs"]
pub(super) mod terminal;

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    if attempt.effective_payload.auth_mode.as_deref() == Some("copilot_sdk") {
        return Err(AttemptError::Stop(
            super::super::copilot_dispatch::unavailable(),
        ));
    }
    let mut stream_retry_policy = attempt.retry_policy.clone();
    let capacity_retry_allowed = stream_retry_policy.max_retries > 0;
    let terminal_gate = attempt.provider_attempt_gate.clone();
    let sticky = state.lifecycle.smart_routing.sticky(
        super::super::stage_route::smart::session_scope(ctx).as_deref(),
        ctx.canonical_req
            .requested_model
            .as_deref()
            .unwrap_or_default(),
    );
    if sticky.as_deref() != Some(&candidate.provider_account_id) {
        stream_retry_policy.max_retries = 0;
    }
    let PreparedAttempt {
        ref effective_payload,
        ref model,
        ref provider_attempt_gate,
        ..
    } = attempt;
    if candidate.adapter == "freebuff_compatible" {
        provider_attempt_gate
            .begin_attempt()
            .map_err(AttemptError::Stop)?;
    }
    let stream_attempt_started_at = Instant::now();
    // Recovery owns its individual sends; a guard here must not count their errors twice.
    let cancellation = (!crate::protocol::sap_aicore::owns_payload(effective_payload)
        && !matches!(
            candidate.adapter.as_str(),
            "qwen_web_compatible" | "chatgpt_web_reverse_compatible"
        ))
    .then(|| {
        ProviderAttemptCancellation::new(
            global_gateway_metrics(),
            &candidate.provider_account_id,
            model,
            stream_attempt_started_at,
        )
    });
    let stream_result = if crate::protocol::sap_aicore::owns_payload(effective_payload) {
        buffered::sap_dispatch::run(
            state,
            provider_attempt_gate,
            effective_payload,
            &ctx.canonical_req,
            model,
            &mut ctx.observed_upstream_status,
            |response| async {
                crate::protocol::sap_aicore::runtime::decode_stream(
                    response,
                    effective_payload,
                    &ctx.canonical_req,
                    model,
                )
            },
        )
        .await
        .map(|response| (response, None, stream_attempt_started_at))
    } else if candidate.adapter == "freebuff_compatible" {
        freebuff::execute_stream(
            &state.upstream_client.freebuff,
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
        provider_attempt_gate
            .execute_with_capacity_retry(
                || {
                    state.upstream_client.execute_stream_with_wire(
                        &effective_payload,
                        &ctx.canonical_req,
                        &model,
                        Some(&ctx.request_headers),
                        ctx.native_wire.as_deref(),
                    )
                },
                |_| {},
                &stream_retry_policy,
                capacity_retry_allowed,
            )
            .await
            .map(|response| (response, None, stream_attempt_started_at))
    };
    if candidate.adapter == "freebuff_compatible" {
        // Recovery helpers already record actual sends, not their later admission errors.
        provider_attempt_gate.observe_result(&stream_result);
    }
    match stream_result {
        Ok((stream_response, freebuff_lease_handle, stream_started_at)) => {
            // Candidate-local state is attached only after successful preflight.
            let response_metadata = match &stream_response {
                UpstreamStreamingResponse::Native(response) => {
                    crate::upstream::native_response_metadata::NativeResponseMetadata::read(
                        response.headers(),
                    )
                }
                _ => Default::default(),
            };
            let cancellation = cancellation.unwrap_or_else(|| {
                ProviderAttemptCancellation::new(
                    global_gateway_metrics(),
                    &candidate.provider_account_id,
                    model,
                    stream_started_at,
                )
            });
            let normalized = stream_preflight::normalize(
                ctx,
                state,
                candidate,
                stream_response,
                stream_started_at,
                attempt,
            )
            .await;
            cancellation.disarm();
            let (byte_stream, attempt) = normalized?;
            let (byte_stream, stream_usage_handle) =
                stream_translation::translate(ctx, candidate, &attempt, byte_stream);
            let PreparedAttempt {
                route_policy_config,
                model,
                controller,
                permit,
                tools_were_injected,
                ..
            } = attempt;
            let (byte_stream, stream_usage_handle) = if candidate.adapter == "kiro_compatible" {
                let (tapped_stream, usage_handle) = tap_sse_usage_with_error(byte_stream);
                (Box::pin(tapped_stream) as ByteStream, Some(usage_handle))
            } else {
                (byte_stream, stream_usage_handle)
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
            // Hold concurrency permit in terminal::Owner across the active SSE body so AIMD
            // active_concurrency and capacity-weighted routing reflect live streaming load.
            let permit = Some(permit);

            let mut tracked = terminal::track(
                ctx,
                state,
                candidate,
                terminal::Owner {
                    stream: byte_stream,
                    permit,
                    usage: stream_usage_handle,
                    native_semantics: None,
                    model,
                    route_policy_config,
                    controller,
                    gate: terminal_gate,
                    started_at: stream_started_at,
                    legacy_lease: freebuff_lease_handle,
                    terminal: None,
                },
            );
            tracked.native_response_metadata = response_metadata;
            return Ok(PipelineOutput::Sse(tracked));
        }
        Err(e) => {
            if let Some(cancellation) = cancellation {
                cancellation.disarm();
            }
            let PreparedAttempt {
                route_policy_config,
                model,
                controller,
                permit,
                ..
            } = attempt;
            if should_record_provider_failure(&e) {
                let failure_kind = classify_failure_kind(&e);
                controller.on_failure(failure_kind);
                if !matches!(
                    candidate.adapter.as_str(),
                    "qwen_web_compatible" | "chatgpt_web_reverse_compatible"
                ) && !crate::protocol::sap_aicore::owns_payload(&candidate.payload)
                {
                    observe_provider_failure_metric(
                        global_gateway_metrics().as_ref(),
                        &candidate.provider_account_id,
                        &model,
                        stream_attempt_started_at.elapsed().as_millis() as u64,
                        &e.message,
                    );
                }
                spawn_record_provider_failure(
                    ctx.started_at,
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
                return Err(AttemptError::Next(e));
            } else {
                return Err(AttemptError::Stop(e));
            }
        }
    }
}
