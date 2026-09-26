//! Candidate chatgpt recovery ownership.
use super::*;

pub(super) async fn execute_chatgpt_web_nonstream_with_recovery(
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

pub(super) async fn execute_chatgpt_web_stream_with_recovery(
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
