use super::*;

pub(super) fn push_request_audit_filters(
    builder: &mut QueryBuilder<Postgres>,
    filters: &RequestAuditFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) {
    if let Some(value) = non_empty(filters.project_id.as_deref()) {
        builder
            .push(" and project_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.route_policy_id.as_deref()) {
        builder
            .push(" and route_policy_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.provider_account_id.as_deref()) {
        builder
            .push(" and provider_account_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.session_id.as_deref()) {
        builder
            .push(" and session_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.api_key_id.as_deref()) {
        builder
            .push(" and api_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.user_credential_id.as_deref()) {
        builder
            .push(" and user_credential_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.access_key_id.as_deref()) {
        builder
            .push(" and access_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.source_access_key_id.as_deref()) {
        builder
            .push(" and source_access_key_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.response_id.as_deref()) {
        builder
            .push(" and response_id = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.protocol_family.as_deref()) {
        builder
            .push(" and protocol_family = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.status.as_deref()) {
        builder.push(" and status = ").push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.endpoint_kind.as_deref()) {
        builder
            .push(" and endpoint_kind = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = non_empty(filters.error_code.as_deref()) {
        builder
            .push(" and route_trace ->> 'errorCode' = ")
            .push_bind(value.to_string());
    }
    if let Some(value) = filters.fallback_eligible {
        builder
            .push(" and coalesce((route_trace ->> 'fallbackEligible')::boolean, false) = ")
            .push_bind(value);
    }
    if let Some(value) = filters.artifact_available {
        if value {
            builder.push(
                " and (request_artifact_object_key is not null or response_artifact_object_key is not null)",
            );
        } else {
            builder.push(
                " and request_artifact_object_key is null and response_artifact_object_key is null",
            );
        }
    }
    if let Some(value) = filters.stream {
        builder.push(" and stream = ").push_bind(value);
    }
    if let Some(value) = created_from {
        builder.push(" and created_at >= ").push_bind(value);
    }
    if let Some(value) = created_to {
        builder.push(" and created_at <= ").push_bind(value);
    }
}

pub(super) fn parse_request_audit_created_range(
    filters: &RequestAuditFilters,
) -> Result<(Option<OffsetDateTime>, Option<OffsetDateTime>), GatewayError> {
    let created_from = parse_optional_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(from), Some(to)) = (created_from, created_to) {
        if from > to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    Ok((created_from, created_to))
}

fn parse_optional_timestamp(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = non_empty(value) else {
        return Ok(None);
    };

    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::bad_request(format!("{field_name} 必须是合法的 RFC3339 时间")))
}

pub(super) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
