//! Fixed millisecond buckets. The owning registry locks observation and snapshots together.
use std::fmt::Write as _;

pub const LATENCY_BOUNDS_MS: [u64; 15] = [
    5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000, 30000, 60000, 120000, 300000,
];

#[derive(Debug, Default, Clone)]
pub(super) struct LatencyHistogram {
    buckets: [u64; 15],
    pub count: u64,
    pub sum: u64,
}

impl LatencyHistogram {
    pub fn observe(&mut self, value: u64) {
        // Only the first containing bucket is stored; export makes it cumulative.
        if let Some(index) = LATENCY_BOUNDS_MS.iter().position(|bound| value <= *bound) {
            self.buckets[index] = self.buckets[index].saturating_add(1);
        }
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.saturating_add(value);
    }

    pub fn render(&self, output: &mut String, name: &str, labels: &str) {
        let separator = if labels.is_empty() { "" } else { "," };
        let mut cumulative = 0u64;
        for (bound, count) in LATENCY_BOUNDS_MS.iter().zip(self.buckets) {
            cumulative = cumulative.saturating_add(count);
            let _ = writeln!(
                output,
                "{name}_bucket{{{labels}{separator}le=\"{bound}\"}} {cumulative}"
            );
        }
        let _ = writeln!(
            output,
            "{name}_bucket{{{labels}{separator}le=\"+Inf\"}} {}",
            self.count
        );
        let suffix = if labels.is_empty() {
            String::new()
        } else {
            format!("{{{labels}}}")
        };
        let _ = writeln!(output, "{name}_sum{suffix} {}", self.sum);
        let _ = writeln!(output, "{name}_count{suffix} {}", self.count);
    }
}
