use super::impact::normalize_window_minutes;
use super::*;

pub async fn get_anomaly_remediation_effectiveness(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
    window_minutes: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSummaryView, GatewayError> {
    let window_minutes = normalize_window_minutes(window_minutes);
    let runs = list_anomaly_incident_remediation_runs(
        pool,
        &GatewayAnalysisAnomalyRemediationRunFilters {
            limit: Some(filters.limit.unwrap_or(100).clamp(1, 200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut impacts = Vec::with_capacity(runs.len());
    for run in &runs {
        if run.status != "applied" {
            impacts.push(None);
            continue;
        }
        let impact =
            get_anomaly_incident_remediation_run_impact(pool, &run.id, Some(window_minutes))
                .await
                .ok();
        impacts.push(impact);
    }

    Ok(build_remediation_effectiveness_summary(
        OffsetDateTime::now_utc(),
        window_minutes,
        &runs,
        &impacts,
    ))
}

pub(super) fn build_remediation_effectiveness_summary(
    generated_at: OffsetDateTime,
    window_minutes: i32,
    runs: &[GatewayAnalysisAnomalyIncidentRemediationRunView],
    impacts: &[Option<GatewayAnalysisAnomalyRemediationRunImpactView>],
) -> GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
    let mut by_status = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut action_map = BTreeMap::new();

    let mut completion_rate = empty_effectiveness_metric();
    let mut failure_rate = empty_effectiveness_metric();
    let mut request_artifact_coverage = empty_effectiveness_metric();
    let mut response_artifact_coverage = empty_effectiveness_metric();
    let mut first_token_latency_ms_avg = empty_effectiveness_metric();
    let mut total_tokens_per_sample = empty_effectiveness_metric();

    let mut impacted_runs = 0;
    let mut unavailable_runs = 0;

    for (index, run) in runs.iter().enumerate() {
        accumulate_key_bucket(&mut by_status, Some(run.status.as_str()));
        accumulate_key_bucket(&mut by_execution_mode, Some(run.execution_mode.as_str()));
        accumulate_key_bucket(&mut by_action_key, Some(run.action_key.as_str()));

        let entry = action_map
            .entry(run.action_key.clone())
            .or_insert_with(|| empty_action_effectiveness(run.action_key.clone()));
        entry.run_count += 1;

        let Some(impact) = impacts.get(index).and_then(|item| item.as_ref()) else {
            unavailable_runs += 1;
            entry.unavailable_run_count += 1;
            add_metric_classification(&mut completion_rate, Classification::Unavailable);
            add_metric_classification(&mut failure_rate, Classification::Unavailable);
            add_metric_classification(&mut request_artifact_coverage, Classification::Unavailable);
            add_metric_classification(&mut response_artifact_coverage, Classification::Unavailable);
            add_metric_classification(&mut first_token_latency_ms_avg, Classification::Unavailable);
            add_metric_classification(&mut total_tokens_per_sample, Classification::Unavailable);
            add_metric_classification(&mut entry.completion_rate, Classification::Unavailable);
            add_metric_classification(&mut entry.failure_rate, Classification::Unavailable);
            add_metric_classification(
                &mut entry.request_artifact_coverage,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.response_artifact_coverage,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.first_token_latency_ms_avg,
                Classification::Unavailable,
            );
            add_metric_classification(
                &mut entry.total_tokens_per_sample,
                Classification::Unavailable,
            );
            continue;
        };

        impacted_runs += 1;
        entry.impacted_run_count += 1;
        let classifications = [
            (
                "completion_rate",
                classify_metric(
                    &impact.metrics.completion_rate,
                    MetricDirection::HigherBetter,
                    Some(0.02),
                    None,
                ),
            ),
            (
                "failure_rate",
                classify_metric(
                    &impact.metrics.failure_rate,
                    MetricDirection::LowerBetter,
                    Some(0.02),
                    None,
                ),
            ),
            (
                "request_artifact_coverage",
                classify_metric(
                    &impact.metrics.request_artifact_coverage,
                    MetricDirection::HigherBetter,
                    Some(0.05),
                    None,
                ),
            ),
            (
                "response_artifact_coverage",
                classify_metric(
                    &impact.metrics.response_artifact_coverage,
                    MetricDirection::HigherBetter,
                    Some(0.05),
                    None,
                ),
            ),
            (
                "first_token_latency_ms_avg",
                classify_metric(
                    &impact.metrics.first_token_latency_ms_avg,
                    MetricDirection::LowerBetter,
                    Some(50.0),
                    None,
                ),
            ),
            (
                "total_tokens_per_sample",
                classify_metric(
                    &impact.metrics.total_tokens_per_sample,
                    MetricDirection::LowerBetter,
                    None,
                    Some(0.1),
                ),
            ),
        ];
        for (key, classification) in classifications {
            match key {
                "completion_rate" => {
                    add_metric_classification(&mut completion_rate, classification);
                    add_metric_classification(&mut entry.completion_rate, classification);
                }
                "failure_rate" => {
                    add_metric_classification(&mut failure_rate, classification);
                    add_metric_classification(&mut entry.failure_rate, classification);
                }
                "request_artifact_coverage" => {
                    add_metric_classification(&mut request_artifact_coverage, classification);
                    add_metric_classification(&mut entry.request_artifact_coverage, classification);
                }
                "response_artifact_coverage" => {
                    add_metric_classification(&mut response_artifact_coverage, classification);
                    add_metric_classification(
                        &mut entry.response_artifact_coverage,
                        classification,
                    );
                }
                "first_token_latency_ms_avg" => {
                    add_metric_classification(&mut first_token_latency_ms_avg, classification);
                    add_metric_classification(
                        &mut entry.first_token_latency_ms_avg,
                        classification,
                    );
                }
                "total_tokens_per_sample" => {
                    add_metric_classification(&mut total_tokens_per_sample, classification);
                    add_metric_classification(&mut entry.total_tokens_per_sample, classification);
                }
                _ => {}
            }
        }
    }

    GatewayAnalysisAnomalyRemediationEffectivenessSummaryView {
        generated_at: format_timestamp(generated_at),
        window_minutes,
        total_runs: runs.len(),
        impacted_runs,
        unavailable_runs,
        by_status: into_key_buckets(by_status),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_action_key: into_key_buckets(by_action_key),
        completion_rate,
        failure_rate,
        request_artifact_coverage,
        response_artifact_coverage,
        first_token_latency_ms_avg,
        total_tokens_per_sample,
        actions: action_map.into_values().collect(),
    }
}

fn empty_effectiveness_metric() -> GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
    GatewayAnalysisAnomalyRemediationEffectivenessMetricView {
        improved_runs: 0,
        regressed_runs: 0,
        neutral_runs: 0,
        unavailable_runs: 0,
    }
}

fn empty_action_effectiveness(
    action_key: String,
) -> GatewayAnalysisAnomalyRemediationActionEffectivenessView {
    GatewayAnalysisAnomalyRemediationActionEffectivenessView {
        action_key,
        run_count: 0,
        impacted_run_count: 0,
        unavailable_run_count: 0,
        completion_rate: empty_effectiveness_metric(),
        failure_rate: empty_effectiveness_metric(),
        request_artifact_coverage: empty_effectiveness_metric(),
        response_artifact_coverage: empty_effectiveness_metric(),
        first_token_latency_ms_avg: empty_effectiveness_metric(),
        total_tokens_per_sample: empty_effectiveness_metric(),
    }
}

fn classify_metric(
    metric: &GatewayAnalysisAnomalyRemediationImpactMetricView,
    direction: MetricDirection,
    absolute_threshold: Option<f64>,
    ratio_threshold: Option<f64>,
) -> Classification {
    if metric.before_value.is_none() || metric.after_value.is_none() || metric.delta_value.is_none()
    {
        return Classification::Unavailable;
    }
    if let Some(ratio_threshold) = ratio_threshold {
        let Some(delta_ratio) = metric.delta_ratio else {
            return Classification::Unavailable;
        };
        if delta_ratio.abs() < ratio_threshold {
            return Classification::Neutral;
        }
        return match direction {
            MetricDirection::HigherBetter => {
                if delta_ratio > 0.0 {
                    Classification::Improved
                } else {
                    Classification::Regressed
                }
            }
            MetricDirection::LowerBetter => {
                if delta_ratio < 0.0 {
                    Classification::Improved
                } else {
                    Classification::Regressed
                }
            }
        };
    }

    let delta_value = metric.delta_value.unwrap_or(0.0);
    if delta_value.abs() < absolute_threshold.unwrap_or(0.0) {
        return Classification::Neutral;
    }
    match direction {
        MetricDirection::HigherBetter => {
            if delta_value > 0.0 {
                Classification::Improved
            } else {
                Classification::Regressed
            }
        }
        MetricDirection::LowerBetter => {
            if delta_value < 0.0 {
                Classification::Improved
            } else {
                Classification::Regressed
            }
        }
    }
}

fn add_metric_classification(
    metric: &mut GatewayAnalysisAnomalyRemediationEffectivenessMetricView,
    classification: Classification,
) {
    match classification {
        Classification::Improved => metric.improved_runs += 1,
        Classification::Regressed => metric.regressed_runs += 1,
        Classification::Neutral => metric.neutral_runs += 1,
        Classification::Unavailable => metric.unavailable_runs += 1,
    }
}

#[derive(Debug, Clone, Copy)]
enum MetricDirection {
    HigherBetter,
    LowerBetter,
}

#[derive(Debug, Clone, Copy)]
enum Classification {
    Improved,
    Regressed,
    Neutral,
    Unavailable,
}
