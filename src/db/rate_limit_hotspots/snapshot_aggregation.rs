use super::*;

pub(crate) fn build_rate_limit_hotspot_snapshot_inventory_summary(
    snapshots: &[GatewayRateLimitHotspotSnapshotView],
) -> GatewayRateLimitHotspotSnapshotInventorySummaryView {
    let mut by_code = BTreeMap::new();
    let mut by_project = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_api_key_id = BTreeMap::new();
    let mut by_requested_model = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut by_label = BTreeMap::new();
    let mut total_rate_limited_requests = 0usize;

    for snapshot in snapshots {
        total_rate_limited_requests += snapshot.summary.total_rate_limited_requests;
        merge_key_buckets(&mut by_code, &snapshot.summary.by_code);
        merge_key_buckets(&mut by_project, &snapshot.summary.by_project);
        merge_key_buckets(
            &mut by_route_policy_id,
            &snapshot.summary.by_route_policy_id,
        );
        merge_key_buckets(&mut by_api_key_id, &snapshot.summary.by_api_key_id);
        merge_key_buckets(
            &mut by_requested_model,
            &snapshot.summary.by_requested_model,
        );
        merge_key_buckets(&mut by_resolved_model, &snapshot.summary.by_resolved_model);
        merge_key_buckets(&mut by_endpoint_kind, &snapshot.summary.by_endpoint_kind);
        push_key_bucket(&mut by_label, snapshot.label.as_deref(), 1);
    }

    GatewayRateLimitHotspotSnapshotInventorySummaryView {
        total_snapshots: snapshots.len(),
        total_rate_limited_requests,
        by_code: into_key_buckets(by_code),
        by_project: into_key_buckets(by_project),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_api_key_id: into_key_buckets(by_api_key_id),
        by_requested_model: into_key_buckets(by_requested_model),
        by_resolved_model: into_key_buckets(by_resolved_model),
        by_endpoint_kind: into_key_buckets(by_endpoint_kind),
        by_label: into_key_buckets(by_label),
    }
}

pub(crate) fn build_rate_limit_hotspot_snapshot_trend_point(
    snapshot: GatewayRateLimitHotspotSnapshotView,
) -> GatewayRateLimitHotspotSnapshotTrendPointView {
    let total = snapshot.summary.total_rate_limited_requests;
    GatewayRateLimitHotspotSnapshotTrendPointView {
        top_code_share: top_bucket_share(&snapshot.summary.by_code, total),
        top_project_share: top_bucket_share(&snapshot.summary.by_project, total),
        top_api_key_share: top_bucket_share(&snapshot.summary.by_api_key_id, total),
        top_requested_model_share: top_bucket_share(&snapshot.summary.by_requested_model, total),
        top_endpoint_share: top_bucket_share(&snapshot.summary.by_endpoint_kind, total),
        total_rate_limited_requests: total,
        snapshot,
    }
}

pub(crate) fn build_rate_limit_hotspot_snapshot_trend_report(
    generated_at: String,
    filters: GatewayRateLimitHotspotSnapshotReportFilterView,
    window_size: usize,
    inventory_summary: GatewayRateLimitHotspotSnapshotInventorySummaryView,
    points: Vec<GatewayRateLimitHotspotSnapshotTrendPointView>,
) -> GatewayRateLimitHotspotSnapshotTrendReportView {
    GatewayRateLimitHotspotSnapshotTrendReportView {
        matched_snapshots_count: points.len(),
        summary: build_rate_limit_hotspot_snapshot_trend_summary(&points),
        generated_at,
        filters,
        window_size,
        inventory_summary,
        points,
    }
}

fn build_rate_limit_hotspot_snapshot_trend_summary(
    points: &[GatewayRateLimitHotspotSnapshotTrendPointView],
) -> Option<GatewayRateLimitHotspotSnapshotTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayRateLimitHotspotSnapshotTrendSummaryView {
        latest_snapshot_id: Some(latest.snapshot.snapshot_id.clone()),
        previous_snapshot_id: previous.map(|value| value.snapshot.snapshot_id.clone()),
        total_rate_limited_requests: build_metric_summary(
            Some(latest.total_rate_limited_requests as f64),
            previous.map(|value| value.total_rate_limited_requests as f64),
        ),
        top_code_share: build_metric_summary(
            latest.top_code_share,
            previous.and_then(|value| value.top_code_share),
        ),
        top_project_share: build_metric_summary(
            latest.top_project_share,
            previous.and_then(|value| value.top_project_share),
        ),
        top_api_key_share: build_metric_summary(
            latest.top_api_key_share,
            previous.and_then(|value| value.top_api_key_share),
        ),
        top_requested_model_share: build_metric_summary(
            latest.top_requested_model_share,
            previous.and_then(|value| value.top_requested_model_share),
        ),
        top_endpoint_share: build_metric_summary(
            latest.top_endpoint_share,
            previous.and_then(|value| value.top_endpoint_share),
        ),
    })
}
