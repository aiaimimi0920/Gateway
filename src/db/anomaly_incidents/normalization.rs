use super::*;

pub(super) fn normalize_incident_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "acknowledged" => "acknowledged",
        "resolved" => "resolved",
        _ => "open",
    }
}

pub(super) fn normalize_incident_follow_up_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "investigating" => "investigating",
        "monitoring" => "monitoring",
        "done" => "done",
        _ => "pending",
    }
}

pub(super) fn normalize_incident_escalation_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "escalated" => "escalated",
        "resolved" => "resolved",
        _ => "none",
    }
}

pub(super) fn trimmed_owned(value: Option<&str>) -> Option<String> {
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

pub(super) fn require_incident_id(incident_id: &str) -> Result<&str, GatewayError> {
    trimmed_owned_ref(incident_id).ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))
}
