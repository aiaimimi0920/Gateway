use super::*;

pub(crate) fn build_rate_limit_hotspot_summary(
    rows: &[GatewayRequestAuditView],
) -> GatewayRateLimitHotspotSummaryView {
    let mut by_code = BTreeMap::new();
    let mut by_project = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_api_key_id = BTreeMap::new();
    let mut by_requested_model = BTreeMap::new();
    let mut by_resolved_model = BTreeMap::new();
    let mut by_endpoint_kind = BTreeMap::new();
    let mut total_rate_limited_requests = 0usize;

    for row in rows {
        let Some(error_code) = route_trace_error_code(row) else {
            continue;
        };
        if !is_rate_limit_error_code(Some(error_code)) {
            continue;
        }
        total_rate_limited_requests += 1;
        accumulate_bucket(&mut by_code, Some(error_code));
        accumulate_bucket(&mut by_project, Some(row.project_id.as_str()));
        accumulate_bucket(&mut by_route_policy_id, row.route_policy_id.as_deref());
        accumulate_bucket(&mut by_api_key_id, row.api_key_id.as_deref());
        accumulate_bucket(&mut by_requested_model, row.requested_model.as_deref());
        accumulate_bucket(&mut by_resolved_model, row.resolved_model.as_deref());
        accumulate_bucket(&mut by_endpoint_kind, Some(row.endpoint_kind.as_str()));
    }

    GatewayRateLimitHotspotSummaryView {
        total_rate_limited_requests,
        by_code: into_key_buckets(by_code),
        by_project: into_key_buckets(by_project),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_api_key_id: into_key_buckets(by_api_key_id),
        by_requested_model: into_key_buckets(by_requested_model),
        by_resolved_model: into_key_buckets(by_resolved_model),
        by_endpoint_kind: into_key_buckets(by_endpoint_kind),
    }
}

pub(crate) fn build_rate_limit_hotspot_trend_report(
    generated_at: String,
    filters: GatewayRateLimitHotspotFilterView,
    rows: &[GatewayRequestAuditView],
) -> GatewayRateLimitHotspotTrendReportView {
    let rate_limit_rows = rows
        .iter()
        .filter(|row| is_rate_limit_error_code(route_trace_error_code(row)))
        .cloned()
        .collect::<Vec<_>>();
    let bucket_size_minutes = filters.bucket_size_minutes.max(1);
    let window_size = filters.window_size.max(1);
    let bucket_size_ms = i128::from(bucket_size_minutes as i64) * 60 * 1000;
    let anchor_ms = rate_limit_rows
        .iter()
        .filter_map(|row| parse_timestamp_ms(row.created_at.as_str()))
        .max()
        .unwrap_or_else(current_time_ms);
    let latest_bucket_start_ms = anchor_ms.div_euclid(bucket_size_ms) * bucket_size_ms;
    let mut points = Vec::with_capacity(window_size);

    for index in 0..window_size {
        let bucket_start_ms = latest_bucket_start_ms - i128::from(index as i64) * bucket_size_ms;
        let bucket_end_ms = bucket_start_ms + bucket_size_ms;
        let bucket_rows = rate_limit_rows
            .iter()
            .filter(|row| {
                parse_timestamp_ms(row.created_at.as_str())
                    .is_some_and(|value| value >= bucket_start_ms && value < bucket_end_ms)
            })
            .cloned()
            .collect::<Vec<_>>();
        let summary = build_rate_limit_hotspot_summary(&bucket_rows);
        points.push(GatewayRateLimitHotspotTrendPointView {
            bucket_start_at: format_timestamp_ms(bucket_start_ms),
            bucket_end_at: format_timestamp_ms(bucket_end_ms),
            total_rate_limited_requests: summary.total_rate_limited_requests,
            by_code: summary.by_code,
            by_project: summary.by_project,
            by_route_policy_id: summary.by_route_policy_id,
            by_api_key_id: summary.by_api_key_id,
            by_requested_model: summary.by_requested_model,
            by_resolved_model: summary.by_resolved_model,
            by_endpoint_kind: summary.by_endpoint_kind,
        });
    }

    GatewayRateLimitHotspotTrendReportView {
        generated_at,
        filters,
        matched_requests_count: rate_limit_rows.len(),
        window_size,
        bucket_size_minutes,
        summary: build_rate_limit_hotspot_trend_summary(&points),
        points,
    }
}

fn build_rate_limit_hotspot_trend_summary(
    points: &[GatewayRateLimitHotspotTrendPointView],
) -> Option<GatewayRateLimitHotspotTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayRateLimitHotspotTrendSummaryView {
        latest_bucket_start_at: Some(latest.bucket_start_at.clone()),
        previous_bucket_start_at: previous.map(|value| value.bucket_start_at.clone()),
        total_rate_limited_requests: build_metric_summary(
            Some(latest.total_rate_limited_requests as f64),
            previous.map(|value| value.total_rate_limited_requests as f64),
        ),
        top_code_share: build_metric_summary(
            top_bucket_share(&latest.by_code, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_code, value.total_rate_limited_requests)
            }),
        ),
        top_project_share: build_metric_summary(
            top_bucket_share(&latest.by_project, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_project, value.total_rate_limited_requests)
            }),
        ),
        top_api_key_share: build_metric_summary(
            top_bucket_share(&latest.by_api_key_id, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_api_key_id, value.total_rate_limited_requests)
            }),
        ),
        top_requested_model_share: build_metric_summary(
            top_bucket_share(
                &latest.by_requested_model,
                latest.total_rate_limited_requests,
            ),
            previous.and_then(|value| {
                top_bucket_share(&value.by_requested_model, value.total_rate_limited_requests)
            }),
        ),
        top_endpoint_share: build_metric_summary(
            top_bucket_share(&latest.by_endpoint_kind, latest.total_rate_limited_requests),
            previous.and_then(|value| {
                top_bucket_share(&value.by_endpoint_kind, value.total_rate_limited_requests)
            }),
        ),
        latest_top_code_key: latest.by_code.first().map(|bucket| bucket.key.clone()),
        latest_top_project_key: latest.by_project.first().map(|bucket| bucket.key.clone()),
        latest_top_api_key_key: latest
            .by_api_key_id
            .first()
            .map(|bucket| bucket.key.clone()),
        latest_top_requested_model_key: latest
            .by_requested_model
            .first()
            .map(|bucket| bucket.key.clone()),
        latest_top_endpoint_key: latest
            .by_endpoint_kind
            .first()
            .map(|bucket| bucket.key.clone()),
    })
}

fn current_time_ms() -> i128 {
    OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000
}

fn parse_timestamp_ms(value: &str) -> Option<i128> {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp_nanos() / 1_000_000)
}

fn format_timestamp_ms(value: i128) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(value * 1_000_000)
        .map(format_timestamp)
        .unwrap_or_else(|_| value.to_string())
}

fn route_trace_error_code(row: &GatewayRequestAuditView) -> Option<&str> {
    row.route_trace
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|object| object.get("errorCode"))
        .and_then(Value::as_str)
}

fn is_rate_limit_error_code(value: Option<&str>) -> bool {
    let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
    normalized.starts_with("rate_limit_exceeded") || normalized.starts_with("rate-limit")
}
