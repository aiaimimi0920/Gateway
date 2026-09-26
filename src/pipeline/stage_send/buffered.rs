//! Buffered binary, JSON and canonical dispatch with unchanged retry snapshots.
use super::*;

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    if attempt.tools_were_injected
        && ctx.canonical_req.endpoint_kind == EndpointKind::Responses
        && candidate.adapter == "anthropic_compatible"
    {
        return responses_bridge::send(ctx, state, candidate, candidate_index, attempt).await;
    }
    let PreparedAttempt {
        route_policy_config,
        provider_attempt_gate,
        model,
        effective_payload,
        retry_policy,
        original_tools,
        original_tool_choice,
        original_messages_text,
        tools_were_injected,
        controller,
        permit,
        ..
    } = attempt;
    // Retry attempts share immutable snapshots instead of cloning the
    // full request, credentials, and headers before every send.
    let payload = Arc::new(effective_payload);
    let canonical_req = Arc::new(ctx.canonical_req.clone());
    let extra_hdrs = Arc::new(ctx.request_headers.clone());
    let provider_account_id = candidate.provider_account_id.clone();
    let client = &state.upstream_client;

    if expects_binary_passthrough(&ctx.canonical_req) {
        let metric_provider = candidate.provider_account_id.clone();
        let metric_model = model.clone();
        let result = execute_with_retry_after_admission_observed(
            || {
                let payload = Arc::clone(&payload);
                let canonical_req = Arc::clone(&canonical_req);
                let model = model.clone();
                let extra_hdrs = Arc::clone(&extra_hdrs);
                let provider_account_id = provider_account_id.clone();
                async move {
                    client
                        .execute_binary_passthrough_with_provider_account_id(
                            &provider_account_id,
                            payload.as_ref(),
                            canonical_req.as_ref(),
                            &model,
                            Some(extra_hdrs.as_ref()),
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
                    return Err(AttemptError::Next(e));
                } else {
                    return Err(AttemptError::Stop(e));
                }
            }
        }
    }

    if expects_json_passthrough(&ctx.canonical_req) {
        let metric_provider = candidate.provider_account_id.clone();
        let metric_model = model.clone();
        let result = execute_with_retry_after_admission_observed(
            || {
                let payload = Arc::clone(&payload);
                let canonical_req = Arc::clone(&canonical_req);
                let model = model.clone();
                let extra_hdrs = Arc::clone(&extra_hdrs);
                async move {
                    client
                        .execute_json_passthrough(
                            &candidate.provider_account_id,
                            candidate.resolved_execution_mode,
                            payload.as_ref(),
                            canonical_req.as_ref(),
                            &model,
                            Some(extra_hdrs.as_ref()),
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
                    return Err(AttemptError::Next(e));
                } else {
                    return Err(AttemptError::Stop(e));
                }
            }
        }
    }

    let result = if candidate.adapter == "qwen_web_compatible" {
        execute_qwen_web_nonstream_with_recovery(
            state,
            candidate,
            &provider_attempt_gate,
            payload.as_ref(),
            canonical_req.as_ref(),
            &model,
            extra_hdrs.as_ref(),
            &retry_policy,
        )
        .await
    } else if candidate.adapter == "chatgpt_web_reverse_compatible" {
        execute_chatgpt_web_nonstream_with_recovery(
            state,
            candidate,
            &provider_attempt_gate,
            payload.as_ref(),
            canonical_req.as_ref(),
            &model,
            extra_hdrs.as_ref(),
            &retry_policy,
        )
        .await
    } else {
        let metric_provider = candidate.provider_account_id.clone();
        let metric_model = model.clone();
        execute_with_retry_after_admission_observed(
            || {
                let payload = Arc::clone(&payload);
                let canonical_req = Arc::clone(&canonical_req);
                let model = model.clone();
                let extra_hdrs = Arc::clone(&extra_hdrs);
                let provider_account_id = provider_account_id.clone();
                async move {
                    if candidate.adapter == "freebuff_compatible" {
                        freebuff::execute(
                            &client.freebuff,
                            client.client(),
                            payload.as_ref(),
                            canonical_req.as_ref(),
                            &model,
                            Some(extra_hdrs.as_ref()),
                        )
                        .await
                    } else {
                        client
                            .execute_with_provider_account_id(
                                &provider_account_id,
                                payload.as_ref(),
                                canonical_req.as_ref(),
                                &model,
                                Some(extra_hdrs.as_ref()),
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
                return Err(AttemptError::Next(e));
            } else {
                return Err(AttemptError::Stop(e));
            }
        }
    }
}
