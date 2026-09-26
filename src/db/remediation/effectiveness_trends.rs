use super::impact::ratio;
use super::*;

pub async fn get_anomaly_remediation_effectiveness_trend_report(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView, GatewayError> {
    let limit = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
        limit: Some(limit),
        ..filters.clone()
    };
    let snapshots = list_anomaly_remediation_effectiveness_snapshots(&normalized_filters).await?;
    let inventory_summary = summarize_anomaly_remediation_effectiveness_snapshots(
        &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
            limit: Some(500),
            ..normalized_filters.clone()
        },
    )
    .await?;
    let points = snapshots
        .into_iter()
        .map(build_remediation_effectiveness_trend_point)
        .collect::<Vec<_>>();

    Ok(
        GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView {
            generated_at: format_timestamp(OffsetDateTime::now_utc()),
            filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotReportFilterView {
                label: normalized_filters.label,
                route_policy_id: normalized_filters.route_policy_id,
                action_key: normalized_filters.action_key,
                created_from: normalized_filters.created_from,
                created_to: normalized_filters.created_to,
            },
            matched_snapshots_count: points.len(),
            window_size: limit,
            inventory_summary,
            summary: build_remediation_effectiveness_trend_summary(&points),
            points,
        },
    )
}

fn build_remediation_effectiveness_trend_point(
    snapshot: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
    let total_runs = snapshot.summary.total_runs;
    GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView {
        total_runs,
        impacted_run_rate: ratio(snapshot.summary.impacted_runs, total_runs),
        unavailable_run_rate: ratio(snapshot.summary.unavailable_runs, total_runs),
        completion_rate: build_trend_metric_point(&snapshot.summary.completion_rate),
        failure_rate: build_trend_metric_point(&snapshot.summary.failure_rate),
        request_artifact_coverage: build_trend_metric_point(
            &snapshot.summary.request_artifact_coverage,
        ),
        response_artifact_coverage: build_trend_metric_point(
            &snapshot.summary.response_artifact_coverage,
        ),
        first_token_latency_ms_avg: build_trend_metric_point(
            &snapshot.summary.first_token_latency_ms_avg,
        ),
        total_tokens_per_sample: build_trend_metric_point(
            &snapshot.summary.total_tokens_per_sample,
        ),
        snapshot,
    }
}

fn build_trend_metric_point(
    metric: &GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
    let total = metric.improved_runs
        + metric.regressed_runs
        + metric.neutral_runs
        + metric.unavailable_runs;
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricPointView {
        improved_rate: ratio(metric.improved_runs, total),
        regressed_rate: ratio(metric.regressed_runs, total),
        neutral_rate: ratio(metric.neutral_runs, total),
        unavailable_rate: ratio(metric.unavailable_runs, total),
    }
}

fn build_remediation_effectiveness_trend_summary(
    points: &[GatewayAnalysisAnomalyRemediationEffectivenessTrendPointView],
) -> Option<GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(
        GatewayAnalysisAnomalyRemediationEffectivenessTrendSummaryView {
            latest_snapshot_id: Some(latest.snapshot.snapshot_id.clone()),
            previous_snapshot_id: previous.map(|item| item.snapshot.snapshot_id.clone()),
            total_runs: build_trend_metric_summary(
                Some(latest.total_runs as f64),
                previous.map(|item| item.total_runs as f64),
            ),
            impacted_run_rate: build_trend_metric_summary(
                latest.impacted_run_rate,
                previous.and_then(|item| item.impacted_run_rate),
            ),
            unavailable_run_rate: build_trend_metric_summary(
                latest.unavailable_run_rate,
                previous.and_then(|item| item.unavailable_run_rate),
            ),
            completion_rate_regressed: build_trend_metric_summary(
                latest.completion_rate.regressed_rate,
                previous.and_then(|item| item.completion_rate.regressed_rate),
            ),
            failure_rate_regressed: build_trend_metric_summary(
                latest.failure_rate.regressed_rate,
                previous.and_then(|item| item.failure_rate.regressed_rate),
            ),
            request_artifact_coverage_regressed: build_trend_metric_summary(
                latest.request_artifact_coverage.regressed_rate,
                previous.and_then(|item| item.request_artifact_coverage.regressed_rate),
            ),
            response_artifact_coverage_regressed: build_trend_metric_summary(
                latest.response_artifact_coverage.regressed_rate,
                previous.and_then(|item| item.response_artifact_coverage.regressed_rate),
            ),
            first_token_latency_ms_avg_regressed: build_trend_metric_summary(
                latest.first_token_latency_ms_avg.regressed_rate,
                previous.and_then(|item| item.first_token_latency_ms_avg.regressed_rate),
            ),
            total_tokens_per_sample_regressed: build_trend_metric_summary(
                latest.total_tokens_per_sample.regressed_rate,
                previous.and_then(|item| item.total_tokens_per_sample.regressed_rate),
            ),
        },
    )
}

fn build_trend_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta)) if previous != 0.0 => Some(delta / previous),
        _ => None,
    };
    GatewayAnalysisAnomalyRemediationEffectivenessTrendMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}
