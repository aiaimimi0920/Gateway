use crate::error::GatewayError;
pub(super) fn normalize_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn normalize_string_array(values: Option<Vec<String>>) -> Vec<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or_default() {
        let trimmed = value.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

pub(super) fn normalize_optional_string_array(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let normalized = normalize_string_array(values);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn required_trimmed(value: &str, field_name: &str) -> Result<String, GatewayError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayError::bad_request(format!("{field_name} 不能为空")));
    }
    Ok(trimmed.to_string())
}
