//! Streaming dispatch, selected-state capture and terminal callback ownership.
use super::*;

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    let PreparedAttempt {
        ref effective_payload,
        ref model,
        ref provider_attempt_gate,
        ..
    } = attempt;
    let stream_attempt_started_at = Instant::now();
    let stream_result = if candidate.adapter == "freebuff_compatible" {
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
            let (byte_stream, attempt) = stream_preflight::normalize(
                ctx,
                state,
                candidate,
                stream_response,
                stream_started_at,
                attempt,
            )
            .await?;
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

            let (byte_stream, stream_completion_semantics_handle) =
                if is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
                    let (tapped_stream, completion_semantics_handle) =
                        tap_sse_completion_semantics_with_error(byte_stream);
                    (
                        Box::pin(tapped_stream) as ByteStream,
                        Some(completion_semantics_handle),
                    )
                } else {
                    (byte_stream, None)
                };

            let (byte_stream, stream_archive_handle) =
                if is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
                    let (tapped_stream, archive_handle) = tap_stream_archive_with_error(
                        byte_stream,
                        crate::conversation_archive::CONVERSATION_ARCHIVE_MAX_RESPONSE_BYTES,
                    );
                    (Box::pin(tapped_stream) as ByteStream, Some(archive_handle))
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
            let audit_snapshot = crate::pipeline::stage_finalize::snapshot_request_audit(ctx);
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
            let tracked = TrackedStream::new_with_started_at_and_error(
                byte_stream,
                stream_started_at,
                move |metrics, success| {
                    if let Some(lease_handle) = freebuff_lease_handle {
                        tokio::spawn(async move {
                            if success {
                                lease_handle.release().await;
                            } else {
                                lease_handle
                                    .invalidate("freebuff stream terminated before completion")
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
                        let usage = stream_usage_handle.as_ref().and_then(snapshot_tapped_usage);
                        let canonical_completion_semantics = stream_completion_semantics_handle
                            .as_ref()
                            .and_then(snapshot_tapped_completion_semantics);
                        let archive_snapshot =
                            stream_archive_handle.as_ref().map(snapshot_tapped_archive);
                        let finalizer = state_for_audit
                            .local_runtime
                            .as_ref()
                            .map(|local| local.track_finalizer());
                        tokio::spawn(async move {
                            let _finalizer = finalizer;
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
                        let finalizer = state_for_finalize
                            .local_runtime
                            .as_ref()
                            .map(|local| local.track_finalizer());
                        tokio::spawn(async move {
                            let _finalizer = finalizer;
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
                return Err(AttemptError::Next(e));
            } else {
                return Err(AttemptError::Stop(e));
            }
        }
    }
}
