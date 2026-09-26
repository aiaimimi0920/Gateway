use super::*;

pub(super) fn accumulate_bucket(map: &mut BTreeMap<String, usize>, value: Option<&str>) {
    let key = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    *map.entry(key).or_default() += 1;
}

pub(super) fn into_summary_buckets(
    map: BTreeMap<String, usize>,
) -> Vec<GatewayRequestAuditSummaryBucketView> {
    let mut buckets = map
        .into_iter()
        .map(|(value, count)| GatewayRequestAuditSummaryBucketView { value, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    buckets
}

pub(super) fn into_keyed_summary_buckets(
    map: BTreeMap<String, usize>,
) -> Vec<GatewaySummaryBucketKeyView> {
    let mut buckets = map
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    buckets
}

pub(super) fn build_distribution(values: &[Option<i64>]) -> GatewayAnalysisMetricDistributionView {
    let mut normalized = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return GatewayAnalysisMetricDistributionView {
            avg: None,
            p50: None,
            p95: None,
        };
    }

    normalized.sort_unstable();
    let sum = normalized.iter().sum::<i64>() as f64;
    GatewayAnalysisMetricDistributionView {
        avg: Some((sum / normalized.len() as f64 * 100.0).round() / 100.0),
        p50: percentile(&normalized, 0.5),
        p95: percentile(&normalized, 0.95),
    }
}

pub(super) fn build_ts_distribution(
    values: &[Option<f64>],
) -> GatewayAnalysisMetricDistributionView {
    let mut normalized = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return GatewayAnalysisMetricDistributionView {
            avg: None,
            p50: None,
            p95: None,
        };
    }

    normalized.sort_by(|left, right| left.total_cmp(right));
    let average = normalized.iter().sum::<f64>() / normalized.len() as f64;

    GatewayAnalysisMetricDistributionView {
        avg: Some(round_metric(average, 3)),
        p50: percentile_by_rank(&normalized, 0.50),
        p95: percentile_by_rank(&normalized, 0.95),
    }
}

fn percentile(values: &[i64], ratio: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let index = ((values.len() - 1) as f64 * ratio).floor() as usize;
    values.get(index).map(|value| *value as f64)
}

fn percentile_by_rank(values: &[f64], ratio: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let index = (((ratio * values.len() as f64).ceil() as usize).max(1) - 1).min(values.len() - 1);
    values.get(index).copied()
}

pub(super) fn round_metric(value: f64, precision: u32) -> f64 {
    let factor = 10_f64.powi(precision as i32);
    (value * factor).round() / factor
}
