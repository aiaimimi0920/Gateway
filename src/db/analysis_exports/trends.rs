use super::dataset::read_analysis_export_dataset_for_export;
use super::*;

pub(super) async fn build_analysis_export_trend_point(
    export: &GatewayPersistedAnalysisExportView,
) -> GatewayAnalysisExportTrendPointView {
    let rows = match read_analysis_export_dataset_for_export(export).await {
        Ok(rows) => rows,
        Err(_) => {
            return GatewayAnalysisExportTrendPointView {
                export: export.clone(),
                dataset_available: false,
                dataset_unavailable_reason: Some("dataset_missing".to_string()),
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                stream_samples: None,
                completed_samples: None,
                failed_samples: None,
                cancelled_samples: None,
                tool_request_samples: None,
                tool_response_samples: None,
                system_prompt_samples: None,
                reasoning_samples: None,
                metadata_samples: None,
                explicit_session_samples: None,
                previous_response_samples: None,
            };
        }
    };

    let mut prompt_tokens = 0i64;
    let mut completion_tokens = 0i64;
    let mut total_tokens = 0i64;
    let mut stream_samples = 0i64;
    let mut completed_samples = 0i64;
    let mut failed_samples = 0i64;
    let mut cancelled_samples = 0i64;
    let mut tool_request_samples = 0i64;
    let mut tool_response_samples = 0i64;
    let mut system_prompt_samples = 0i64;
    let mut reasoning_samples = 0i64;
    let mut metadata_samples = 0i64;
    let mut explicit_session_samples = 0i64;
    let mut previous_response_samples = 0i64;

    for row in &rows {
        prompt_tokens += i64::from(row.prompt_tokens.unwrap_or_default());
        completion_tokens += i64::from(row.completion_tokens.unwrap_or_default());
        total_tokens += i64::from(row.total_tokens.unwrap_or_default());
        if row.stream {
            stream_samples += 1;
        }
        match row.status.as_str() {
            "completed" => completed_samples += 1,
            "failed" => failed_samples += 1,
            "cancelled" => cancelled_samples += 1,
            _ => {}
        }
        let request_tool_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("requestToolCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let request_historical_tool_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("requestHistoricalToolCallCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if request_tool_count > 0 || request_historical_tool_count > 0 {
            tool_request_samples += 1;
        }
        let response_tool_call_count = row
            .analysis_profile
            .as_ref()
            .and_then(|value| value.get("responseToolCallCount"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if response_tool_call_count > 0 {
            tool_response_samples += 1;
        }
        if analysis_profile_flag(row, "hasSystemPrompt") {
            system_prompt_samples += 1;
        }
        if analysis_profile_flag(row, "hasReasoning") {
            reasoning_samples += 1;
        }
        if analysis_profile_flag(row, "hasMetadata") {
            metadata_samples += 1;
        }
        if analysis_profile_flag(row, "hasExplicitSessionKey") {
            explicit_session_samples += 1;
        }
        if analysis_profile_flag(row, "hasPreviousResponse") {
            previous_response_samples += 1;
        }
    }

    GatewayAnalysisExportTrendPointView {
        export: export.clone(),
        dataset_available: true,
        dataset_unavailable_reason: None,
        prompt_tokens: Some(prompt_tokens),
        completion_tokens: Some(completion_tokens),
        total_tokens: Some(total_tokens),
        stream_samples: Some(stream_samples),
        completed_samples: Some(completed_samples),
        failed_samples: Some(failed_samples),
        cancelled_samples: Some(cancelled_samples),
        tool_request_samples: Some(tool_request_samples),
        tool_response_samples: Some(tool_response_samples),
        system_prompt_samples: Some(system_prompt_samples),
        reasoning_samples: Some(reasoning_samples),
        metadata_samples: Some(metadata_samples),
        explicit_session_samples: Some(explicit_session_samples),
        previous_response_samples: Some(previous_response_samples),
    }
}

fn analysis_profile_flag(row: &GatewayAnalysisExportRowView, field: &str) -> bool {
    row.analysis_profile
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn build_analysis_export_trend_summary(
    points: &[GatewayAnalysisExportTrendPointView],
) -> Option<GatewayAnalysisExportTrendSummaryView> {
    let latest = points.first()?;
    let previous = points.get(1);
    Some(GatewayAnalysisExportTrendSummaryView {
        latest_export_id: Some(latest.export.export_id.clone()),
        previous_export_id: previous.map(|item| item.export.export_id.clone()),
        prompt_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.prompt_tokens, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.prompt_tokens, item.export.sample_count)),
        ),
        completion_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.completion_tokens, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.completion_tokens, item.export.sample_count)),
        ),
        total_tokens_per_sample: build_trend_metric_summary(
            safe_ratio_i64(latest.total_tokens, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.total_tokens, item.export.sample_count)),
        ),
        request_artifact_coverage: build_trend_metric_summary(
            safe_ratio_usize(
                Some(latest.export.request_artifact_count),
                latest.export.sample_count,
            ),
            previous.and_then(|item| {
                safe_ratio_usize(
                    Some(item.export.request_artifact_count),
                    item.export.sample_count,
                )
            }),
        ),
        response_artifact_coverage: build_trend_metric_summary(
            safe_ratio_usize(
                Some(latest.export.response_artifact_count),
                latest.export.sample_count,
            ),
            previous.and_then(|item| {
                safe_ratio_usize(
                    Some(item.export.response_artifact_count),
                    item.export.sample_count,
                )
            }),
        ),
        stream_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.stream_samples, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.stream_samples, item.export.sample_count)),
        ),
        completion_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.completed_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.completed_samples, item.export.sample_count)),
        ),
        failure_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.failed_samples, latest.export.sample_count),
            previous.and_then(|item| safe_ratio_i64(item.failed_samples, item.export.sample_count)),
        ),
        cancellation_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.cancelled_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.cancelled_samples, item.export.sample_count)),
        ),
        tool_request_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.tool_request_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.tool_request_samples, item.export.sample_count)
            }),
        ),
        tool_response_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.tool_response_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.tool_response_samples, item.export.sample_count)
            }),
        ),
        reasoning_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.reasoning_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.reasoning_samples, item.export.sample_count)),
        ),
        metadata_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.metadata_samples, latest.export.sample_count),
            previous
                .and_then(|item| safe_ratio_i64(item.metadata_samples, item.export.sample_count)),
        ),
        explicit_session_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.explicit_session_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.explicit_session_samples, item.export.sample_count)
            }),
        ),
        previous_response_rate: build_trend_metric_summary(
            safe_ratio_i64(latest.previous_response_samples, latest.export.sample_count),
            previous.and_then(|item| {
                safe_ratio_i64(item.previous_response_samples, item.export.sample_count)
            }),
        ),
    })
}

fn build_trend_metric_summary(
    latest_value: Option<f64>,
    previous_value: Option<f64>,
) -> GatewayAnalysisExportTrendMetricSummaryView {
    let delta_value = match (latest_value, previous_value) {
        (Some(latest), Some(previous)) => Some(latest - previous),
        _ => None,
    };
    let delta_ratio = match (latest_value, previous_value, delta_value) {
        (Some(_), Some(previous), Some(delta_value)) if previous != 0.0 => {
            Some(delta_value / previous)
        }
        _ => None,
    };
    GatewayAnalysisExportTrendMetricSummaryView {
        latest_value,
        previous_value,
        delta_value,
        delta_ratio,
    }
}

fn safe_ratio_i64(numerator: Option<i64>, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    numerator.map(|value| value as f64 / denominator as f64)
}

fn safe_ratio_usize(numerator: Option<usize>, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }
    numerator.map(|value| value as f64 / denominator as f64)
}
