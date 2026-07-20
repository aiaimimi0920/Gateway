use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_PROVIDER_SERIES: usize = 256;

/// A bounded, process-local metric registry for request and provider signals.
///
/// The registry deliberately keeps only low-cardinality provider/model/failure
/// labels. It is used by the HTTP middleware and can also be used by pipeline
/// adapters without adding another runtime dependency or changing `AppState`.
#[derive(Debug, Default)]
pub struct GatewayMetrics {
    requests_total: AtomicU64,
    request_errors_total: AtomicU64,
    request_drain_rejections_total: AtomicU64,
    request_in_flight: AtomicU64,
    request_duration_ms_count: AtomicU64,
    request_duration_ms_sum: AtomicU64,
    rate_limit_checks_total: AtomicU64,
    rate_limit_rejections_total: AtomicU64,
    rate_limit_store_failures_total: AtomicU64,
    provider: Mutex<BTreeMap<ProviderMetricKey, ProviderMetricCounters>>,
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
    latency_ms_sum: u64,
    latency_ms_count: u64,
    failures: BTreeMap<String, u64>,
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
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.request_duration_ms_count
            .fetch_add(1, Ordering::Relaxed);
        self.request_duration_ms_sum
            .fetch_add(latency_ms, Ordering::Relaxed);
        if status >= 400 {
            self.request_errors_total.fetch_add(1, Ordering::Relaxed);
        }
        if drain_rejected {
            self.request_drain_rejections_total
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn observe_provider_outcome(&self, outcome: ProviderMetricOutcome<'_>) {
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
        counters.latency_ms_sum = counters.latency_ms_sum.saturating_add(outcome.latency_ms);
        counters.latency_ms_count = counters.latency_ms_count.saturating_add(1);
        if !outcome.success {
            counters.errors = counters.errors.saturating_add(1);
            let failure_class =
                bounded_label(outcome.failure_class.unwrap_or("unknown"), "unknown");
            let entry = counters.failures.entry(failure_class).or_default();
            *entry = entry.saturating_add(1);
        }
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
        GatewayMetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            request_errors_total: self.request_errors_total.load(Ordering::Relaxed),
            request_drain_rejections_total: self
                .request_drain_rejections_total
                .load(Ordering::Relaxed),
            request_in_flight: self.request_in_flight.load(Ordering::Relaxed),
            request_duration_ms_count: self.request_duration_ms_count.load(Ordering::Relaxed),
            request_duration_ms_sum: self.request_duration_ms_sum.load(Ordering::Relaxed),
            rate_limit_checks_total: self.rate_limit_checks_total.load(Ordering::Relaxed),
            rate_limit_rejections_total: self.rate_limit_rejections_total.load(Ordering::Relaxed),
            rate_limit_store_failures_total: self
                .rate_limit_store_failures_total
                .load(Ordering::Relaxed),
        }
    }

    pub fn render_prometheus(&self) -> String {
        let snapshot = self.snapshot();
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
        let _ = writeln!(
            output,
            "gateway_request_duration_ms_count {}",
            snapshot.request_duration_ms_count
        );
        let _ = writeln!(
            output,
            "gateway_request_duration_ms_sum {}",
            snapshot.request_duration_ms_sum
        );
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
            .expect("gateway metrics mutex poisoned");
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
            let _ = writeln!(
                output,
                "gateway_provider_latency_ms_sum{{provider=\"{}\",model=\"{}\"}} {}",
                key.provider, key.model, counters.latency_ms_sum
            );
            let _ = writeln!(
                output,
                "gateway_provider_latency_ms_count{{provider=\"{}\",model=\"{}\"}} {}",
                key.provider, key.model, counters.latency_ms_count
            );
            for (failure_class, count) in &counters.failures {
                let _ = writeln!(
                    output,
                    "gateway_provider_errors_total{{provider=\"{}\",model=\"{}\",failure_class=\"{}\"}} {}",
                    key.provider, key.model, failure_class, count
                );
            }
        }

        output
    }
}

fn bounded_label(value: &str, fallback: &str) -> String {
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
