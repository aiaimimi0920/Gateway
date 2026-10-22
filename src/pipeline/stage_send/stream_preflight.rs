//! Normalize upstream bytes while preserving first-event failure and permit handling.
use super::*;
use futures::TryStreamExt;
#[path = "early_stream.rs"]
mod early_stream;

pub(super) async fn normalize(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    stream_response: UpstreamStreamingResponse,
    stream_started_at: Instant,
    attempt: PreparedAttempt,
) -> Result<(ByteStream, PreparedAttempt), AttemptError> {
    let PreparedAttempt {
        ref controller,
        ref route_policy_config,
        ref model,
        ..
    } = attempt;
    let needs_completion = matches!(&stream_response, UpstreamStreamingResponse::Native(_))
        || (matches!(&stream_response, UpstreamStreamingResponse::Http(_))
            && attempt.effective_payload.canonical_adapter() == "openai_compatible"
            && ctx.canonical_req.endpoint_kind == EndpointKind::Responses
            && !attempt
                .effective_payload
                .bridges_openai_responses_to_chat_completions(ctx.canonical_req.endpoint_kind));
    let byte_stream: ByteStream = match stream_response {
        UpstreamStreamingResponse::NativeEvents(stream) => {
            // WS transport validates terminal events itself, after exposing the
            // original failure event to the caller and before final settlement.
            stream
        }
        UpstreamStreamingResponse::Native(response) => {
            let stream = crate::upstream::stream::tap_model_declaration(
                response
                    .bytes_stream()
                    .map_err(|e| StreamError::NativeTransport(e.without_url())),
                model_observation::metadata(ctx, candidate, model),
                Arc::clone(global_gateway_metrics()),
                true,
            );
            Box::pin(stream)
        }
        UpstreamStreamingResponse::Bytes(stream) => {
            Box::pin(crate::upstream::stream::tap_model_declaration(
                stream.map_err(StreamError::Transport),
                model_observation::metadata(ctx, candidate, model),
                Arc::clone(global_gateway_metrics()),
                false,
            ))
        }
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
                    Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>,
                >;

            let first = upstream.next().await;
            let first_chunk = match first {
                Some(Ok(chunk)) => chunk,
                Some(Err(err)) => {
                    let error =
                        crate::error::classify_network_error(&err, Some(&candidate.adapter));
                    attempt.provider_attempt_gate.observe_error(&error);
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
                    drop(attempt.permit);

                    if should_try_next_candidate(&error) {
                        warn!(
                            req_id = %ctx.req_id,
                            provider = %candidate.provider_account_id,
                            error = %error,
                            "candidate failed during generic tool-stream preflight; trying next"
                        );
                        return Err(AttemptError::Next(error));
                    } else {
                        return Err(AttemptError::Stop(error));
                    }
                }
                None => {
                    let error = GatewayError::server_error(
                        "Streaming adapter ended before any events were received",
                    )
                    .with_provider(candidate.adapter.as_str());
                    attempt.provider_attempt_gate.observe_error(&error);
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
                    drop(attempt.permit);

                    if should_try_next_candidate(&error) {
                        warn!(
                            req_id = %ctx.req_id,
                            provider = %candidate.provider_account_id,
                            error = %error,
                            "candidate returned an empty tool stream; trying next"
                        );
                        return Err(AttemptError::Next(error));
                    } else {
                        return Err(AttemptError::Stop(error));
                    }
                }
            };

            if let Some(error) = accio::detect_accio_provider_error(first_chunk.as_ref()) {
                attempt.provider_attempt_gate.observe_error(&error);
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
                drop(attempt.permit);

                if should_try_next_candidate(&error) {
                    warn!(
                        req_id = %ctx.req_id,
                        provider = %candidate.provider_account_id,
                        error = %error,
                        "streaming tool-adapter preflight failed; trying next"
                    );
                    return Err(AttemptError::Next(error));
                } else {
                    return Err(AttemptError::Stop(error));
                }
            }

            let replay =
                futures::stream::once(
                    async move { Ok::<bytes::Bytes, rquest::Error>(first_chunk) },
                )
                .chain(upstream);
            let replay = crate::upstream::stream::tap_model_declaration(
                replay,
                model_observation::metadata(ctx, candidate, model),
                Arc::clone(global_gateway_metrics()),
                true,
            );

            // 首包仍按原始 HTTP 错误分类；进入协议转换后才建立领域错误边界。
            Box::pin(accio::translate_anthropic_like_stream_to_openai_with_error(
                replay.map_err(StreamError::Transport),
                model.clone(),
            ))
        }
        UpstreamStreamingResponse::Http(response) if candidate.adapter == "grok_compatible" => {
            let stream = crate::upstream::stream::tap_model_declaration(
                response.bytes_stream(),
                model_observation::metadata(ctx, candidate, model),
                Arc::clone(global_gateway_metrics()),
                true,
            );
            Box::pin(
                grok::translate_grok_stream(stream, model.clone()).map_err(StreamError::Transport),
            )
        }
        UpstreamStreamingResponse::Http(response) => {
            let stream = crate::upstream::stream::tap_model_declaration(
                response.bytes_stream().map_err(StreamError::Transport),
                model_observation::metadata(ctx, candidate, model),
                Arc::clone(global_gateway_metrics()),
                true,
            );
            Box::pin(stream)
        }
    };
    let byte_stream = if candidate.payload.canonical_adapter() == "openai_compatible"
        && is_conversation_endpoint(ctx.canonical_req.endpoint_kind)
    {
        match early_stream::check(byte_stream).await {
            Ok(stream) => stream,
            Err(error) => {
                attempt.provider_attempt_gate.observe_error(&error);
                controller.on_failure(classify_failure_kind(&error));
                observe_provider_failure_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    model,
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
                return Err(if should_try_next_candidate(&error) {
                    AttemptError::Next(error)
                } else {
                    AttemptError::Stop(error)
                });
            }
        }
    } else {
        byte_stream
    };
    // Preflight must observe raw event order before completion validation can
    // collapse a chunk containing both output and a later error into one Err.
    let byte_stream: ByteStream = if needs_completion {
        Box::pin(responses::require_responses_completion(byte_stream))
    } else {
        byte_stream
    };
    Ok((byte_stream, attempt))
}
