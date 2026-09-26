use super::*;

pub async fn get_anomaly_remediation_effectiveness_snapshot_anomaly_report(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_profile_key(profile_key);
    let thresholds =
        build_remediation_effectiveness_threshold_config(&normalized_profile_key, overrides);
    let trend_report = get_anomaly_remediation_effectiveness_trend_report(filters).await?;
    Ok(build_remediation_effectiveness_anomaly_report(
        trend_report,
        normalized_profile_key,
        thresholds,
    ))
}

fn build_remediation_effectiveness_threshold_config(
    profile_key: &str,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.5,
            impacted_run_rate_critical_threshold: 0.35,
            unavailable_run_rate_warning_threshold: 0.4,
            unavailable_run_rate_critical_threshold: 0.6,
            completion_rate_regressed_warning_threshold: 0.35,
            completion_rate_regressed_critical_threshold: 0.55,
            failure_rate_regressed_warning_threshold: 0.35,
            failure_rate_regressed_critical_threshold: 0.55,
            request_artifact_regressed_warning_threshold: 0.35,
            request_artifact_regressed_critical_threshold: 0.55,
            response_artifact_regressed_warning_threshold: 0.35,
            response_artifact_regressed_critical_threshold: 0.55,
            first_token_latency_regressed_warning_threshold: 0.35,
            first_token_latency_regressed_critical_threshold: 0.55,
            total_tokens_regressed_warning_threshold: 0.35,
            total_tokens_regressed_critical_threshold: 0.55,
        },
        "aggressive" => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.75,
            impacted_run_rate_critical_threshold: 0.6,
            unavailable_run_rate_warning_threshold: 0.18,
            unavailable_run_rate_critical_threshold: 0.3,
            completion_rate_regressed_warning_threshold: 0.18,
            completion_rate_regressed_critical_threshold: 0.3,
            failure_rate_regressed_warning_threshold: 0.18,
            failure_rate_regressed_critical_threshold: 0.3,
            request_artifact_regressed_warning_threshold: 0.18,
            request_artifact_regressed_critical_threshold: 0.3,
            response_artifact_regressed_warning_threshold: 0.18,
            response_artifact_regressed_critical_threshold: 0.3,
            first_token_latency_regressed_warning_threshold: 0.18,
            first_token_latency_regressed_critical_threshold: 0.3,
            total_tokens_regressed_warning_threshold: 0.18,
            total_tokens_regressed_critical_threshold: 0.3,
        },
        _ => GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
            impacted_run_rate_warning_threshold: 0.65,
            impacted_run_rate_critical_threshold: 0.45,
            unavailable_run_rate_warning_threshold: 0.25,
            unavailable_run_rate_critical_threshold: 0.4,
            completion_rate_regressed_warning_threshold: 0.25,
            completion_rate_regressed_critical_threshold: 0.4,
            failure_rate_regressed_warning_threshold: 0.25,
            failure_rate_regressed_critical_threshold: 0.4,
            request_artifact_regressed_warning_threshold: 0.25,
            request_artifact_regressed_critical_threshold: 0.4,
            response_artifact_regressed_warning_threshold: 0.25,
            response_artifact_regressed_critical_threshold: 0.4,
            first_token_latency_regressed_warning_threshold: 0.25,
            first_token_latency_regressed_critical_threshold: 0.4,
            total_tokens_regressed_warning_threshold: 0.25,
            total_tokens_regressed_critical_threshold: 0.4,
        },
    };
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig {
        impacted_run_rate_warning_threshold: overrides
            .impacted_run_rate_warning_threshold
            .unwrap_or(base.impacted_run_rate_warning_threshold),
        impacted_run_rate_critical_threshold: overrides
            .impacted_run_rate_critical_threshold
            .unwrap_or(base.impacted_run_rate_critical_threshold),
        unavailable_run_rate_warning_threshold: overrides
            .unavailable_run_rate_warning_threshold
            .unwrap_or(base.unavailable_run_rate_warning_threshold),
        unavailable_run_rate_critical_threshold: overrides
            .unavailable_run_rate_critical_threshold
            .unwrap_or(base.unavailable_run_rate_critical_threshold),
        completion_rate_regressed_warning_threshold: overrides
            .completion_rate_regressed_warning_threshold
            .unwrap_or(base.completion_rate_regressed_warning_threshold),
        completion_rate_regressed_critical_threshold: overrides
            .completion_rate_regressed_critical_threshold
            .unwrap_or(base.completion_rate_regressed_critical_threshold),
        failure_rate_regressed_warning_threshold: overrides
            .failure_rate_regressed_warning_threshold
            .unwrap_or(base.failure_rate_regressed_warning_threshold),
        failure_rate_regressed_critical_threshold: overrides
            .failure_rate_regressed_critical_threshold
            .unwrap_or(base.failure_rate_regressed_critical_threshold),
        request_artifact_regressed_warning_threshold: overrides
            .request_artifact_regressed_warning_threshold
            .unwrap_or(base.request_artifact_regressed_warning_threshold),
        request_artifact_regressed_critical_threshold: overrides
            .request_artifact_regressed_critical_threshold
            .unwrap_or(base.request_artifact_regressed_critical_threshold),
        response_artifact_regressed_warning_threshold: overrides
            .response_artifact_regressed_warning_threshold
            .unwrap_or(base.response_artifact_regressed_warning_threshold),
        response_artifact_regressed_critical_threshold: overrides
            .response_artifact_regressed_critical_threshold
            .unwrap_or(base.response_artifact_regressed_critical_threshold),
        first_token_latency_regressed_warning_threshold: overrides
            .first_token_latency_regressed_warning_threshold
            .unwrap_or(base.first_token_latency_regressed_warning_threshold),
        first_token_latency_regressed_critical_threshold: overrides
            .first_token_latency_regressed_critical_threshold
            .unwrap_or(base.first_token_latency_regressed_critical_threshold),
        total_tokens_regressed_warning_threshold: overrides
            .total_tokens_regressed_warning_threshold
            .unwrap_or(base.total_tokens_regressed_warning_threshold),
        total_tokens_regressed_critical_threshold: overrides
            .total_tokens_regressed_critical_threshold
            .unwrap_or(base.total_tokens_regressed_critical_threshold),
    }
}

fn build_remediation_effectiveness_anomaly_report(
    trend_report: GatewayAnalysisAnomalyRemediationEffectivenessTrendReportView,
    profile_key: String,
    thresholds: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyThresholdConfig,
) -> GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
    let mut anomalies = Vec::new();
    if let Some(summary) = trend_report.summary.as_ref() {
        push_remediation_effectiveness_anomaly(
            &mut anomalies,
            "impacted_run_rate_drop",
            "治理效果样本命中率偏低，当前窗口里可评估 run 太少，效果结论不稳定。",
            summary.impacted_run_rate.latest_value,
            summary.impacted_run_rate.previous_value,
            summary.impacted_run_rate.delta_value,
            summary.impacted_run_rate.delta_ratio,
            thresholds.impacted_run_rate_warning_threshold,
            thresholds.impacted_run_rate_critical_threshold,
            true,
            summary.latest_snapshot_id.as_deref(),
            summary.previous_snapshot_id.as_deref(),
        );
        push_remediation_effectiveness_anomaly(
            &mut anomalies,
            "unavailable_run_rate_spike",
            "治理效果快照里的 unavailable run 比例过高，impact capture 链路需要排查。",
            summary.unavailable_run_rate.latest_value,
            summary.unavailable_run_rate.previous_value,
            summary.unavailable_run_rate.delta_value,
            summary.unavailable_run_rate.delta_ratio,
            thresholds.unavailable_run_rate_warning_threshold,
            thresholds.unavailable_run_rate_critical_threshold,
            false,
            summary.latest_snapshot_id.as_deref(),
            summary.previous_snapshot_id.as_deref(),
        );
        let checks = [
            (
                "completion_effectiveness_regressed",
                "completion 效果回归比例偏高，最近一批 remediation 可能没有改善完成率。",
                &summary.completion_rate_regressed,
                thresholds.completion_rate_regressed_warning_threshold,
                thresholds.completion_rate_regressed_critical_threshold,
            ),
            (
                "failure_effectiveness_regressed",
                "failure 效果回归比例偏高，最近一批 remediation 可能放大了失败率。",
                &summary.failure_rate_regressed,
                thresholds.failure_rate_regressed_warning_threshold,
                thresholds.failure_rate_regressed_critical_threshold,
            ),
            (
                "request_artifact_effectiveness_regressed",
                "request artifact 效果回归比例偏高，治理动作可能伤到了留存链路。",
                &summary.request_artifact_coverage_regressed,
                thresholds.request_artifact_regressed_warning_threshold,
                thresholds.request_artifact_regressed_critical_threshold,
            ),
            (
                "response_artifact_effectiveness_regressed",
                "response artifact 效果回归比例偏高，治理动作可能伤到了 response 留存覆盖率。",
                &summary.response_artifact_coverage_regressed,
                thresholds.response_artifact_regressed_warning_threshold,
                thresholds.response_artifact_regressed_critical_threshold,
            ),
            (
                "latency_effectiveness_regressed",
                "首 token latency 的回归比例偏高，最近一批 remediation 可能拖慢了热路径。",
                &summary.first_token_latency_ms_avg_regressed,
                thresholds.first_token_latency_regressed_warning_threshold,
                thresholds.first_token_latency_regressed_critical_threshold,
            ),
            (
                "token_effectiveness_regressed",
                "单样本 token 成本的回归比例偏高，最近一批 remediation 可能提高了成本。",
                &summary.total_tokens_per_sample_regressed,
                thresholds.total_tokens_regressed_warning_threshold,
                thresholds.total_tokens_regressed_critical_threshold,
            ),
        ];
        for (code, message, metric, warning, critical) in checks {
            if metric.latest_value.unwrap_or(0.0) < warning {
                continue;
            }
            anomalies.push(GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
                code: code.to_string(),
                severity: if metric.latest_value.unwrap_or(0.0) >= critical {
                    "critical".to_string()
                } else {
                    "warning".to_string()
                },
                message: message.to_string(),
                latest_snapshot_id: summary.latest_snapshot_id.clone(),
                previous_snapshot_id: summary.previous_snapshot_id.clone(),
                latest_value: metric.latest_value,
                previous_value: metric.previous_value,
                delta_value: metric.delta_value,
                delta_ratio: metric.delta_ratio,
                threshold_value: Some(warning),
            });
        }
    }
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    for anomaly in &anomalies {
        accumulate_key_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_key_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }
    GatewayAnalysisAnomalyRemediationEffectivenessAnomalyReportView {
        generated_at: trend_report.generated_at.clone(),
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        latest_snapshot: trend_report
            .points
            .first()
            .map(|item| item.snapshot.clone()),
        previous_snapshot: trend_report.points.get(1).map(|item| item.snapshot.clone()),
        trend_summary: trend_report.summary,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

fn push_remediation_effectiveness_anomaly(
    anomalies: &mut Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView>,
    code: &str,
    message: &str,
    latest_value: Option<f64>,
    previous_value: Option<f64>,
    delta_value: Option<f64>,
    delta_ratio: Option<f64>,
    warning: f64,
    critical: f64,
    lower_is_worse: bool,
    latest_snapshot_id: Option<&str>,
    previous_snapshot_id: Option<&str>,
) {
    let value = latest_value.unwrap_or(if lower_is_worse { 1.0 } else { 0.0 });
    let triggered = if lower_is_worse {
        value <= warning
    } else {
        value >= warning
    };
    if !triggered {
        return;
    }
    let severity = if if lower_is_worse {
        value <= critical
    } else {
        value >= critical
    } {
        "critical"
    } else {
        "warning"
    };
    anomalies.push(GatewayAnalysisAnomalyRemediationEffectivenessAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_snapshot_id: latest_snapshot_id.map(|value| value.to_string()),
        previous_snapshot_id: previous_snapshot_id.map(|value| value.to_string()),
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
        threshold_value: Some(warning),
    });
}

pub(super) fn normalize_profile_key(profile_key: Option<&str>) -> String {
    match profile_key
        .and_then(trimmed_owned_ref)
        .unwrap_or("balanced")
        .to_lowercase()
        .as_str()
    {
        "conservative" => "conservative".to_string(),
        "aggressive" => "aggressive".to_string(),
        _ => "balanced".to_string(),
    }
}
