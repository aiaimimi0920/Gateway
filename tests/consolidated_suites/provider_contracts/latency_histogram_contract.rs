use neuro_gateway::metrics::request::{GatewayMetrics, ProviderMetricOutcome};
use std::sync::Arc;

const BOUNDS: [u64; 15] = [
    5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000, 30000, 60000, 120000, 300000,
];

fn sample(output: &str, name: &str) -> u64 {
    output
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")))
        .unwrap_or_else(|| panic!("missing metric {name}"))
        .parse()
        .unwrap()
}

fn provider_observation(metrics: &GatewayMetrics, provider: &str, latency_ms: u64, reason: &str) {
    metrics.observe_provider_outcome(ProviderMetricOutcome {
        provider,
        model: Some("model"),
        success: reason.is_empty(),
        latency_ms,
        failure_class: (!reason.is_empty()).then_some(reason),
    });
}

#[test]
fn fixed_buckets_are_inclusive_cumulative_and_preserve_sum_count() {
    let metrics = GatewayMetrics::new();
    let mut values = vec![0, 300001];
    for bound in BOUNDS {
        values.extend([bound - 1, bound, bound + 1]);
    }
    for value in &values {
        metrics.observe_request(200, *value, false);
        provider_observation(&metrics, "provider", *value, "");
    }
    let output = metrics.render_prometheus();
    assert!(output.contains("# TYPE gateway_request_duration_ms histogram"));
    assert!(output.contains("# TYPE gateway_provider_latency_ms histogram"));
    for bound in BOUNDS {
        let count = values.iter().filter(|value| **value <= bound).count() as u64;
        assert_eq!(
            sample(
                &output,
                &format!("gateway_request_duration_ms_bucket{{le=\"{bound}\"}}")
            ),
            count
        );
        assert_eq!(sample(&output, &format!("gateway_provider_latency_ms_bucket{{provider=\"provider\",model=\"model\",le=\"{bound}\"}}")), count);
    }
    let count = values.len() as u64;
    let sum = values.iter().sum::<u64>();
    assert_eq!(
        sample(&output, "gateway_request_duration_ms_bucket{le=\"+Inf\"}"),
        count
    );
    assert_eq!(sample(&output, "gateway_request_duration_ms_count"), count);
    assert_eq!(sample(&output, "gateway_request_duration_ms_sum"), sum);
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_bucket{provider=\"provider\",model=\"model\",le=\"+Inf\"}"
        ),
        count
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_count{provider=\"provider\",model=\"model\"}"
        ),
        count
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_sum{provider=\"provider\",model=\"model\"}"
        ),
        sum
    );
}

#[test]
fn arbitrary_failure_reasons_cannot_expand_labels_or_export_secrets() {
    let metrics = GatewayMetrics::new();
    for index in 0..1000 {
        provider_observation(&metrics, "provider", 1, &format!("prompt-secret-{index}"));
    }
    let output = metrics.render_prometheus();
    assert!(!output.contains("prompt-secret"));
    assert_eq!(sample(&output, "gateway_provider_errors_total{provider=\"provider\",model=\"model\",failure_class=\"unknown\"}"), 1000);
    assert_eq!(
        output
            .lines()
            .filter(|line| line.contains("failure_class="))
            .count(),
        1
    );
}

#[test]
fn overflow_aggregates_histograms_without_evicting_existing_series() {
    let metrics = GatewayMetrics::new();
    for index in 0..400 {
        provider_observation(&metrics, &format!("provider-{index}"), 17, "");
    }
    provider_observation(&metrics, "provider-0", 11, "");
    let output = metrics.render_prometheus();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("gateway_provider_requests_total{"))
            .count(),
        257
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_count{provider=\"overflow\",model=\"overflow\"}"
        ),
        144
    );
    assert_eq!(sample(&output, "gateway_provider_latency_ms_bucket{provider=\"overflow\",model=\"overflow\",le=\"25\"}"), 144);
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_sum{provider=\"provider-0\",model=\"model\"}"
        ),
        28
    );
}

#[test]
fn concurrent_scrapes_keep_each_histogram_snapshot_consistent() {
    let metrics = Arc::new(GatewayMetrics::new());
    let writers: Vec<_> = (0..8)
        .map(|_| {
            let metrics = Arc::clone(&metrics);
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    metrics.observe_request(200, 17, false);
                    provider_observation(&metrics, "provider", 17, "");
                }
            })
        })
        .collect();
    for _ in 0..100 {
        let output = metrics.render_prometheus();
        let count = sample(&output, "gateway_request_duration_ms_count");
        assert_eq!(sample(&output, "gateway_requests_total"), count);
        assert_eq!(
            sample(&output, "gateway_request_duration_ms_bucket{le=\"+Inf\"}"),
            count
        );
        assert_eq!(
            sample(&output, "gateway_request_duration_ms_bucket{le=\"25\"}"),
            count
        );
        assert_eq!(
            sample(&output, "gateway_request_duration_ms_sum"),
            count * 17
        );
        if output.contains("gateway_provider_requests_total{") {
            let count = sample(
                &output,
                "gateway_provider_latency_ms_count{provider=\"provider\",model=\"model\"}",
            );
            assert_eq!(sample(&output, "gateway_provider_latency_ms_bucket{provider=\"provider\",model=\"model\",le=\"+Inf\"}"), count);
            assert_eq!(
                sample(
                    &output,
                    "gateway_provider_latency_ms_sum{provider=\"provider\",model=\"model\"}"
                ),
                count * 17
            );
        }
    }
    for writer in writers {
        writer.join().unwrap();
    }
    assert_eq!(metrics.snapshot().requests_total, 8000);
}

#[test]
fn ttft_none_is_not_a_zero_sample_and_uses_the_same_fixed_buckets() {
    let metrics = GatewayMetrics::new();
    for ttft in [None, Some(0), Some(25), Some(26), None] {
        metrics.observe_provider_outcome_with_ttft(
            ProviderMetricOutcome {
                provider: "provider",
                model: Some("model"),
                success: true,
                latency_ms: 100,
                failure_class: None,
            },
            ttft,
        );
    }
    let output = metrics.render_prometheus();
    assert_eq!(
        sample(
            &output,
            "gateway_provider_latency_ms_count{provider=\"provider\",model=\"model\"}"
        ),
        5
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_ttft_ms_count{provider=\"provider\",model=\"model\"}"
        ),
        3
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_ttft_ms_sum{provider=\"provider\",model=\"model\"}"
        ),
        51
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_ttft_ms_bucket{provider=\"provider\",model=\"model\",le=\"25\"}"
        ),
        2
    );
    assert_eq!(
        sample(
            &output,
            "gateway_provider_ttft_ms_bucket{provider=\"provider\",model=\"model\",le=\"+Inf\"}"
        ),
        3
    );
}

#[test]
fn reliability_event_labels_are_a_closed_domain() {
    use neuro_gateway::metrics::diagnostics::{ReliabilityEvent, RequestTermination};
    let metrics = GatewayMetrics::new();
    for index in 0..1000 {
        metrics.observe_reliability_event(
            ReliabilityEvent::Fallback,
            Some(&format!("https://secret/{index}")),
        );
    }
    metrics.observe_reliability_event(ReliabilityEvent::Retry, Some("recovery"));
    metrics.observe_request_terminal(200, 17, false, RequestTermination::Cancelled);
    let output = metrics.render_prometheus();
    assert!(!output.contains("https"));
    assert_eq!(
        sample(
            &output,
            "gateway_reliability_events_total{event=\"fallback\",reason=\"unknown\"}"
        ),
        1000
    );
    assert_eq!(
        sample(
            &output,
            "gateway_reliability_events_total{event=\"retry\",reason=\"recovery\"}"
        ),
        1
    );
    assert_eq!(sample(&output, "gateway_request_errors_total"), 1);
}
