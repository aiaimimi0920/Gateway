// ---------------------------------------------------------------------------
// SlidingWindowMetrics — fixed-capacity circular buffer of per-request entries
//
// Ported from TypeScript sliding-metrics.ts (NeuroLoom platform).
// All operations are protected by a parking_lot::Mutex so the struct is safe
// to share across async tasks.
// ---------------------------------------------------------------------------

use std::collections::VecDeque;

use parking_lot::Mutex;

// ---------------------------------------------------------------------------
// MetricEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MetricEntry {
    /// Wall-clock milliseconds (unix epoch) at which the request completed.
    pub timestamp_ms: u64,
    /// Total request duration in milliseconds.
    pub latency_ms: u64,
    /// Whether the upstream request was considered successful.
    pub success: bool,
    /// Time-to-first-token in milliseconds (streaming only; `None` for
    /// non-streaming or when unavailable).
    pub first_token_latency_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// MetricsSummary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub total_requests: usize,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    /// `None` when no streaming entries are present in the window.
    pub avg_first_token_latency_ms: Option<f64>,
}

// ---------------------------------------------------------------------------
// SlidingWindowMetrics
// ---------------------------------------------------------------------------

/// Thread-safe, fixed-size sliding window that stores the most recent
/// `max_size` request observations and exposes statistical summaries.
pub struct SlidingWindowMetrics {
    entries: Mutex<VecDeque<MetricEntry>>,
    max_size: usize,
}

impl SlidingWindowMetrics {
    /// Create a new window that retains at most `max_size` entries.
    ///
    /// Typical default: `200`.
    pub fn new(max_size: usize) -> Self {
        assert!(max_size > 0, "max_size must be at least 1");
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
        }
    }

    // ── write ────────────────────────────────────────────────────────────

    /// Record a completed request.
    ///
    /// `ttft_ms` — time-to-first-token for streaming requests; pass `None`
    /// for non-streaming calls or when the measurement is unavailable.
    pub fn record(&self, latency_ms: u64, success: bool, ttft_ms: Option<u64>) {
        let timestamp_ms = current_timestamp_ms();
        let entry = MetricEntry {
            timestamp_ms,
            latency_ms,
            success,
            first_token_latency_ms: ttft_ms,
        };

        let mut entries = self.entries.lock();
        if entries.len() == self.max_size {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    // ── read ─────────────────────────────────────────────────────────────

    /// Compute statistics over the last `window_size` entries (or all
    /// entries when `window_size` is `None` or larger than the buffer).
    pub fn summary(&self, window_size: Option<usize>) -> MetricsSummary {
        let entries = self.entries.lock();
        let n = entries.len();

        if n == 0 {
            return MetricsSummary {
                total_requests: 0,
                success_rate: 0.0,
                avg_latency_ms: 0.0,
                p50_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                p99_latency_ms: 0.0,
                avg_first_token_latency_ms: None,
            };
        }

        let take = window_size.map(|w| w.min(n)).unwrap_or(n);
        let window: Vec<&MetricEntry> = entries.iter().rev().take(take).collect();

        let total = window.len();
        let successes = window.iter().filter(|e| e.success).count();
        let success_rate = successes as f64 / total as f64;

        let mut latencies: Vec<f64> = window.iter().map(|e| e.latency_ms as f64).collect();
        latencies.sort_by(f64::total_cmp);

        let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50 = percentile(&latencies, 50.0);
        let p95 = percentile(&latencies, 95.0);
        let p99 = percentile(&latencies, 99.0);

        let ttft_values: Vec<f64> = window
            .iter()
            .filter_map(|e| e.first_token_latency_ms.map(|v| v as f64))
            .collect();

        let avg_ttft = if ttft_values.is_empty() {
            None
        } else {
            Some(ttft_values.iter().sum::<f64>() / ttft_values.len() as f64)
        };

        MetricsSummary {
            total_requests: total,
            success_rate,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            avg_first_token_latency_ms: avg_ttft,
        }
    }

    /// Success rate over the last `n` entries. Returns `0.0` when there are
    /// no entries.
    pub fn recent_success_rate(&self, n: usize) -> f64 {
        let entries = self.entries.lock();
        if entries.is_empty() {
            return 0.0;
        }
        let take = n.min(entries.len());
        let window: Vec<&MetricEntry> = entries.iter().rev().take(take).collect();
        let successes = window.iter().filter(|e| e.success).count();
        successes as f64 / window.len() as f64
    }

    /// Average latency over the last `n` entries. Returns `0.0` when there
    /// are no entries.
    pub fn recent_avg_latency(&self, n: usize) -> f64 {
        let entries = self.entries.lock();
        if entries.is_empty() {
            return 0.0;
        }
        let take = n.min(entries.len());
        let window: Vec<&MetricEntry> = entries.iter().rev().take(take).collect();
        let sum: f64 = window.iter().map(|e| e.latency_ms as f64).sum();
        sum / window.len() as f64
    }

    /// Total number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    /// Returns `true` when no entries have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the `p`-th percentile of a **sorted** slice using linear
/// interpolation (identical to NumPy's default method).
///
/// Panics if `sorted` is empty or `p` is not in `[0, 100]`.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty(), "percentile requires at least one value");
    debug_assert!((0.0..=100.0).contains(&p), "p must be in [0, 100]");

    if sorted.len() == 1 {
        return sorted[0];
    }

    let virtual_index = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = virtual_index.floor() as usize;
    let hi = virtual_index.ceil() as usize;
    let fraction = virtual_index - lo as f64;

    sorted[lo] + (sorted[hi] - sorted[lo]) * fraction
}

/// Milliseconds since the Unix epoch. Falls back to `0` on platforms without
/// system time support (should not occur in practice).
fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(max_size: usize) -> SlidingWindowMetrics {
        SlidingWindowMetrics::new(max_size)
    }

    // ── percentile helper ────────────────────────────────────────────────

    #[test]
    fn percentile_single_value() {
        assert_eq!(percentile(&[42.0], 50.0), 42.0);
        assert_eq!(percentile(&[42.0], 99.0), 42.0);
    }

    #[test]
    fn percentile_two_values_midpoint() {
        // p50 of [0, 100] → 50.0 (linear interpolation)
        let v = vec![0.0, 100.0];
        assert!((percentile(&v, 50.0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn percentile_p0_and_p100() {
        let v = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(percentile(&v, 0.0), 10.0);
        assert_eq!(percentile(&v, 100.0), 50.0);
    }

    #[test]
    fn percentile_known_values() {
        // sorted: [1, 2, 3, 4, 5]
        // p50 virtual_index = 0.5 * 4 = 2.0 → sorted[2] = 3
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&v, 50.0) - 3.0).abs() < 1e-9);
        // p25 virtual_index = 0.25 * 4 = 1.0 → sorted[1] = 2
        assert!((percentile(&v, 25.0) - 2.0).abs() < 1e-9);
    }

    // ── empty window ─────────────────────────────────────────────────────

    #[test]
    fn empty_summary_returns_zeros() {
        let m = make_metrics(10);
        let s = m.summary(None);
        assert_eq!(s.total_requests, 0);
        assert_eq!(s.success_rate, 0.0);
        assert_eq!(s.avg_latency_ms, 0.0);
        assert!(s.avg_first_token_latency_ms.is_none());
    }

    #[test]
    fn empty_recent_success_rate_returns_zero() {
        let m = make_metrics(10);
        assert_eq!(m.recent_success_rate(5), 0.0);
    }

    #[test]
    fn empty_recent_avg_latency_returns_zero() {
        let m = make_metrics(10);
        assert_eq!(m.recent_avg_latency(5), 0.0);
    }

    // ── basic recording ──────────────────────────────────────────────────

    #[test]
    fn record_increments_len() {
        let m = make_metrics(10);
        assert_eq!(m.len(), 0);
        m.record(100, true, None);
        assert_eq!(m.len(), 1);
        m.record(200, false, Some(50));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn eviction_when_max_size_reached() {
        let m = make_metrics(3);
        m.record(1, true, None);
        m.record(2, true, None);
        m.record(3, true, None);
        assert_eq!(m.len(), 3);
        m.record(4, true, None);
        // Still capped at 3.
        assert_eq!(m.len(), 3);
    }

    // ── success_rate ─────────────────────────────────────────────────────

    #[test]
    fn success_rate_all_success() {
        let m = make_metrics(10);
        for _ in 0..5 {
            m.record(100, true, None);
        }
        let s = m.summary(None);
        assert!((s.success_rate - 1.0).abs() < 1e-9);
    }

    #[test]
    fn success_rate_half_success() {
        let m = make_metrics(10);
        m.record(100, true, None);
        m.record(100, true, None);
        m.record(100, false, None);
        m.record(100, false, None);
        let s = m.summary(None);
        assert!((s.success_rate - 0.5).abs() < 1e-9);
    }

    // ── latency stats ────────────────────────────────────────────────────

    #[test]
    fn avg_latency_is_correct() {
        let m = make_metrics(10);
        m.record(100, true, None);
        m.record(200, true, None);
        m.record(300, true, None);
        let s = m.summary(None);
        assert!((s.avg_latency_ms - 200.0).abs() < 1e-9);
    }

    #[test]
    fn p50_p95_p99_with_10_entries() {
        let m = make_metrics(20);
        // Insert 10 entries with latencies 10..100 ms in steps of 10.
        for i in 1..=10 {
            m.record(i * 10, true, None);
        }
        let s = m.summary(None);
        // sorted: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        // p50 virtual_index = 0.5 * 9 = 4.5 → 50 + 0.5*(60-50) = 55
        assert!(
            (s.p50_latency_ms - 55.0).abs() < 1e-6,
            "p50 was {}",
            s.p50_latency_ms
        );
        // p95 virtual_index = 0.95 * 9 = 8.55 → 90 + 0.55*(100-90) = 95.5
        assert!(
            (s.p95_latency_ms - 95.5).abs() < 1e-6,
            "p95 was {}",
            s.p95_latency_ms
        );
        // p99 virtual_index = 0.99 * 9 = 8.91 → 90 + 0.91*(100-90) = 99.1
        assert!(
            (s.p99_latency_ms - 99.1).abs() < 1e-6,
            "p99 was {}",
            s.p99_latency_ms
        );
    }

    // ── window_size parameter ────────────────────────────────────────────

    #[test]
    fn summary_respects_window_size() {
        let m = make_metrics(20);
        // 5 slow entries then 5 fast entries.
        for _ in 0..5 {
            m.record(1000, true, None);
        }
        for _ in 0..5 {
            m.record(10, true, None);
        }
        // last 5 entries → avg should be ~10
        let s = m.summary(Some(5));
        assert!((s.avg_latency_ms - 10.0).abs() < 1e-9);
        assert_eq!(s.total_requests, 5);
    }

    // ── TTFT ─────────────────────────────────────────────────────────────

    #[test]
    fn ttft_avg_is_none_when_no_streaming_entries() {
        let m = make_metrics(10);
        m.record(100, true, None);
        let s = m.summary(None);
        assert!(s.avg_first_token_latency_ms.is_none());
    }

    #[test]
    fn ttft_avg_computed_over_present_entries_only() {
        let m = make_metrics(10);
        m.record(100, true, Some(50));
        m.record(200, true, None); // no TTFT
        m.record(300, true, Some(150));
        let s = m.summary(None);
        // avg of 50 and 150 = 100
        assert!(
            (s.avg_first_token_latency_ms.unwrap() - 100.0).abs() < 1e-9,
            "ttft avg = {:?}",
            s.avg_first_token_latency_ms
        );
    }

    // ── recent helpers ───────────────────────────────────────────────────

    #[test]
    fn recent_success_rate_covers_only_last_n() {
        let m = make_metrics(20);
        // 5 failures, then 5 successes.
        for _ in 0..5 {
            m.record(100, false, None);
        }
        for _ in 0..5 {
            m.record(100, true, None);
        }
        // Last 5 → 100% success.
        assert!((m.recent_success_rate(5) - 1.0).abs() < 1e-9);
        // All 10 → 50% success.
        assert!((m.recent_success_rate(10) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn recent_avg_latency_covers_only_last_n() {
        let m = make_metrics(20);
        for _ in 0..5 {
            m.record(1000, true, None);
        }
        for _ in 0..5 {
            m.record(100, true, None);
        }
        assert!((m.recent_avg_latency(5) - 100.0).abs() < 1e-9);
    }

    // ── Send + Sync ──────────────────────────────────────────────────────

    #[test]
    fn metrics_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SlidingWindowMetrics>();
    }
}
