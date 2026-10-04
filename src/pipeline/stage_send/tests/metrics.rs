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
            is_retry: false,
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
            is_retry: true,
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
    assert!(output
        .contains("gateway_reliability_events_total{event=\"retry\",reason=\"same_candidate\"} 1"));
    metrics.observe_request(200, 128, false);
    assert_eq!(metrics.snapshot().requests_total, 1);
}

#[tokio::test]
async fn stream_metric_callback_preserves_ttft_and_terminal_reason_once() {
    use crate::upstream::stream::StreamMetrics;
    use std::sync::Mutex;
    let metrics = Arc::new(GatewayMetrics::new());
    for terminal in ["empty", "pending", "error", "cancel", "complete"] {
        let observed = Arc::clone(&metrics);
        let completed = Arc::new(Mutex::new(None::<StreamMetrics>));
        let snapshot = Arc::clone(&completed);
        let chunks: Vec<Result<Bytes, std::io::Error>> = if terminal == "empty" {
            vec![Ok(Bytes::new())]
        } else if terminal == "error" {
            vec![
                Ok(Bytes::from_static(b"first")),
                Err(std::io::Error::other("fixture")),
            ]
        } else {
            vec![Ok(Bytes::from_static(b"first"))]
        };
        let source: std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
        > = if terminal == "pending" {
            Box::pin(futures::stream::pending())
        } else {
            Box::pin(futures::stream::iter(chunks))
        };
        let mut stream = TrackedStream::new_with_started_at_and_error(
            source,
            Instant::now() - std::time::Duration::from_millis(40),
            move |stream, success| {
                observe_provider_stream_metric(&observed, terminal, "model", &stream, success);
                *snapshot.lock().unwrap() = Some(stream);
            },
        );
        if terminal == "pending" {
            assert!(futures::poll!(stream.next()).is_pending());
        } else {
            assert!(stream.next().await.unwrap().is_ok());
            if terminal != "cancel" {
                let _ = stream.next().await;
            }
        }
        drop(stream);
        let output = metrics.render_prometheus();
        assert!(output.contains(&format!(
            "gateway_provider_latency_ms_count{{provider=\"{terminal}\",model=\"model\"}} 1"
        )));
        let snapshot = completed.lock().unwrap();
        let snapshot = snapshot.as_ref().unwrap();
        assert!(snapshot.total_duration_ms >= 40);
        if matches!(terminal, "empty" | "pending") {
            assert_eq!(snapshot.first_token_latency_ms, None);
        } else {
            assert!(snapshot.first_token_latency_ms.unwrap() >= 40);
        }
        if terminal == "cancel" {
            assert!(output
                .contains("provider=\"cancel\",model=\"model\",failure_class=\"cancelled\"} 1"));
        }
        if terminal == "error" {
            assert!(output.contains(
                "provider=\"error\",model=\"model\",failure_class=\"stream_interrupted\"} 1"
            ));
        }
        assert!(output.contains(&format!(
            "gateway_provider_ttft_ms_count{{provider=\"{terminal}\",model=\"model\"}} {}",
            u64::from(!matches!(terminal, "empty" | "pending"))
        )));
    }
}

#[test]
fn direct_cancellation_guard_only_records_pending_attempts() {
    let metrics = GatewayMetrics::new();
    let guard = ProviderAttemptCancellation::new(&metrics, "provider", "model", Instant::now());
    guard.disarm();
    assert!(!metrics
        .render_prometheus()
        .contains("gateway_provider_requests_total{"));
    drop(ProviderAttemptCancellation::new(
        &metrics,
        "provider",
        "model",
        Instant::now(),
    ));
    let error = GatewayError::rate_limited("synthetic", 0);
    observe_fallback_metric(&metrics, &error);
    let output = metrics.render_prometheus();
    assert!(output.contains("failure_class=\"cancelled\"} 1"));
    assert!(output.contains(
        "gateway_reliability_events_total{event=\"fallback\",reason=\"rate_limited\"} 1"
    ));
}
