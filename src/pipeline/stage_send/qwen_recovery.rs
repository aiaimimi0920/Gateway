//! Candidate qwen recovery ownership.
use super::*;

fn should_refresh_qwen_web_after_failure(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            crate::protocol::qwen_web::QWEN_WEB_BROWSER_CHALLENGE_REQUIRED_CODE
                | crate::protocol::qwen_web::QWEN_WEB_SESSION_INVALID_CODE
        )
    )
}

pub(super) async fn execute_qwen_web_nonstream_with_recovery(
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
    let first_attempt = provider_attempt_gate
        .execute_observed(
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
            provider_attempt_gate.check_remaining()?;
            let refreshed_payload = keepalive::refresh_qwen_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                model,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            global_gateway_metrics()
                .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
            let refreshed_metric_provider = provider_account_id.clone();
            let refreshed_metric_model = model.to_string();
            provider_attempt_gate
                .execute_observed(
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

pub(super) async fn execute_qwen_web_stream_with_recovery(
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    provider_attempt_gate: &super::stage_rate_limit::ProviderAttemptGate,
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: &std::collections::HashMap<String, String>,
) -> Result<(UpstreamStreamingResponse, Instant), GatewayError> {
    provider_attempt_gate.begin_attempt()?;
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
    provider_attempt_gate.observe_result(&first_attempt);
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
            provider_attempt_gate.check_remaining()?;
            let refreshed_payload = keepalive::refresh_qwen_web_payload_after_challenge(
                &state.redis_pool,
                state.pg_pool.as_ref(),
                payload,
                model,
            )
            .await?;
            provider_attempt_gate.admit().await?;
            global_gateway_metrics()
                .observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
            provider_attempt_gate.begin_attempt()?;
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
            provider_attempt_gate.observe_result(&refreshed_attempt);
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
