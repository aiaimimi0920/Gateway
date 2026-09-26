//! Candidate feedback ownership.
use super::*;

/// Determine the AIMD failure kind from the error.
pub(super) fn classify_failure_kind(e: &GatewayError) -> FailureKind {
    if e.kind == crate::error::ErrorKind::RateLimit {
        FailureKind::RateLimited
    } else {
        FailureKind::General
    }
}

/// Return `true` when the error hint suggests we should try the next candidate.
pub(super) fn should_try_next_candidate(e: &GatewayError) -> bool {
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

pub(super) fn should_record_provider_failure(error: &GatewayError) -> bool {
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

pub(super) fn retry_policy_for_request(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> RetryPolicy {
    let mut policy = RetryPolicy::default();
    if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
        && matches!(req.endpoint_kind, EndpointKind::ChatCompletions)
    {
        // Program text execution already performs its own handle recovery and
        // can consume the full 300-second operation timeout. Replaying that
        // entire workflow at the provider layer can keep one request alive for
        // several additional timeout windows.
        policy.max_retries = 0;
    }
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

pub(super) fn spawn_record_provider_success(
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

pub(super) fn spawn_record_provider_failure(
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

pub(super) fn observe_provider_attempt_metric(
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

pub(super) fn observe_provider_result_metric<T>(
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

pub(super) fn observe_provider_success_metric(
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

pub(super) fn observe_provider_failure_metric(
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
