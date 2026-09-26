use crate::error::GatewayError;
use crate::state::AppState;

use super::*;

pub(super) fn ensure_refill_enabled(state: &AppState) -> Result<(), GatewayError> {
    if state.credential_pool_automation.refill_queue_enabled() {
        Ok(())
    } else {
        Err(GatewayError::service_unavailable("补号任务框架未启用")
            .with_code("credential_refill_disabled"))
    }
}

pub(super) fn validate_requested_count(requested_count: usize) -> Result<(), GatewayError> {
    if (1..=MAX_REQUESTED_COUNT).contains(&requested_count) {
        Ok(())
    } else {
        Err(GatewayError::bad_request(format!(
            "requestedCount 必须位于 1..={MAX_REQUESTED_COUNT}"
        )))
    }
}

pub(super) fn validate_folder_paths(paths: &[String]) -> Result<(), GatewayError> {
    if paths.len() > MAX_FOLDER_PATHS {
        return Err(GatewayError::bad_request("relativePaths 数量过多"));
    }
    for path in paths {
        let normalized = path.trim();
        if normalized.is_empty()
            || normalized.chars().count() > MAX_FOLDER_PATH_LENGTH
            || std::path::Path::new(normalized).is_absolute()
            || std::path::Path::new(normalized)
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(GatewayError::bad_request(
                "relativePaths 必须是根目录内不包含 '..' 的相对路径",
            ));
        }
    }
    Ok(())
}

pub(super) fn normalize_identifier(
    value: &str,
    field: &str,
    max: usize,
) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty()
        || normalized.chars().count() > max
        || !normalized.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(GatewayError::bad_request(format!(
            "{field} 只能包含 ASCII 字母、数字、'-'、'_' 或 '.'"
        )));
    }
    Ok(normalized.to_string())
}

pub(super) fn normalize_required_text(
    value: &str,
    field: &str,
    max: usize,
) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.chars().count() > max {
        return Err(GatewayError::bad_request(format!(
            "{field} 不能为空且长度不能超过 {max}"
        )));
    }
    Ok(normalized.to_string())
}

pub(super) fn normalize_optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
) -> Result<Option<String>, GatewayError> {
    value
        .map(|value| normalize_required_text(&value, field, max))
        .transpose()
}
