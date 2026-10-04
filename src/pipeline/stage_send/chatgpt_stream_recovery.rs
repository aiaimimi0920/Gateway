//! ChatGPT stream-start recovery; successful attempts transfer to the terminal stream owner.
use super::*;

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
    let cancellation = ProviderAttemptCancellation::new(
        global_gateway_metrics(),
        &candidate.provider_account_id,
        model,
        first_attempt_started_at,
    );
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
    cancellation.disarm();
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
            global_gateway_metrics()
                .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
            let refreshed_attempt_started_at = Instant::now();
            let cancellation = ProviderAttemptCancellation::new(
                global_gateway_metrics(),
                &candidate.provider_account_id,
                model,
                refreshed_attempt_started_at,
            );
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
            cancellation.disarm();
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
                    global_gateway_metrics()
                        .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
                    let relay_started_at = Instant::now();
                    let cancellation = ProviderAttemptCancellation::new(
                        global_gateway_metrics(),
                        &candidate.provider_account_id,
                        model,
                        relay_started_at,
                    );
                    let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                        &state.redis_pool,
                        state.pg_pool.as_ref(),
                        &refreshed_payload,
                        req,
                        model,
                        true,
                    )
                    .await;
                    cancellation.disarm();
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
                    global_gateway_metrics()
                        .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
                    let relay_started_at = Instant::now();
                    let cancellation = ProviderAttemptCancellation::new(
                        global_gateway_metrics(),
                        &candidate.provider_account_id,
                        model,
                        relay_started_at,
                    );
                    let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                        &state.redis_pool,
                        state.pg_pool.as_ref(),
                        &refreshed_payload,
                        req,
                        model,
                        true,
                    )
                    .await;
                    cancellation.disarm();
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
            global_gateway_metrics()
                .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
            let relay_started_at = Instant::now();
            let cancellation = ProviderAttemptCancellation::new(
                global_gateway_metrics(),
                &candidate.provider_account_id,
                model,
                relay_started_at,
            );
            let relay_result = keepalive::execute_chatgpt_web_browser_relay(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                req,
                model,
                true,
            )
            .await;
            cancellation.disarm();
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
