use super::normalization::trimmed_owned_ref;
use super::*;

pub(super) fn normalize_analysis_export_profile_key(value: Option<&str>) -> &'static str {
    match value
        .and_then(trimmed_owned_ref)
        .map(|value| value.to_lowercase())
    {
        Some(value) if value == "conservative" => "conservative",
        Some(value) if value == "aggressive" => "aggressive",
        _ => "balanced",
    }
}

pub(super) fn build_analysis_export_anomaly_threshold_config(
    profile_key: &str,
    policy_thresholds: Option<GatewayAnalysisExportAnomalyThresholdConfig>,
    overrides: &GatewayAnalysisExportAnomalyOverrides,
) -> Result<GatewayAnalysisExportAnomalyThresholdConfig, GatewayError> {
    let mut thresholds = policy_thresholds.unwrap_or_else(|| match profile_key {
        "conservative" => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.2,
            failure_rate_critical_threshold: 0.3,
            failure_rate_delta_ratio_threshold: 0.8,
            completion_rate_warning_threshold: 0.7,
            completion_rate_critical_threshold: 0.55,
            completion_rate_delta_value_threshold: -0.15,
            response_artifact_coverage_warning_threshold: 0.75,
            response_artifact_coverage_critical_threshold: 0.55,
            response_artifact_coverage_delta_value_threshold: -0.15,
            request_artifact_coverage_warning_threshold: 0.8,
            request_artifact_coverage_critical_threshold: 0.6,
            request_artifact_coverage_delta_value_threshold: -0.15,
            tokens_per_sample_warning_delta_ratio_threshold: 0.5,
            tokens_per_sample_critical_delta_ratio_threshold: 1.0,
            tokens_per_sample_critical_absolute_threshold: 2500.0,
        },
        "aggressive" => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.12,
            failure_rate_critical_threshold: 0.2,
            failure_rate_delta_ratio_threshold: 0.35,
            completion_rate_warning_threshold: 0.8,
            completion_rate_critical_threshold: 0.7,
            completion_rate_delta_value_threshold: -0.08,
            response_artifact_coverage_warning_threshold: 0.85,
            response_artifact_coverage_critical_threshold: 0.7,
            response_artifact_coverage_delta_value_threshold: -0.08,
            request_artifact_coverage_warning_threshold: 0.9,
            request_artifact_coverage_critical_threshold: 0.75,
            request_artifact_coverage_delta_value_threshold: -0.08,
            tokens_per_sample_warning_delta_ratio_threshold: 0.25,
            tokens_per_sample_critical_delta_ratio_threshold: 0.6,
            tokens_per_sample_critical_absolute_threshold: 1800.0,
        },
        _ => GatewayAnalysisExportAnomalyThresholdConfig {
            failure_rate_warning_threshold: 0.15,
            failure_rate_critical_threshold: 0.25,
            failure_rate_delta_ratio_threshold: 0.5,
            completion_rate_warning_threshold: 0.75,
            completion_rate_critical_threshold: 0.6,
            completion_rate_delta_value_threshold: -0.1,
            response_artifact_coverage_warning_threshold: 0.8,
            response_artifact_coverage_critical_threshold: 0.6,
            response_artifact_coverage_delta_value_threshold: -0.1,
            request_artifact_coverage_warning_threshold: 0.85,
            request_artifact_coverage_critical_threshold: 0.65,
            request_artifact_coverage_delta_value_threshold: -0.1,
            tokens_per_sample_warning_delta_ratio_threshold: 0.35,
            tokens_per_sample_critical_delta_ratio_threshold: 0.8,
            tokens_per_sample_critical_absolute_threshold: 2000.0,
        },
    });
    apply_analysis_export_anomaly_overrides(&mut thresholds, overrides)?;
    Ok(thresholds)
}

fn apply_analysis_export_anomaly_overrides(
    thresholds: &mut GatewayAnalysisExportAnomalyThresholdConfig,
    overrides: &GatewayAnalysisExportAnomalyOverrides,
) -> Result<(), GatewayError> {
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_warning_threshold,
        "failureRateWarningThreshold",
    )? {
        thresholds.failure_rate_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_critical_threshold,
        "failureRateCriticalThreshold",
    )? {
        thresholds.failure_rate_critical_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.failure_rate_delta_ratio_threshold,
        "failureRateDeltaRatioThreshold",
    )? {
        thresholds.failure_rate_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.completion_rate_warning_threshold,
        "completionRateWarningThreshold",
    )? {
        thresholds.completion_rate_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.completion_rate_critical_threshold,
        "completionRateCriticalThreshold",
    )? {
        thresholds.completion_rate_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.completion_rate_delta_value_threshold,
        "completionRateDeltaValueThreshold",
    )? {
        thresholds.completion_rate_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.response_artifact_coverage_warning_threshold,
        "responseArtifactCoverageWarningThreshold",
    )? {
        thresholds.response_artifact_coverage_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.response_artifact_coverage_critical_threshold,
        "responseArtifactCoverageCriticalThreshold",
    )? {
        thresholds.response_artifact_coverage_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.response_artifact_coverage_delta_value_threshold,
        "responseArtifactCoverageDeltaValueThreshold",
    )? {
        thresholds.response_artifact_coverage_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.request_artifact_coverage_warning_threshold,
        "requestArtifactCoverageWarningThreshold",
    )? {
        thresholds.request_artifact_coverage_warning_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.request_artifact_coverage_critical_threshold,
        "requestArtifactCoverageCriticalThreshold",
    )? {
        thresholds.request_artifact_coverage_critical_threshold = value;
    }
    if let Some(value) = normalize_signed_override(
        overrides.request_artifact_coverage_delta_value_threshold,
        "requestArtifactCoverageDeltaValueThreshold",
    )? {
        thresholds.request_artifact_coverage_delta_value_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_warning_delta_ratio_threshold,
        "tokensPerSampleWarningDeltaRatioThreshold",
    )? {
        thresholds.tokens_per_sample_warning_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_critical_delta_ratio_threshold,
        "tokensPerSampleCriticalDeltaRatioThreshold",
    )? {
        thresholds.tokens_per_sample_critical_delta_ratio_threshold = value;
    }
    if let Some(value) = normalize_non_negative_override(
        overrides.tokens_per_sample_critical_absolute_threshold,
        "tokensPerSampleCriticalAbsoluteThreshold",
    )? {
        thresholds.tokens_per_sample_critical_absolute_threshold = value;
    }
    Ok(())
}

fn normalize_non_negative_override(
    value: Option<f64>,
    field_name: &str,
) -> Result<Option<f64>, GatewayError> {
    match value {
        Some(value) if !value.is_finite() => Err(GatewayError::conflict(format!(
            "{field_name} 必须是合法数字。"
        ))),
        Some(value) if value < 0.0 => {
            Err(GatewayError::conflict(format!("{field_name} 不能小于 0。")))
        }
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn normalize_signed_override(
    value: Option<f64>,
    field_name: &str,
) -> Result<Option<f64>, GatewayError> {
    match value {
        Some(value) if !value.is_finite() => Err(GatewayError::conflict(format!(
            "{field_name} 必须是合法数字。"
        ))),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}
