use neuro_gateway::metrics::request::{GatewayMetrics, ProviderMetricOutcome};

#[test]
fn gateway_metrics_render_stable_request_and_provider_series() {
    let metrics = GatewayMetrics::new();
    metrics.begin_request();
    metrics.observe_request(200, 17, false);
    metrics.observe_request(503, 240, false);
    metrics.observe_request(503, 11, true);
    metrics.end_request();
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider: "openai",
        model: Some("gpt-4o"),
        success: false,
        latency_ms: 240,
        failure_class: Some("provider_transient"),
    });

    let output = metrics.render_prometheus();
    assert!(output.contains("gateway_requests_total 3"));
    assert!(output.contains("gateway_request_errors_total 2"));
    assert!(output.contains("gateway_request_drain_rejections_total 1"));
    assert!(output.contains("gateway_request_in_flight 0"));
    assert!(
        output.contains("gateway_provider_requests_total{provider=\"openai\",model=\"gpt-4o\"} 1")
    );
    assert!(output.contains("gateway_provider_errors_total{provider=\"openai\",model=\"gpt-4o\",failure_class=\"provider_transient\"} 1"));
    assert!(output.contains("gateway_request_duration_ms_count 3"));
}

#[test]
fn gateway_metrics_bound_label_values() {
    let metrics = GatewayMetrics::new();
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider: "provider with spaces/and\nsecrets",
        model: Some("model\"with\"quotes"),
        success: true,
        latency_ms: 1,
        failure_class: None,
    });

    let output = metrics.render_prometheus();
    assert!(!output.contains("provider with spaces"));
    assert!(!output.contains("model\"with\"quotes"));
    assert!(output.contains("gateway_provider_requests_total"));
}

#[test]
fn gateway_metrics_bound_provider_series_cardinality() {
    let metrics = GatewayMetrics::new();
    for index in 0..400 {
        let provider = format!("provider-{index}");
        metrics.observe_provider_outcome(ProviderMetricOutcome {
            provider: &provider,
            model: Some("model"),
            success: true,
            latency_ms: 1,
            failure_class: None,
        });
    }

    let output = metrics.render_prometheus();
    let provider_series = output
        .lines()
        .filter(|line| line.starts_with("gateway_provider_requests_total{"))
        .count();
    assert!(provider_series <= 257);
    assert!(output.contains("provider=\"overflow\""));
}
