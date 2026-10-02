//! Buffered Anthropic Responses requests accumulated through the existing SSE bridge.
use super::*;

pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    candidate_index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    let PreparedAttempt {
        effective_payload,
        model,
        reply_model,
        original_tools,
        original_tool_choice,
        original_messages_text,
        controller,
        permit,
        route_policy_config,
        tools_were_injected,
        ..
    } = attempt;
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
                    return Err(AttemptError::Next(failure));
                } else {
                    return Err(AttemptError::Stop(failure));
                }
            }

            match responses_bridge_stream::accumulate(
                response.bytes_stream(),
                &ctx.req_id,
                &reply_model,
                original_tools.clone(),
                original_tool_choice.clone(),
                Some(original_messages_text.clone()),
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
                        infer_canonical_completion_semantics(&ctx.canonical_req, &canonical_resp)
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
                        return Err(AttemptError::Next(error));
                    } else {
                        return Err(AttemptError::Stop(error));
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
                return Err(AttemptError::Next(error));
            } else {
                return Err(AttemptError::Stop(error));
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
                return Err(AttemptError::Next(error));
            } else {
                return Err(AttemptError::Stop(error));
            }
        }
    }
}
