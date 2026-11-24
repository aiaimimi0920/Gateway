//! Buffered binary, JSON and canonical dispatch with unchanged retry snapshots.
use super::*;
use crate::upstream::client::UpstreamClient;
pub(super) mod sap_dispatch;

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    if attempt.effective_payload.auth_mode.as_deref() == Some("copilot_sdk") {
        crate::protocol::copilot::single_turn(&ctx.canonical_req, &attempt.model)
            .map_err(AttemptError::Stop)?;
    }
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
    let native_wire = ctx.native_wire.clone();
    let provider_account_id = candidate.provider_account_id.clone();
    let client = &state.upstream_client;

    if expects_binary_passthrough(&ctx.canonical_req) {
        let metric_provider = candidate.provider_account_id.clone();
        let metric_model = model.clone();
        let result = provider_attempt_gate
            .execute_observed(
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

    // Keep product reasoning and custom-tool fields intact for native Responses clients.
    // SDK text owns its dispatch even when the generic OpenAI adapter prefers native Responses.
    let sensenova_native =
        crate::protocol::sensenova::native_selected(&payload, canonical_req.endpoint_kind);
    let sap_responses = crate::protocol::sap_aicore::native_responses::selected(
        &payload,
        canonical_req.endpoint_kind,
    );
    let sap_orchestration = crate::protocol::sap_aicore::orchestration::native_selected(
        &payload,
        canonical_req.endpoint_kind,
    );
    let native_text = payload.auth_mode.as_deref() != Some("copilot_sdk")
        && !tools_were_injected
        && (UpstreamClient::uses_native_codex(&payload, &canonical_req, &model)
            || payload.is_discovered_native(&canonical_req)
            || sensenova_native
            || sap_responses
            || sap_orchestration
            || crate::protocol::commandcode::native_json::selected(&payload, &canonical_req)
            || (crate::protocol::minimax::owns_payload(&payload)
                && canonical_req.endpoint_kind == EndpointKind::Messages)
            // Preserve Agnes response status/details without entering Codex transport.
            || (crate::protocol::agnes::owns_payload(&payload)
                && canonical_req.endpoint_kind == EndpointKind::Responses)
            || (payload.adapter == crate::protocol::chatgpt::basispoints::ADAPTER
                && canonical_req.endpoint_kind == EndpointKind::Responses));
    if expects_json_passthrough(&ctx.canonical_req) || native_text {
        let metric_provider = candidate.provider_account_id.clone();
        let metric_model = model.clone();
        let result = if crate::protocol::sap_aicore::owns_payload(&payload) {
            sap_dispatch::json(
                state,
                &provider_attempt_gate,
                &payload,
                &canonical_req,
                &model,
                &mut ctx.observed_upstream_status,
            )
            .await
        } else {
            provider_attempt_gate
                .execute_observed(
                    || {
                        let payload = Arc::clone(&payload);
                        let canonical_req = Arc::clone(&canonical_req);
                        let model = model.clone();
                        let extra_hdrs = Arc::clone(&extra_hdrs);
                        let native_wire = native_wire.clone();
                        // Native and legacy dispatch must not inflate every enclosing
                        // retry/gate frame. Same-task polling preserves cancellation.
                        Box::pin(async move {
                            client
                                .execute_json_with_wire(
                                    &candidate.provider_account_id,
                                    candidate.resolved_execution_mode,
                                    payload.as_ref(),
                                    canonical_req.as_ref(),
                                    &model,
                                    Some(extra_hdrs.as_ref()),
                                    native_wire.as_deref(),
                                )
                                .await
                        })
                    },
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
            Ok(mut json_resp) => {
                let declaration =
                    crate::protocol::model_declaration::ModelDeclaration::from_body(&json_resp);
                model_observation::buffered(ctx, candidate, &model, declaration.as_ref());
                if crate::protocol::commandcode::native_json::selected(&payload, &canonical_req) {
                    let (usage, completion) = crate::protocol::commandcode::native_json::observe(
                        &json_resp,
                        canonical_req.endpoint_kind,
                    );
                    ctx.observed_usage = Some(usage);
                    ctx.canonical_completion_semantics = Some(completion.into());
                } else if sensenova_native || sap_responses {
                    let (usage, completion) = match canonical_req.endpoint_kind {
                        EndpointKind::ChatCompletions => {
                            crate::protocol::sensenova::chat_reasoning::observe(&json_resp)
                        }
                        EndpointKind::Responses => {
                            crate::protocol::responses::native_admission::observe(&json_resp)
                        }
                        _ => crate::protocol::anthropic::native_admission::observe(&json_resp),
                    };
                    ctx.observed_usage = Some(usage);
                    ctx.canonical_completion_semantics = Some(completion.into());
                } else if native_text {
                    let observed = if crate::protocol::dashscope::is_dashscope(&ctx.canonical_req) {
                        crate::protocol::dashscope::response::observe_native(&json_resp, &model)
                    } else {
                        match ctx.canonical_req.endpoint_kind {
                            EndpointKind::Messages => {
                                crate::protocol::anthropic::unpack_anthropic_response(&json_resp)
                            }
                            EndpointKind::Responses => {
                                crate::protocol::responses::unpack_responses_response(&json_resp)
                            }
                            _ => crate::protocol::openai::unpack_openai_response(&json_resp),
                        }
                    };
                    if let Ok(observed) = observed {
                        ctx.observed_usage = observed.usage.clone().map(Some);
                        ctx.canonical_completion_semantics =
                            infer_canonical_completion_semantics(&ctx.canonical_req, &observed)
                                .map(str::to_string);
                    }
                }
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

                if sensenova_native
                    || sap_responses
                    || sap_orchestration
                    || crate::protocol::commandcode::native_json::selected(&payload, &canonical_req)
                {
                    // Observe the real model first; only the public projection uses the caller alias.
                    json_resp["model"] =
                        pack::caller_visible_reply_model(&canonical_req, &model).into();
                }
                return Ok(PipelineOutput::Json(json_resp));
            }
            Err(e) => {
                if should_record_provider_failure(&e) {
                    let failure_kind = classify_failure_kind(&e);
                    controller.on_failure(failure_kind);
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

    let result = if crate::protocol::sap_aicore::owns_payload(&payload) {
        sap_dispatch::canonical(
            state,
            &provider_attempt_gate,
            &payload,
            &canonical_req,
            &model,
            &mut ctx.observed_upstream_status,
        )
        .await
    } else if payload.auth_mode.as_deref() == Some("copilot_sdk") {
        // SDK failures and cancellation have no upstream HTTP response either.
        ctx.observed_upstream_status = Some(None);
        super::super::copilot_dispatch::execute(
            state,
            ctx,
            candidate,
            &model,
            &provider_attempt_gate,
            |observation| {
                observe_provider_attempt_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    &model,
                    observation,
                )
            },
        )
        .await
    } else if candidate.adapter == "qwen_web_compatible" {
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
        provider_attempt_gate
            .execute_observed(
                || {
                    let payload = Arc::clone(&payload);
                    let canonical_req = Arc::clone(&canonical_req);
                    let model = model.clone();
                    let extra_hdrs = Arc::clone(&extra_hdrs);
                    let provider_account_id = provider_account_id.clone();
                    // Keep the legacy multi-provider future out of every enclosing
                    // retry/gate frame. Same-task polling preserves cancellation
                    // and the admission lease; no extra task or send is created.
                    Box::pin(async move {
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
                    })
                },
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
            model_observation::buffered(
                ctx,
                candidate,
                &crate::protocol::lobsterai::virtual_model::observed_model(
                    payload.as_ref(),
                    &model,
                ),
                canonical_resp.model_declaration.as_ref(),
            );
            controller.on_success();
            spawn_record_provider_success(
                Arc::clone(state),
                candidate.provider_account_id.clone(),
                candidate.provider_credential_id.clone(),
            );
            drop(permit);

            // If we injected tools, parse XML tool calls from the
            // response text and convert to standard tool_calls format.
            if !original_tools.is_empty()
                && !crate::protocol::dim_oauth::business::owns_payload(payload.as_ref())
                && !crate::protocol::nous_portal::business::owns_payload(payload.as_ref())
            {
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
            ctx.observed_usage = Some(canonical_resp.usage.clone());
            ctx.observed_upstream_status = ctx.observed_upstream_status.or_else(|| {
                upstream_status_observation(
                    payload.auth_mode.as_deref(),
                    canonical_resp.upstream_status,
                )
            });

            // Pack the response back into the wire format matching
            // the original protocol family.
            if ctx.canonical_req.endpoint_kind != EndpointKind::Messages {
                canonical_resp.usage = crate::protocol::lobsterai::virtual_model::chat_usage(
                    ctx.lobster_model_resolution.as_ref(),
                    canonical_resp.usage.take(),
                );
            }
            let json_resp = pack_response(&ctx.canonical_req, &canonical_resp);
            return Ok(PipelineOutput::Json(json_resp));
        }
        Err(e) => {
            if should_record_provider_failure(&e) {
                let failure_kind = classify_failure_kind(&e);
                controller.on_failure(failure_kind);
                spawn_record_provider_failure(
                    ctx.started_at,
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

pub(super) fn upstream_status_observation(
    auth_mode: Option<&str>,
    status: Option<u16>,
) -> Option<Option<u16>> {
    // Legacy HTTP adapters may omit status. Only a known non-HTTP execution proves explicit absence.
    if auth_mode == Some("copilot_sdk") {
        Some(status)
    } else {
        status.map(Some)
    }
}
