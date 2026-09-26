use super::UpsertProviderCredentialInput;
use crate::error::GatewayError;
use time::OffsetDateTime;

pub(super) fn validate_provider_credential_input(
    input: &UpsertProviderCredentialInput,
) -> Result<(), GatewayError> {
    normalize_required_text(&input.provider_account_id, "providerAccountId", 120)?;
    normalize_required_text(&input.label, "Provider credential 标题", 160)?;
    if !input.payload.is_object() {
        return Err(GatewayError::bad_request(
            "provider credential payload 必须是 JSON object",
        ));
    }
    Ok(())
}

pub(super) fn normalize_required_text(
    value: &str,
    label: &str,
    max_len: usize,
) -> Result<String, GatewayError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(GatewayError::bad_request(format!("{label} 不能为空")));
    }
    if normalized.chars().count() > max_len {
        return Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_len} 个字符"
        )));
    }
    Ok(normalized.to_string())
}

pub(super) fn normalize_optional_text(value: Option<&str>, max_len: usize) -> Option<String> {
    let normalized = value?.trim();
    if normalized.is_empty() {
        return None;
    }
    Some(normalized.chars().take(max_len).collect())
}

pub(super) fn normalize_provider_credential_status(status: Option<&str>, fallback: &str) -> String {
    status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub(super) fn archived_at_for_status(
    status: &str,
    existing_archived_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    if status.eq_ignore_ascii_case("archived") {
        return existing_archived_at.or(Some(now));
    }
    None
}
