use super::*;

#[test]
fn provider_metric_helpers_record_success_and_classified_failure() {
    let metrics = crate::metrics::request::GatewayMetrics::new();
    observe_provider_success_metric(&metrics, "openai", "gpt-4o", 25);
    observe_provider_failure_metric(
        &metrics,
        "openai",
        "gpt-4o",
        120,
        "rate limit exceeded with too many requests",
    );

    let output = metrics.render_prometheus();
    assert!(
        output.contains("gateway_provider_requests_total{provider=\"openai\",model=\"gpt-4o\"} 2")
    );
    assert!(output.contains("failure_class=\"rate_limited\""));
}

#[test]
fn provider_attempt_metric_observer_uses_resolved_model_and_elapsed_attempt_latency() {
    let metrics = crate::metrics::request::GatewayMetrics::new();
    observe_provider_attempt_metric(
        &metrics,
        "provider-account-1",
        "gpt-5.3-codex",
        RetryAttemptObservation {
            succeeded: true,
            latency_ms: 37,
            error: None,
        },
    );
    let error = GatewayError::server_error("upstream unavailable");
    observe_provider_attempt_metric(
        &metrics,
        "provider-account-1",
        "gpt-5.3-codex",
        RetryAttemptObservation {
            succeeded: false,
            latency_ms: 91,
            error: Some(&error),
        },
    );

    let output = metrics.render_prometheus();
    assert!(output.contains(
            "gateway_provider_requests_total{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 2"
        ));
    assert!(output.contains(
            "gateway_provider_latency_ms_sum{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 128"
        ));
    assert!(!output.contains("unknown_model"));
    assert!(!output
        .contains("latency_ms_sum{provider=\"provider-account-1\",model=\"gpt-5.3-codex\"} 0"));
}
