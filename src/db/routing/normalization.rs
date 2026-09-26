use super::*;

pub fn normalize_route_policy_config(
    input: GatewayRoutePolicyConfigInput,
) -> Result<GatewayRoutePolicyConfig, GatewayError> {
    let defaults = GatewayRoutePolicyConfig::default();
    Ok(GatewayRoutePolicyConfig {
        sticky_sessions: input.sticky_sessions.unwrap_or(true),
        pre_stream_fallback_enabled: input.pre_stream_fallback_enabled.unwrap_or(true),
        selection_strategy: normalize_selection_strategy(input.selection_strategy.as_deref()),
        provider_load_aware_routing_enabled: input
            .provider_load_aware_routing_enabled
            .unwrap_or(true),
        max_concurrent_requests: normalize_non_negative_i32(input.max_concurrent_requests)?,
        provider_max_concurrent_requests: normalize_non_negative_i32(
            input.provider_max_concurrent_requests,
        )?,
        rate_limit_enforcement_version: normalize_rate_limit_enforcement_version(
            input.rate_limit_enforcement_version.as_deref(),
        )?,
        rate_limit_window_seconds: normalize_non_negative_i32(input.rate_limit_window_seconds)?,
        rate_limit_max_requests: normalize_non_negative_i32(input.rate_limit_max_requests)?,
        api_key_rate_limit: normalize_rate_limit_definition(
            "apiKeyRateLimit",
            input.api_key_rate_limit,
            defaults.api_key_rate_limit,
        )?,
        model_rate_limits: normalize_rate_limit_map(
            "modelRateLimits",
            input.model_rate_limits,
            defaults.model_rate_limits,
            true,
        )?,
        endpoint_rate_limits: normalize_rate_limit_map(
            "endpointRateLimits",
            input.endpoint_rate_limits,
            defaults.endpoint_rate_limits,
            true,
        )?,
        provider_attempt_rate_limit: normalize_rate_limit_definition(
            "providerAttemptRateLimit",
            input.provider_attempt_rate_limit,
            defaults.provider_attempt_rate_limit,
        )?,
        circuit_breaker_threshold: normalize_non_negative_i32(input.circuit_breaker_threshold)?
            .unwrap_or(3),
        circuit_breaker_cooldown_seconds: normalize_non_negative_i32(
            input.circuit_breaker_cooldown_seconds,
        )?
        .unwrap_or(60),
        allowed_provider_account_ids: normalize_string_list(
            input.allowed_provider_account_ids,
            false,
        ),
        allowed_protocol_families: normalize_string_list(input.allowed_protocol_families, true),
        allowed_model_ids: normalize_string_list(input.allowed_model_ids, true),
        blocked_model_ids: normalize_string_list(input.blocked_model_ids, true),
        max_request_body_bytes: normalize_positive_i64(
            "maxRequestBodyBytes",
            input.max_request_body_bytes,
            1_000_000_000,
        )?,
        stream_idle_timeout_seconds: normalize_positive_i32(
            "streamIdleTimeoutSeconds",
            input.stream_idle_timeout_seconds,
            300,
        )?,
        total_request_timeout_seconds: normalize_positive_i32(
            "totalRequestTimeoutSeconds",
            input.total_request_timeout_seconds,
            300,
        )?,
        max_stream_heartbeat_gap_seconds: normalize_positive_i32(
            "maxStreamHeartbeatGapSeconds",
            input.max_stream_heartbeat_gap_seconds,
            60,
        )?,
        routing_anomaly_auto_remediation: input.routing_anomaly_auto_remediation,
        rate_limit_hotspot_auto_remediation: input.rate_limit_hotspot_auto_remediation,
        fallback_http_statuses: input
            .fallback_http_statuses
            .map(|values| {
                values
                    .into_iter()
                    .filter(|value| (100..=599).contains(value))
                    .collect::<Vec<_>>()
            })
            .unwrap_or(defaults.fallback_http_statuses),
        fallback_error_codes: normalize_string_list(input.fallback_error_codes, true)
            .or(defaults.fallback_error_codes),
    })
}

fn normalize_selection_strategy(value: Option<&str>) -> String {
    match value.map(|raw| raw.trim().to_lowercase()) {
        Some(value) if value == "priority" => "priority".to_string(),
        _ => "weighted_random".to_string(),
    }
}

fn normalize_non_negative_i32(value: Option<i32>) -> Result<Option<i32>, GatewayError> {
    match value {
        Some(value) if value < 0 => Err(GatewayError::bad_request("数值必须是非负整数")),
        other => Ok(other),
    }
}

fn normalize_rate_limit_enforcement_version(
    value: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if value != "v1" {
        return Err(
            GatewayError::bad_request("rateLimitEnforcementVersion 当前仅支持 v1")
                .with_code("rate_limit_enforcement_version_invalid"),
        );
    }
    Ok(Some(value))
}

fn normalize_positive_i32(
    label: &str,
    value: Option<i32>,
    max_value: i32,
) -> Result<Option<i32>, GatewayError> {
    match value {
        None => Ok(None),
        Some(value) if value <= 0 => {
            Err(GatewayError::bad_request(format!("{label} 必须是正整数")))
        }
        Some(value) if value > max_value => Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        ))),
        Some(value) => Ok(Some(value)),
    }
}

fn normalize_positive_i64(
    label: &str,
    value: Option<i64>,
    max_value: i64,
) -> Result<Option<i64>, GatewayError> {
    match value {
        None => Ok(None),
        Some(value) if value <= 0 => {
            Err(GatewayError::bad_request(format!("{label} 必须是正整数")))
        }
        Some(value) if value > max_value => Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        ))),
        Some(value) => Ok(Some(value)),
    }
}

fn normalize_rate_limit_definition(
    label: &str,
    value: Option<GatewayRateLimitDefinitionInput>,
    fallback: Option<GatewayRateLimitDefinition>,
) -> Result<Option<GatewayRateLimitDefinition>, GatewayError> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let window_seconds = normalize_positive_i32(
        &format!("{label}.windowSeconds"),
        value.window_seconds,
        86_400,
    )?;
    let max_requests = normalize_positive_i32(
        &format!("{label}.maxRequests"),
        value.max_requests,
        1_000_000,
    )?;
    match (window_seconds, max_requests) {
        (None, None) => Ok(fallback),
        (Some(window_seconds), Some(max_requests)) => Ok(Some(GatewayRateLimitDefinition {
            window_seconds,
            max_requests,
        })),
        _ => Err(GatewayError::bad_request(format!(
            "{label} 需要同时指定 windowSeconds 和 maxRequests"
        ))),
    }
}

fn normalize_rate_limit_map(
    label: &str,
    value: Option<HashMap<String, GatewayRateLimitDefinitionInput>>,
    fallback: Option<HashMap<String, GatewayRateLimitDefinition>>,
    normalize_key_lowercase: bool,
) -> Result<Option<HashMap<String, GatewayRateLimitDefinition>>, GatewayError> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    if value.is_empty() {
        return Ok(fallback);
    }
    let mut normalized = HashMap::new();
    for (raw_key, definition) in value {
        let mut key = raw_key.trim().to_string();
        if normalize_key_lowercase {
            key = key.to_lowercase();
        }
        if key.is_empty() {
            return Err(GatewayError::bad_request(format!(
                "{label} 条目的 key 不能为空"
            )));
        }
        if let Some(entry) =
            normalize_rate_limit_definition(&format!("{label}.{key}"), Some(definition), None)?
        {
            normalized.insert(key, entry);
        }
    }
    Ok((!normalized.is_empty()).then_some(normalized))
}

pub(super) fn normalize_string_list(
    values: Option<Vec<String>>,
    lower_case: bool,
) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or_default() {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized_value = if lower_case {
            trimmed.to_lowercase()
        } else {
            trimmed.to_string()
        };
        if seen.insert(normalized_value.clone()) {
            normalized.push(normalized_value);
        }
    }
    (!normalized.is_empty()).then_some(normalized)
}

pub(super) fn parse_route_policy_config(
    value: &Value,
) -> Result<GatewayRoutePolicyConfig, GatewayError> {
    serde_json::from_value::<GatewayRoutePolicyConfig>(value.clone()).map_err(|_| {
        GatewayError::server_error("persisted route policy config is invalid")
            .with_code("route_policy_config_invalid")
    })
}
