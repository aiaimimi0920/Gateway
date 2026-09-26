use super::*;

pub async fn summarize_provider_routing_analysis(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayProviderRoutingAnalysisSummaryView, GatewayError> {
    let rows = list_analysis_samples(pool, filters).await?;
    Ok(build_provider_routing_summary(&rows))
}

pub async fn get_provider_routing_anomaly_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    profile_key: Option<&str>,
    overrides: GatewayProviderRoutingAnalysisAnomalyOverrides,
) -> Result<GatewayProviderRoutingAnalysisAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_provider_routing_profile_key(profile_key);
    let thresholds = build_provider_routing_anomaly_thresholds(&normalized_profile_key, overrides);
    let summary = summarize_provider_routing_analysis(pool, filters).await?;
    Ok(build_provider_routing_anomaly_report(
        filters,
        &normalized_profile_key,
        thresholds,
        summary,
    ))
}

pub(super) fn build_provider_routing_summary(
    rows: &[GatewayAnalysisSampleView],
) -> GatewayProviderRoutingAnalysisSummaryView {
    let mut by_selected_provider = BTreeMap::new();
    let mut by_degradation_reason = BTreeMap::new();
    let mut selected_provider_samples = 0;
    let mut degraded_selected_provider_samples = 0;
    let mut saturated_selected_provider_samples = 0;
    let mut breaker_open_selected_provider_samples = 0;

    let mut routing_scores = Vec::new();
    let mut health_weights = Vec::new();
    let mut capacity_weights = Vec::new();

    for row in rows {
        let Some(selected) = route_trace_selected_candidate(row.route_trace.as_ref()) else {
            continue;
        };

        selected_provider_samples += 1;
        accumulate_bucket(
            &mut by_selected_provider,
            selected.get("providerAccountId").and_then(Value::as_str),
        );

        let routing_score = route_trace_selected_candidate_number(selected, "routingScore");
        let health_weight = route_trace_selected_candidate_number(selected, "healthWeight");
        let capacity_weight = route_trace_selected_candidate_number(selected, "capacityWeight");

        routing_scores.push(routing_score);
        health_weights.push(health_weight);
        capacity_weights.push(capacity_weight);

        if selected
            .get("degraded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            degraded_selected_provider_samples += 1;
        }
        if selected
            .get("breakerOpen")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            breaker_open_selected_provider_samples += 1;
        }
        if capacity_weight.unwrap_or(1.0) <= 0.0 {
            saturated_selected_provider_samples += 1;
        }
        if let Some(reasons) = selected.get("degradationReasons").and_then(Value::as_array) {
            for reason in reasons.iter().filter_map(Value::as_str) {
                accumulate_bucket(&mut by_degradation_reason, Some(reason));
            }
        }
    }

    GatewayProviderRoutingAnalysisSummaryView {
        total_samples: rows.len(),
        selected_provider_samples,
        degraded_selected_provider_samples,
        saturated_selected_provider_samples,
        breaker_open_selected_provider_samples,
        routing_score: build_ts_distribution(&routing_scores),
        health_weight: build_ts_distribution(&health_weights),
        capacity_weight: build_ts_distribution(&capacity_weights),
        by_selected_provider: into_keyed_summary_buckets(by_selected_provider),
        by_degradation_reason: into_keyed_summary_buckets(by_degradation_reason),
    }
}

pub(super) fn build_provider_routing_anomaly_report(
    filters: &RequestAuditFilters,
    profile_key: &str,
    thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
    summary: GatewayProviderRoutingAnalysisSummaryView,
) -> GatewayProviderRoutingAnalysisAnomalyReportView {
    let mut anomalies = Vec::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();

    let selected_samples = summary.selected_provider_samples.max(1) as f64;
    let degraded_rate = summary.degraded_selected_provider_samples as f64 / selected_samples;
    let saturated_rate = summary.saturated_selected_provider_samples as f64 / selected_samples;
    let breaker_open_rate =
        summary.breaker_open_selected_provider_samples as f64 / selected_samples;
    let routing_score_avg = summary.routing_score.avg;

    if let Some(routing_score_avg) = routing_score_avg {
        if routing_score_avg <= thresholds.routing_score_warning_threshold {
            push_provider_routing_anomaly(
                &mut anomalies,
                "provider_routing_score_drop",
                if routing_score_avg <= thresholds.routing_score_critical_threshold {
                    "critical"
                } else {
                    "warning"
                },
                "当前窗口内被选中 provider 的平均 routing score 明显偏低，说明选路正在持续踩到退化节点。",
                Some(routing_score_avg),
                Some(thresholds.routing_score_warning_threshold),
            );
        }
    }

    if degraded_rate >= thresholds.degraded_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "degraded_provider_route_spike",
            if degraded_rate >= thresholds.degraded_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内命中 degraded provider 的请求占比过高。",
            Some(round_metric(degraded_rate, 3)),
            Some(thresholds.degraded_route_warning_threshold),
        );
    }

    if saturated_rate >= thresholds.saturated_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "saturated_provider_route_spike",
            if saturated_rate >= thresholds.saturated_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内命中并发饱和 provider 的请求占比过高。",
            Some(round_metric(saturated_rate, 3)),
            Some(thresholds.saturated_route_warning_threshold),
        );
    }

    if breaker_open_rate >= thresholds.breaker_open_route_warning_threshold {
        push_provider_routing_anomaly(
            &mut anomalies,
            "breaker_open_provider_route_detected",
            if breaker_open_rate >= thresholds.breaker_open_route_critical_threshold {
                "critical"
            } else {
                "warning"
            },
            "当前窗口内仍有请求命中 breaker-open provider，说明 routing 健康退避仍存在裂口。",
            Some(round_metric(breaker_open_rate, 3)),
            Some(thresholds.breaker_open_route_warning_threshold),
        );
    }

    for anomaly in &anomalies {
        accumulate_bucket(&mut by_severity, Some(anomaly.severity.as_str()));
        accumulate_bucket(&mut by_code, Some(anomaly.code.as_str()));
    }

    GatewayProviderRoutingAnalysisAnomalyReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_provider_routing_filter_view(filters),
        profile_key: profile_key.to_string(),
        thresholds,
        summary,
        anomalies,
        by_severity: into_keyed_summary_buckets(by_severity),
        by_code: into_keyed_summary_buckets(by_code),
    }
}

fn normalize_provider_routing_profile_key(profile_key: Option<&str>) -> String {
    match non_empty(profile_key) {
        Some("conservative") => "conservative".to_string(),
        Some("aggressive") => "aggressive".to_string(),
        _ => "balanced".to_string(),
    }
}

pub(super) fn build_provider_routing_anomaly_thresholds(
    profile_key: &str,
    overrides: GatewayProviderRoutingAnalysisAnomalyOverrides,
) -> GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
    let base = match profile_key {
        "conservative" => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.45,
            routing_score_critical_threshold: 0.25,
            degraded_route_warning_threshold: 0.4,
            degraded_route_critical_threshold: 0.65,
            saturated_route_warning_threshold: 0.15,
            saturated_route_critical_threshold: 0.3,
            breaker_open_route_warning_threshold: 0.05,
            breaker_open_route_critical_threshold: 0.15,
        },
        "aggressive" => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.65,
            routing_score_critical_threshold: 0.45,
            degraded_route_warning_threshold: 0.2,
            degraded_route_critical_threshold: 0.4,
            saturated_route_warning_threshold: 0.05,
            saturated_route_critical_threshold: 0.15,
            breaker_open_route_warning_threshold: 0.01,
            breaker_open_route_critical_threshold: 0.05,
        },
        _ => GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
            routing_score_warning_threshold: 0.55,
            routing_score_critical_threshold: 0.35,
            degraded_route_warning_threshold: 0.3,
            degraded_route_critical_threshold: 0.55,
            saturated_route_warning_threshold: 0.1,
            saturated_route_critical_threshold: 0.2,
            breaker_open_route_warning_threshold: 0.02,
            breaker_open_route_critical_threshold: 0.08,
        },
    };

    GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
        routing_score_warning_threshold: overrides
            .routing_score_warning_threshold
            .unwrap_or(base.routing_score_warning_threshold),
        routing_score_critical_threshold: overrides
            .routing_score_critical_threshold
            .unwrap_or(base.routing_score_critical_threshold),
        degraded_route_warning_threshold: overrides
            .degraded_route_warning_threshold
            .unwrap_or(base.degraded_route_warning_threshold),
        degraded_route_critical_threshold: overrides
            .degraded_route_critical_threshold
            .unwrap_or(base.degraded_route_critical_threshold),
        saturated_route_warning_threshold: overrides
            .saturated_route_warning_threshold
            .unwrap_or(base.saturated_route_warning_threshold),
        saturated_route_critical_threshold: overrides
            .saturated_route_critical_threshold
            .unwrap_or(base.saturated_route_critical_threshold),
        breaker_open_route_warning_threshold: overrides
            .breaker_open_route_warning_threshold
            .unwrap_or(base.breaker_open_route_warning_threshold),
        breaker_open_route_critical_threshold: overrides
            .breaker_open_route_critical_threshold
            .unwrap_or(base.breaker_open_route_critical_threshold),
    }
}

fn push_provider_routing_anomaly(
    anomalies: &mut Vec<GatewayProviderRoutingAnalysisAnomalyView>,
    code: &str,
    severity: &str,
    message: &str,
    latest_value: Option<f64>,
    threshold_value: Option<f64>,
) {
    anomalies.push(GatewayProviderRoutingAnalysisAnomalyView {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        latest_value,
        previous_value: None,
        delta_value: None,
        delta_ratio: None,
        threshold_value,
    });
}

fn to_provider_routing_filter_view(
    filters: &RequestAuditFilters,
) -> GatewayProviderRoutingAnalysisFilterView {
    GatewayProviderRoutingAnalysisFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        status: filters.status.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit: filters.limit.unwrap_or(1000).clamp(1, 1000),
    }
}

fn route_trace_selected_candidate(
    route_trace: Option<&Value>,
) -> Option<&serde_json::Map<String, Value>> {
    route_trace
        .and_then(|value| value.get("selectedCandidate"))
        .and_then(Value::as_object)
}

fn route_trace_selected_candidate_number(
    selected: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<f64> {
    selected
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}
