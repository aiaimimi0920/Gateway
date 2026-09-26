use crate::error::GatewayError;

pub(super) fn resolve_provider_supply_metric_kind(
    provider_supply_metric_kind: Option<&str>,
    legacy_metric_kind: Option<&str>,
) -> Result<String, GatewayError> {
    match (provider_supply_metric_kind, legacy_metric_kind) {
        (Some(provider_supply_metric_kind), Some(legacy_metric_kind)) => {
            let normalized_provider_supply_metric_kind =
                normalize_provider_supply_metric_kind(provider_supply_metric_kind)?;
            let normalized_legacy_metric_kind =
                normalize_provider_supply_metric_kind(legacy_metric_kind)?;
            if normalized_provider_supply_metric_kind != normalized_legacy_metric_kind {
                return Err(GatewayError::bad_request(
                    "providerSupplyMetricKind 与 legacy metricKind 不一致",
                ));
            }
            Ok(normalized_provider_supply_metric_kind)
        }
        (Some(provider_supply_metric_kind), None) => {
            normalize_provider_supply_metric_kind(provider_supply_metric_kind)
        }
        (None, Some(legacy_metric_kind)) => {
            normalize_provider_supply_metric_kind(legacy_metric_kind)
        }
        (None, None) => Err(GatewayError::bad_request(
            "providerSupplyMetricKind 不能为空",
        )),
    }
}

fn normalize_provider_supply_metric_kind(value: &str) -> Result<String, GatewayError> {
    let normalized = normalize_required_key(value, "providerSupplyMetricKind", 80)?;
    match normalized.as_str() {
        "credential_count" | "token_window" => Ok(normalized),
        _ => Err(GatewayError::bad_request(
            "providerSupplyMetricKind 只能是 credential_count 或 token_window",
        )),
    }
}

pub(super) fn normalize_required_key(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.len() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    let normalized = trimmed
        .chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' | '-' | '.' | ':' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能只包含非法字符"
        )));
    }
    Ok(normalized)
}

pub(super) fn normalize_credential_material_kind(value: &str) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed == "*" {
        return Ok("*".to_string());
    }
    normalize_required_key(trimmed, "credentialMaterialKind", 120)
}

pub(super) fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if trimmed.chars().count() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(trimmed.to_string())
}

pub(super) fn normalize_optional_key(
    value: Option<&str>,
    label: &str,
    max_len: usize,
) -> Result<Option<String>, GatewayError> {
    value
        .and_then(trim_nonempty)
        .map(|value| normalize_required_key(value, label, max_len))
        .transpose()
}

pub(super) fn trim_nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}
