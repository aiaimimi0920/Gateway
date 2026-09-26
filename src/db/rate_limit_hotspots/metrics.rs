use super::*;

pub(super) fn accumulate_bucket(target: &mut BTreeMap<String, usize>, key: Option<&str>) {
    let Some(normalized) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    *target.entry(normalized.to_string()).or_insert(0) += 1;
}

pub(super) fn into_key_buckets(map: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketKeyView> {
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

pub(super) fn top_bucket_share(
    buckets: &[GatewaySummaryBucketKeyView],
    total: usize,
) -> Option<f64> {
    let count = buckets.first().map(|bucket| bucket.count)?;
    if total == 0 {
        return None;
    }
    Some(count as f64 / total as f64)
}

pub(super) fn build_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayRateLimitHotspotMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta)) if previous != 0.0 => Some(delta / previous),
        _ => None,
    };
    GatewayRateLimitHotspotMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}

pub(super) fn merge_key_buckets(
    target: &mut BTreeMap<String, usize>,
    buckets: &[GatewaySummaryBucketKeyView],
) {
    for bucket in buckets {
        push_key_bucket(target, Some(bucket.key.as_str()), bucket.count);
    }
}

pub(super) fn push_key_bucket(
    target: &mut BTreeMap<String, usize>,
    key: Option<&str>,
    count: usize,
) {
    let Some(normalized) = key.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if count == 0 {
        return;
    }
    *target.entry(normalized.to_string()).or_insert(0) += count;
}
