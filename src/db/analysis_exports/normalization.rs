use super::*;

pub(super) fn normalize_text_mode(value: Option<&str>) -> &'static str {
    match value.and_then(trimmed_owned_ref) {
        Some("none") => "none",
        Some("full") => "full",
        Some("preview_redacted") => "preview_redacted",
        _ => DEFAULT_TEXT_MODE,
    }
}

pub(super) fn normalize_max_text_chars(value: Option<usize>) -> usize {
    value
        .unwrap_or(DEFAULT_MAX_TEXT_CHARS)
        .clamp(0, MAX_TEXT_CHARS_LIMIT)
}

pub(super) fn normalize_export_label(value: Option<&str>) -> Result<Option<String>, GatewayError> {
    let Some(value) = trimmed_owned_ref_opt(value) else {
        return Ok(None);
    };
    Ok(Some(
        value.chars().take(MAX_LABEL_LENGTH).collect::<String>(),
    ))
}

pub(super) fn normalize_export_tags(
    values: Option<&Vec<String>>,
) -> Result<Vec<String>, GatewayError> {
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    if values.len() > MAX_TAGS {
        return Err(GatewayError::conflict("tags 数量不能超过 32 个。"));
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for value in values {
        let Some(normalized) = trimmed_owned(Some(value.as_str())).map(|item| item.to_lowercase())
        else {
            continue;
        };
        if normalized.chars().count() > MAX_TAG_LENGTH {
            return Err(GatewayError::conflict("单个 tag 长度不能超过 40 个字符。"));
        }
        if seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    Ok(result)
}

pub(super) fn parse_optional_rfc3339(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = trimmed_owned_ref_opt(value) else {
        return Ok(None);
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::bad_request(format!("{field_name} 必须是合法的 RFC3339 时间戳")))
}

pub(super) fn has_pinned_tag(tags: &[String]) -> bool {
    normalize_tag_values(tags).iter().any(|tag| tag == "pinned")
}

pub(super) fn normalize_tag_values(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for tag in tags {
        let Some(tag) = trimmed_owned_ref(tag) else {
            continue;
        };
        let tag = tag.to_lowercase();
        if seen.insert(tag.clone()) {
            normalized.push(tag);
        }
    }
    normalized
}

pub(super) fn is_non_active_status_filter(status: Option<&str>) -> bool {
    matches!(
        trimmed_owned_ref_opt(status),
        Some(value) if value != "active"
    )
}

pub(super) fn validate_created_range(
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> Result<(), GatewayError> {
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo。"));
        }
    }
    Ok(())
}

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn trimmed_owned_ref(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(super) fn trimmed_owned_ref_opt(value: Option<&str>) -> Option<&str> {
    value.and_then(trimmed_owned_ref)
}
