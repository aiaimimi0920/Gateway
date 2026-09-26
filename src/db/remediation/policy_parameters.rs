use super::*;

pub(super) fn normalize_anomaly_policy_status(value: Option<&str>) -> String {
    if trimmed_owned_ref_opt(value).is_some_and(|item| item.eq_ignore_ascii_case("disabled")) {
        "disabled".to_string()
    } else {
        "enabled".to_string()
    }
}

pub(super) fn normalize_anomaly_policy_sync_status(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "ok" | "error" => Some(normalized),
        _ => None,
    }
}

pub(super) fn normalize_anomaly_profile_key(value: Option<&str>) -> String {
    let normalized = trimmed_owned_ref_opt(value)
        .unwrap_or("balanced")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "conservative" | "aggressive" | "balanced" => normalized,
        _ => "balanced".to_string(),
    }
}

pub(super) fn normalize_anomaly_severity(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "warning" | "critical" => Some(normalized),
        _ => None,
    }
}

pub(super) fn normalize_follow_up_status(value: Option<&str>) -> Option<String> {
    let normalized = trimmed_owned_ref_opt(value)?.to_ascii_lowercase();
    match normalized.as_str() {
        "pending" | "investigating" | "monitoring" | "done" => Some(normalized),
        _ => None,
    }
}

pub(super) fn normalize_non_negative_int(
    value: Option<i32>,
    fallback: Option<i32>,
    max_value: i32,
    label: &str,
) -> Result<Option<i32>, GatewayError> {
    let Some(value) = value.or(fallback) else {
        return Ok(None);
    };
    if value < 0 {
        return Err(GatewayError::bad_request(format!("{label} 必须是非负整数")));
    }
    if value > max_value {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        )));
    }
    Ok(Some(value))
}

pub(super) fn normalize_string_list(values: Option<&[String]>) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or(&[]) {
        let Some(value) = trimmed_owned_ref(value) else {
            continue;
        };
        let value = value.to_string();
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn parse_threshold_override(overrides: Option<&Value>, key: &str) -> Option<f64> {
    overrides
        .and_then(Value::as_object)
        .and_then(|items| items.get(key))
        .and_then(Value::as_f64)
}

pub(super) fn build_analysis_anomaly_threshold_config(
    profile_key: &str,
    overrides: Option<&Value>,
) -> Value {
    let base = match profile_key {
        "conservative" => json!({
            "failureRateWarningThreshold": 0.2,
            "failureRateCriticalThreshold": 0.3,
            "failureRateDeltaRatioThreshold": 0.8,
            "completionRateWarningThreshold": 0.7,
            "completionRateCriticalThreshold": 0.55,
            "completionRateDeltaValueThreshold": -0.15,
            "responseArtifactCoverageWarningThreshold": 0.75,
            "responseArtifactCoverageCriticalThreshold": 0.55,
            "responseArtifactCoverageDeltaValueThreshold": -0.15,
            "requestArtifactCoverageWarningThreshold": 0.8,
            "requestArtifactCoverageCriticalThreshold": 0.6,
            "requestArtifactCoverageDeltaValueThreshold": -0.15,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.5,
            "tokensPerSampleCriticalDeltaRatioThreshold": 1.0,
            "tokensPerSampleCriticalAbsoluteThreshold": 2500.0
        }),
        "aggressive" => json!({
            "failureRateWarningThreshold": 0.12,
            "failureRateCriticalThreshold": 0.2,
            "failureRateDeltaRatioThreshold": 0.35,
            "completionRateWarningThreshold": 0.8,
            "completionRateCriticalThreshold": 0.7,
            "completionRateDeltaValueThreshold": -0.08,
            "responseArtifactCoverageWarningThreshold": 0.85,
            "responseArtifactCoverageCriticalThreshold": 0.7,
            "responseArtifactCoverageDeltaValueThreshold": -0.08,
            "requestArtifactCoverageWarningThreshold": 0.9,
            "requestArtifactCoverageCriticalThreshold": 0.75,
            "requestArtifactCoverageDeltaValueThreshold": -0.08,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.25,
            "tokensPerSampleCriticalDeltaRatioThreshold": 0.6,
            "tokensPerSampleCriticalAbsoluteThreshold": 1800.0
        }),
        _ => json!({
            "failureRateWarningThreshold": 0.15,
            "failureRateCriticalThreshold": 0.25,
            "failureRateDeltaRatioThreshold": 0.5,
            "completionRateWarningThreshold": 0.75,
            "completionRateCriticalThreshold": 0.6,
            "completionRateDeltaValueThreshold": -0.1,
            "responseArtifactCoverageWarningThreshold": 0.8,
            "responseArtifactCoverageCriticalThreshold": 0.6,
            "responseArtifactCoverageDeltaValueThreshold": -0.1,
            "requestArtifactCoverageWarningThreshold": 0.85,
            "requestArtifactCoverageCriticalThreshold": 0.65,
            "requestArtifactCoverageDeltaValueThreshold": -0.1,
            "tokensPerSampleWarningDeltaRatioThreshold": 0.35,
            "tokensPerSampleCriticalDeltaRatioThreshold": 0.8,
            "tokensPerSampleCriticalAbsoluteThreshold": 2000.0
        }),
    };

    let mut object = base.as_object().cloned().unwrap_or_default();
    for key in [
        "failureRateWarningThreshold",
        "failureRateCriticalThreshold",
        "failureRateDeltaRatioThreshold",
        "completionRateWarningThreshold",
        "completionRateCriticalThreshold",
        "completionRateDeltaValueThreshold",
        "responseArtifactCoverageWarningThreshold",
        "responseArtifactCoverageCriticalThreshold",
        "responseArtifactCoverageDeltaValueThreshold",
        "requestArtifactCoverageWarningThreshold",
        "requestArtifactCoverageCriticalThreshold",
        "requestArtifactCoverageDeltaValueThreshold",
        "tokensPerSampleWarningDeltaRatioThreshold",
        "tokensPerSampleCriticalDeltaRatioThreshold",
        "tokensPerSampleCriticalAbsoluteThreshold",
    ] {
        if let Some(value) = parse_threshold_override(overrides, key) {
            object.insert(key.to_string(), json!(value));
        }
    }
    Value::Object(object)
}
