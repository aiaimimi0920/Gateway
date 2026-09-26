use super::*;

pub(crate) fn build_rate_limit_hotspot_anomaly_thresholds(
    profile_key: &str,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> GatewayRateLimitHotspotAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 25.0,
            total_rate_limited_requests_critical_threshold: 60.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.5,
            top_code_share_warning_threshold: 0.55,
            top_code_share_critical_threshold: 0.7,
            top_project_share_warning_threshold: 0.45,
            top_project_share_critical_threshold: 0.6,
            top_api_key_share_warning_threshold: 0.4,
            top_api_key_share_critical_threshold: 0.55,
            top_requested_model_share_warning_threshold: 0.45,
            top_requested_model_share_critical_threshold: 0.6,
            top_endpoint_share_warning_threshold: 0.6,
            top_endpoint_share_critical_threshold: 0.8,
        },
        "aggressive" => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 8.0,
            total_rate_limited_requests_critical_threshold: 20.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.2,
            top_code_share_warning_threshold: 0.35,
            top_code_share_critical_threshold: 0.5,
            top_project_share_warning_threshold: 0.3,
            top_project_share_critical_threshold: 0.45,
            top_api_key_share_warning_threshold: 0.25,
            top_api_key_share_critical_threshold: 0.4,
            top_requested_model_share_warning_threshold: 0.3,
            top_requested_model_share_critical_threshold: 0.45,
            top_endpoint_share_warning_threshold: 0.45,
            top_endpoint_share_critical_threshold: 0.65,
        },
        _ => GatewayRateLimitHotspotAnomalyThresholdConfig {
            total_rate_limited_requests_warning_threshold: 15.0,
            total_rate_limited_requests_critical_threshold: 35.0,
            total_rate_limited_requests_delta_ratio_threshold: 0.35,
            top_code_share_warning_threshold: 0.45,
            top_code_share_critical_threshold: 0.6,
            top_project_share_warning_threshold: 0.35,
            top_project_share_critical_threshold: 0.5,
            top_api_key_share_warning_threshold: 0.3,
            top_api_key_share_critical_threshold: 0.45,
            top_requested_model_share_warning_threshold: 0.35,
            top_requested_model_share_critical_threshold: 0.5,
            top_endpoint_share_warning_threshold: 0.5,
            top_endpoint_share_critical_threshold: 0.7,
        },
    };

    GatewayRateLimitHotspotAnomalyThresholdConfig {
        total_rate_limited_requests_warning_threshold: overrides
            .total_rate_limited_requests_warning_threshold
            .unwrap_or(base.total_rate_limited_requests_warning_threshold),
        total_rate_limited_requests_critical_threshold: overrides
            .total_rate_limited_requests_critical_threshold
            .unwrap_or(base.total_rate_limited_requests_critical_threshold),
        total_rate_limited_requests_delta_ratio_threshold: overrides
            .total_rate_limited_requests_delta_ratio_threshold
            .unwrap_or(base.total_rate_limited_requests_delta_ratio_threshold),
        top_code_share_warning_threshold: overrides
            .top_code_share_warning_threshold
            .unwrap_or(base.top_code_share_warning_threshold),
        top_code_share_critical_threshold: overrides
            .top_code_share_critical_threshold
            .unwrap_or(base.top_code_share_critical_threshold),
        top_project_share_warning_threshold: overrides
            .top_project_share_warning_threshold
            .unwrap_or(base.top_project_share_warning_threshold),
        top_project_share_critical_threshold: overrides
            .top_project_share_critical_threshold
            .unwrap_or(base.top_project_share_critical_threshold),
        top_api_key_share_warning_threshold: overrides
            .top_api_key_share_warning_threshold
            .unwrap_or(base.top_api_key_share_warning_threshold),
        top_api_key_share_critical_threshold: overrides
            .top_api_key_share_critical_threshold
            .unwrap_or(base.top_api_key_share_critical_threshold),
        top_requested_model_share_warning_threshold: overrides
            .top_requested_model_share_warning_threshold
            .unwrap_or(base.top_requested_model_share_warning_threshold),
        top_requested_model_share_critical_threshold: overrides
            .top_requested_model_share_critical_threshold
            .unwrap_or(base.top_requested_model_share_critical_threshold),
        top_endpoint_share_warning_threshold: overrides
            .top_endpoint_share_warning_threshold
            .unwrap_or(base.top_endpoint_share_warning_threshold),
        top_endpoint_share_critical_threshold: overrides
            .top_endpoint_share_critical_threshold
            .unwrap_or(base.top_endpoint_share_critical_threshold),
    }
}

pub(crate) fn build_rate_limit_hotspot_anomaly_report(
    generated_at: String,
    trend_report: GatewayRateLimitHotspotTrendReportView,
    profile_key: String,
    thresholds: GatewayRateLimitHotspotAnomalyThresholdConfig,
) -> GatewayRateLimitHotspotAnomalyReportView {
    let mut anomalies = Vec::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    let trend_summary = trend_report.summary.clone();
    let latest_point = trend_report.points.first().cloned();
    let previous_point = trend_report.points.get(1).cloned();

    if let Some(summary) = trend_summary.as_ref() {
        let total_latest = summary
            .total_rate_limited_requests
            .latest_value
            .unwrap_or(0.0);
        let total_delta_ok = summary
            .total_rate_limited_requests
            .delta_ratio
            .map_or(true, |value| {
                value >= thresholds.total_rate_limited_requests_delta_ratio_threshold
            });
        let total_critical = total_latest
            >= thresholds.total_rate_limited_requests_critical_threshold
            && total_delta_ok;
        let total_warning = total_latest
            >= thresholds.total_rate_limited_requests_warning_threshold
            && total_delta_ok;
        if total_warning {
            anomalies.push(GatewayRateLimitHotspotAnomalyView {
                code: "rate_limit_request_spike".to_string(),
                severity: if total_critical {
                    "critical"
                } else {
                    "warning"
                }
                .to_string(),
                message: "当前时间桶内的 rate-limit 请求量明显升高，热点流量正在打穿当前限流策略。"
                    .to_string(),
                entity_key: None,
                latest_bucket_start_at: summary.latest_bucket_start_at.clone(),
                previous_bucket_start_at: summary.previous_bucket_start_at.clone(),
                latest_value: summary.total_rate_limited_requests.latest_value,
                previous_value: summary.total_rate_limited_requests.previous_value,
                delta_value: summary.total_rate_limited_requests.delta_value,
                delta_ratio: summary.total_rate_limited_requests.delta_ratio,
                threshold_value: thresholds.total_rate_limited_requests_warning_threshold,
            });
        }

        let checks = vec![
            (
                "rate_limit_code_concentration",
                summary.top_code_share.clone(),
                thresholds.top_code_share_warning_threshold,
                thresholds.top_code_share_critical_threshold,
                summary.latest_top_code_key.clone(),
                "单一 rate-limit 错误码占比过高，说明当前热点已经集中在同一类限流路径。"
                    .to_string(),
            ),
            (
                "rate_limit_project_hotspot",
                summary.top_project_share.clone(),
                thresholds.top_project_share_warning_threshold,
                thresholds.top_project_share_critical_threshold,
                summary.latest_top_project_key.clone(),
                "单一 project 的 rate-limit 占比过高，当前热点已经明显集中在特定 project。"
                    .to_string(),
            ),
            (
                "rate_limit_api_key_hotspot",
                summary.top_api_key_share.clone(),
                thresholds.top_api_key_share_warning_threshold,
                thresholds.top_api_key_share_critical_threshold,
                summary.latest_top_api_key_key.clone(),
                "单一 API key 的 rate-limit 占比过高，当前热点已经集中到单个 key。".to_string(),
            ),
            (
                "rate_limit_model_hotspot",
                summary.top_requested_model_share.clone(),
                thresholds.top_requested_model_share_warning_threshold,
                thresholds.top_requested_model_share_critical_threshold,
                summary.latest_top_requested_model_key.clone(),
                "单一模型的 rate-limit 占比过高，当前热点已经集中在同一个模型请求面。".to_string(),
            ),
            (
                "rate_limit_endpoint_hotspot",
                summary.top_endpoint_share.clone(),
                thresholds.top_endpoint_share_warning_threshold,
                thresholds.top_endpoint_share_critical_threshold,
                summary.latest_top_endpoint_key.clone(),
                "单一 endpoint 的 rate-limit 占比过高，当前热点已经集中在同一条公开调用口径。"
                    .to_string(),
            ),
        ];

        for (code, metric, warning_threshold, critical_threshold, entity_key, message) in checks {
            if metric.latest_value.unwrap_or(0.0) < warning_threshold {
                continue;
            }
            anomalies.push(GatewayRateLimitHotspotAnomalyView {
                code: code.to_string(),
                severity: if metric.latest_value.unwrap_or(0.0) >= critical_threshold {
                    "critical".to_string()
                } else {
                    "warning".to_string()
                },
                message,
                entity_key,
                latest_bucket_start_at: summary.latest_bucket_start_at.clone(),
                previous_bucket_start_at: summary.previous_bucket_start_at.clone(),
                latest_value: metric.latest_value,
                previous_value: metric.previous_value,
                delta_value: metric.delta_value,
                delta_ratio: metric.delta_ratio,
                threshold_value: warning_threshold,
            });
        }
    }

    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }

    GatewayRateLimitHotspotAnomalyReportView {
        generated_at,
        filters: trend_report.filters.clone(),
        profile_key,
        thresholds,
        trend_summary,
        latest_point,
        previous_point,
        anomalies,
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
    }
}

pub(super) fn normalize_profile_key(value: Option<&str>) -> String {
    let normalized = value.unwrap_or("balanced").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "balanced" | "aggressive" | "conservative" => normalized,
        _ => "balanced".to_string(),
    }
}
