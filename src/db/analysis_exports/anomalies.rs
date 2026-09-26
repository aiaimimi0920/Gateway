use super::inventory::{accumulate_bucket, into_key_buckets};
use super::normalization::trimmed_owned_ref_opt;
use super::thresholds::{
    build_analysis_export_anomaly_threshold_config, normalize_analysis_export_profile_key,
};
use super::*;

struct AnalysisExportAnomalyContext {
    filters: GatewayPersistedAnalysisExportFilters,
    profile_key: String,
    thresholds: GatewayAnalysisExportAnomalyThresholdConfig,
}

pub async fn get_analysis_export_anomaly_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    policy_id: Option<&str>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisExportAnomalyOverrides,
) -> Result<GatewayAnalysisExportAnomalyReportView, GatewayError> {
    let context =
        resolve_analysis_export_anomaly_context(pool, filters, policy_id, profile_key, overrides)
            .await?;
    let trend_report = get_analysis_export_trend_report(pool, &context.filters).await?;
    Ok(build_analysis_export_anomaly_report(
        trend_report,
        context.profile_key,
        context.thresholds,
    ))
}

async fn resolve_analysis_export_anomaly_context(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    policy_id: Option<&str>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisExportAnomalyOverrides,
) -> Result<AnalysisExportAnomalyContext, GatewayError> {
    let policy = if let Some(policy_id) = trimmed_owned_ref_opt(policy_id) {
        let policies = list_anomaly_policies(
            pool,
            &GatewayAnalysisAnomalyPolicyFilters {
                policy_id: Some(policy_id.to_string()),
                limit: Some(1),
                ..GatewayAnalysisAnomalyPolicyFilters::default()
            },
        )
        .await?;
        let Some(policy) = policies.into_iter().next() else {
            return Err(GatewayError::not_found(
                "Gateway analysis anomaly policy 不存在。",
            ));
        };
        Some(policy)
    } else {
        None
    };

    if let (Some(request_project_id), Some(policy)) = (
        trimmed_owned_ref_opt(filters.project_id.as_deref()),
        policy.as_ref(),
    ) {
        if let Some(policy_project_id) = trimmed_owned_ref_opt(policy.project_id.as_deref()) {
            if request_project_id != policy_project_id {
                return Err(GatewayError::conflict(
                    "filters.projectId 与 anomaly policy 绑定的 project 不一致。",
                ));
            }
        }
    }

    let policy_profile_key = policy.as_ref().map(|item| item.profile_key.as_str());
    let resolved_profile_key =
        normalize_analysis_export_profile_key(profile_key.or(policy_profile_key)).to_string();
    let policy_thresholds = policy.as_ref().and_then(|item| {
        serde_json::from_value::<GatewayAnalysisExportAnomalyThresholdConfig>(
            item.thresholds.clone(),
        )
        .ok()
    });
    let thresholds = build_analysis_export_anomaly_threshold_config(
        &resolved_profile_key,
        policy_thresholds,
        &overrides,
    )?;

    Ok(AnalysisExportAnomalyContext {
        filters: GatewayPersistedAnalysisExportFilters {
            export_id: None,
            label: filters.label.clone(),
            tag: filters
                .tag
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.tag.clone())),
            project_id: filters
                .project_id
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.project_id.clone())),
            status: Some(
                trimmed_owned_ref_opt(filters.status.as_deref())
                    .unwrap_or("active")
                    .to_string(),
            ),
            text_mode: filters
                .text_mode
                .clone()
                .or_else(|| policy.as_ref().and_then(|item| item.text_mode.clone())),
            created_from: filters.created_from.clone(),
            created_to: filters.created_to.clone(),
            limit: filters.limit,
        },
        profile_key: resolved_profile_key,
        thresholds,
    })
}

fn build_analysis_export_anomaly_report(
    trend_report: GatewayAnalysisExportTrendReportView,
    profile_key: String,
    thresholds: GatewayAnalysisExportAnomalyThresholdConfig,
) -> GatewayAnalysisExportAnomalyReportView {
    let mut anomalies = Vec::new();
    let latest = trend_report.points.first();
    let latest_export_id = trend_report
        .summary
        .as_ref()
        .and_then(|summary| summary.latest_export_id.clone())
        .or_else(|| latest.map(|point| point.export.export_id.clone()));
    let previous_export_id = trend_report
        .summary
        .as_ref()
        .and_then(|summary| summary.previous_export_id.clone())
        .or_else(|| {
            trend_report
                .points
                .get(1)
                .map(|point| point.export.export_id.clone())
        });

    if latest.is_some_and(|point| !point.dataset_available) {
        anomalies.push(GatewayAnalysisExportAnomalyView {
            code: "latest_dataset_missing".to_string(),
            severity: "critical".to_string(),
            message: "最新一批 export 的 dataset.jsonl 不可用，趋势与基线分析已经失真。"
                .to_string(),
            latest_export_id: latest_export_id.clone(),
            previous_export_id: previous_export_id.clone(),
            latest_value: None,
            previous_value: None,
            delta_value: None,
            delta_ratio: None,
            threshold_value: None,
        });
    }

    if let Some(summary) = trend_report.summary.as_ref() {
        if summary.failure_rate.latest_value.unwrap_or(0.0)
            >= thresholds.failure_rate_warning_threshold
            || summary.failure_rate.delta_ratio.unwrap_or(0.0)
                >= thresholds.failure_rate_delta_ratio_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "failure_rate_spike",
                "失败率相对基线显著升高。",
                if summary.failure_rate.latest_value.unwrap_or(0.0)
                    >= thresholds.failure_rate_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.failure_rate,
                Some(thresholds.failure_rate_warning_threshold),
            );
        }
        if summary.completion_rate.delta_value.unwrap_or(0.0)
            <= thresholds.completion_rate_delta_value_threshold
            || summary.completion_rate.latest_value.unwrap_or(1.0)
                <= thresholds.completion_rate_warning_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "completion_rate_drop",
                "完成率相对基线明显下降。",
                if summary.completion_rate.latest_value.unwrap_or(1.0)
                    <= thresholds.completion_rate_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.completion_rate,
                Some(thresholds.completion_rate_warning_threshold),
            );
        }
        if summary
            .response_artifact_coverage
            .latest_value
            .unwrap_or(1.0)
            <= thresholds.response_artifact_coverage_warning_threshold
            || summary
                .response_artifact_coverage
                .delta_value
                .unwrap_or(0.0)
                <= thresholds.response_artifact_coverage_delta_value_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "response_artifact_coverage_drop",
                "response artifact 覆盖率低于安全阈值或相对基线明显下降。",
                if summary
                    .response_artifact_coverage
                    .latest_value
                    .unwrap_or(1.0)
                    <= thresholds.response_artifact_coverage_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.response_artifact_coverage,
                Some(thresholds.response_artifact_coverage_warning_threshold),
            );
        }
        if summary
            .request_artifact_coverage
            .latest_value
            .unwrap_or(1.0)
            <= thresholds.request_artifact_coverage_warning_threshold
            || summary.request_artifact_coverage.delta_value.unwrap_or(0.0)
                <= thresholds.request_artifact_coverage_delta_value_threshold
        {
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "request_artifact_coverage_drop",
                "request artifact 覆盖率低于安全阈值或相对基线明显下降。",
                if summary
                    .request_artifact_coverage
                    .latest_value
                    .unwrap_or(1.0)
                    <= thresholds.request_artifact_coverage_critical_threshold
                {
                    "critical"
                } else {
                    "warning"
                },
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.request_artifact_coverage,
                Some(thresholds.request_artifact_coverage_warning_threshold),
            );
        }
        if summary.total_tokens_per_sample.delta_ratio.unwrap_or(0.0)
            >= thresholds.tokens_per_sample_warning_delta_ratio_threshold
            || summary.total_tokens_per_sample.latest_value.unwrap_or(0.0)
                >= thresholds.tokens_per_sample_critical_absolute_threshold
        {
            let severity = if summary.total_tokens_per_sample.delta_ratio.unwrap_or(0.0)
                >= thresholds.tokens_per_sample_critical_delta_ratio_threshold
                || summary.total_tokens_per_sample.latest_value.unwrap_or(0.0)
                    >= thresholds.tokens_per_sample_critical_absolute_threshold
            {
                "critical"
            } else {
                "warning"
            };
            push_analysis_export_metric_anomaly(
                &mut anomalies,
                "tokens_per_sample_spike",
                "单样本 token 成本相对基线明显上升。",
                severity,
                latest_export_id.clone(),
                previous_export_id.clone(),
                &summary.total_tokens_per_sample,
                Some(thresholds.tokens_per_sample_warning_delta_ratio_threshold),
            );
        }
    }

    let mut by_severity = HashMap::new();
    let mut by_code = HashMap::new();
    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, &anomaly.severity);
        accumulate_bucket(&mut by_code, &anomaly.code);
    }
    GatewayAnalysisExportAnomalyReportView {
        generated_at: trend_report.generated_at.clone(),
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        latest_export: latest.map(|point| point.export.clone()),
        previous_export: trend_report.points.get(1).map(|point| point.export.clone()),
        trend_summary: trend_report.summary,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

fn push_analysis_export_metric_anomaly(
    anomalies: &mut Vec<GatewayAnalysisExportAnomalyView>,
    code: &str,
    message: &str,
    severity: &str,
    latest_export_id: Option<String>,
    previous_export_id: Option<String>,
    metric: &GatewayAnalysisExportTrendMetricSummaryView,
    threshold_value: Option<f64>,
) {
    anomalies.push(GatewayAnalysisExportAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_export_id,
        previous_export_id,
        latest_value: metric.latest_value,
        previous_value: metric.previous_value,
        delta_value: metric.delta_value,
        delta_ratio: metric.delta_ratio,
        threshold_value,
    });
}
