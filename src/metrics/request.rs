use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::diagnostics::{bounded_reason, ReliabilityEvent, RequestTermination};
use super::histogram::LatencyHistogram;

const MAX_PROVIDER_SERIES: usize = 256;

/// A bounded, process-local metric registry for request and provider signals.
///
/// The registry deliberately keeps only low-cardinality provider/model/failure
/// labels. It is used by the HTTP middleware and can also be used by pipeline
/// adapters without adding another runtime dependency or changing `AppState`.
#[derive(Debug, Default)]
pub struct GatewayMetrics {
    request: Mutex<RequestMetricCounters>,
    request_in_flight: AtomicU64,
    rate_limit_checks_total: AtomicU64,
    rate_limit_rejections_total: AtomicU64,
    rate_limit_store_failures_total: AtomicU64,
    provider: Mutex<BTreeMap<ProviderMetricKey, ProviderMetricCounters>>,
    diagnostics: Mutex<BTreeMap<(&'static str, &'static str), u64>>,
}

#[derive(Debug, Default, Clone)]
struct RequestMetricCounters {
    errors: u64,
    drain_rejections: u64,
    duration: LatencyHistogram,
    terminations: [u64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderMetricKey {
    provider: String,
    model: String,
}

#[derive(Debug, Default, Clone)]
struct ProviderMetricCounters {
    requests: u64,
    errors: u64,
    latency: LatencyHistogram,
    ttft: LatencyHistogram,
    failures: BTreeMap<&'static str, u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderMetricOutcome<'a> {
    pub provider: &'a str,
    pub model: Option<&'a str>,
    pub success: bool,
    pub latency_ms: u64,
    pub failure_class: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct GatewayMetricsSnapshot {
    pub requests_total: u64,
    pub request_errors_total: u64,
    pub request_drain_rejections_total: u64,
    pub request_in_flight: u64,
    pub request_duration_ms_count: u64,
    pub request_duration_ms_sum: u64,
    pub rate_limit_checks_total: u64,
    pub rate_limit_rejections_total: u64,
    pub rate_limit_store_failures_total: u64,
}

impl GatewayMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_request(&self) {
        self.request_in_flight.fetch_add(1, Ordering::Relaxed);
    }

    pub fn end_request(&self) {
        let _ =
            self.request_in_flight
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    Some(value.saturating_sub(1))
                });
    }

    pub fn observe_request(&self, status: u16, latency_ms: u64, drain_rejected: bool) {
        self.observe_request_terminal(
            status,
            latency_ms,
            drain_rejected,
            RequestTermination::Completed,
        );
    }

    pub fn observe_request_terminal(
        &self,
        status: u16,
        latency_ms: u64,
        drain_rejected: bool,
        terminal: RequestTermination,
    ) {
        let mut counters = self.request.lock().expect("gateway metrics mutex poisoned");
        counters.duration.observe(latency_ms);
        if status >= 400 || !matches!(terminal, RequestTermination::Completed) {
            counters.errors = counters.errors.saturating_add(1);
        }
        if drain_rejected {
            counters.drain_rejections = counters.drain_rejections.saturating_add(1);
        }
        let count = &mut counters.terminations[terminal.index()];
        *count = count.saturating_add(1);
    }

    pub fn observe_provider_outcome(&self, outcome: ProviderMetricOutcome<'_>) {
        self.observe_provider_outcome_with_ttft(outcome, None);
    }

    /// TTFT is the first non-empty translated chunk, not a semantic text token.
    /// Missing TTFT (empty/error/cancelled before a chunk) contributes no sample.
    pub fn observe_provider_outcome_with_ttft(
        &self,
        outcome: ProviderMetricOutcome<'_>,
        ttft_ms: Option<u64>,
    ) {
        let provider = bounded_label(outcome.provider, "unknown_provider");
        let model = bounded_label(outcome.model.unwrap_or("unknown_model"), "unknown_model");
        let requested_key = ProviderMetricKey { provider, model };
        let mut guard = self
            .provider
            .lock()
            .expect("gateway metrics mutex poisoned");
        let key = if guard.contains_key(&requested_key) || guard.len() < MAX_PROVIDER_SERIES {
            requested_key
        } else {
            ProviderMetricKey {
                provider: "overflow".to_string(),
                model: "overflow".to_string(),
            }
        };
        let counters = guard.entry(key).or_default();
        counters.requests = counters.requests.saturating_add(1);
        counters.latency.observe(outcome.latency_ms);
        if let Some(ttft_ms) = ttft_ms {
            counters.ttft.observe(ttft_ms);
        }
        if !outcome.success {
            counters.errors = counters.errors.saturating_add(1);
            let failure_class = match bounded_reason(outcome.failure_class) {
                "same_candidate" | "recovery" => "unknown",
                reason => reason,
            };
            let entry = counters.failures.entry(failure_class).or_default();
            *entry = entry.saturating_add(1);
        }
    }

    pub fn observe_reliability_event(&self, event: ReliabilityEvent, reason: Option<&str>) {
        let mut guard = self
            .diagnostics
            .lock()
            .expect("gateway metrics mutex poisoned");
        let count = guard
            .entry((event.as_str(), bounded_reason(reason)))
            .or_default();
        *count = count.saturating_add(1);
    }

    pub fn observe_rate_limit_admission(&self, rejected: bool, store_failure: bool) {
        self.rate_limit_checks_total.fetch_add(1, Ordering::Relaxed);
        if rejected {
            self.rate_limit_rejections_total
                .fetch_add(1, Ordering::Relaxed);
        }
        if store_failure {
            self.rate_limit_store_failures_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn snapshot(&self) -> GatewayMetricsSnapshot {
        let request = self.request.lock().expect("gateway metrics mutex poisoned");
        self.snapshot_with_request(&request)
    }

    fn snapshot_with_request(&self, request: &RequestMetricCounters) -> GatewayMetricsSnapshot {
        GatewayMetricsSnapshot {
            requests_total: request.duration.count,
            request_errors_total: request.errors,
            request_drain_rejections_total: request.drain_rejections,
            request_in_flight: self.request_in_flight.load(Ordering::Relaxed),
            request_duration_ms_count: request.duration.count,
            request_duration_ms_sum: request.duration.sum,
            rate_limit_checks_total: self.rate_limit_checks_total.load(Ordering::Relaxed),
            rate_limit_rejections_total: self.rate_limit_rejections_total.load(Ordering::Relaxed),
            rate_limit_store_failures_total: self
                .rate_limit_store_failures_total
                .load(Ordering::Relaxed),
        }
    }

    pub fn render_prometheus(&self) -> String {
        // Copy under the owner lock, then format outside it: scrapes cannot mix
        // bucket/count/sum generations or block hot-path writers while formatting.
        let request = self
            .request
            .lock()
            .expect("gateway metrics mutex poisoned")
            .clone();
        let snapshot = self.snapshot_with_request(&request);
        let mut output = String::new();
        let _ = writeln!(output, "gateway_requests_total {}", snapshot.requests_total);
        let _ = writeln!(
            output,
            "gateway_request_errors_total {}",
            snapshot.request_errors_total
        );
        let _ = writeln!(
            output,
            "gateway_request_drain_rejections_total {}",
            snapshot.request_drain_rejections_total
        );
        let _ = writeln!(
            output,
            "gateway_request_in_flight {}",
            snapshot.request_in_flight
        );
        let _ = writeln!(output, "# TYPE gateway_request_duration_ms histogram");
        request
            .duration
            .render(&mut output, "gateway_request_duration_ms", "");
        for terminal in [
            RequestTermination::Completed,
            RequestTermination::Cancelled,
            RequestTermination::Interrupted,
        ] {
            let _ = writeln!(
                output,
                "gateway_request_terminations_total{{reason=\"{}\"}} {}",
                terminal.as_str(),
                request.terminations[terminal.index()]
            );
        }
        let _ = writeln!(
            output,
            "gateway_rate_limit_checks_total {}",
            snapshot.rate_limit_checks_total
        );
        let _ = writeln!(
            output,
            "gateway_rate_limit_rejections_total {}",
            snapshot.rate_limit_rejections_total
        );
        let _ = writeln!(
            output,
            "gateway_rate_limit_store_failures_total {}",
            snapshot.rate_limit_store_failures_total
        );

        let guard = self
            .provider
            .lock()
            .expect("gateway metrics mutex poisoned")
            .clone();
        let _ = writeln!(output, "# TYPE gateway_provider_latency_ms histogram");
        let _ = writeln!(output, "# TYPE gateway_provider_ttft_ms histogram");
        for (key, counters) in guard.iter() {
            let _ = writeln!(
                output,
                "gateway_provider_requests_total{{provider=\"{}\",model=\"{}\"}} {}",
                key.provider, key.model, counters.requests
            );
            let _ = writeln!(
                output,
                "gateway_provider_errors_total{{provider=\"{}\",model=\"{}\"}} {}",
                key.provider, key.model, counters.errors
            );
            let labels = format!("provider=\"{}\",model=\"{}\"", key.provider, key.model);
            counters
                .latency
                .render(&mut output, "gateway_provider_latency_ms", &labels);
            counters
                .ttft
                .render(&mut output, "gateway_provider_ttft_ms", &labels);
            for (failure_class, count) in &counters.failures {
                let _ = writeln!(
                    output,
                    "gateway_provider_errors_total{{provider=\"{}\",model=\"{}\",failure_class=\"{}\"}} {}",
                    key.provider, key.model, failure_class, count
                );
            }
        }

        let diagnostics = self
            .diagnostics
            .lock()
            .expect("gateway metrics mutex poisoned")
            .clone();
        for ((event, reason), count) in diagnostics {
            let _ = writeln!(
                output,
                "gateway_reliability_events_total{{event=\"{event}\",reason=\"{reason}\"}} {count}"
            );
        }

        output
    }
}

pub(super) fn bounded_label(value: &str, fallback: &str) -> String {
    let mut label = String::with_capacity(value.len().min(64));
    for ch in value.trim().chars() {
        if label.len() >= 64 {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            label.push(ch);
        } else {
            label.push('_');
        }
    }
    if label.is_empty() {
        fallback.to_string()
    } else {
        label
    }
}

static GLOBAL_METRICS: OnceLock<Arc<GatewayMetrics>> = OnceLock::new();

pub fn global_gateway_metrics() -> &'static Arc<GatewayMetrics> {
    GLOBAL_METRICS.get_or_init(|| Arc::new(GatewayMetrics::new()))
}
