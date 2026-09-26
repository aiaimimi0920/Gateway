use super::*;

#[derive(Debug, Clone, FromRow)]
struct GatewayRequestAuditRow {
    id: String,
    project_id: String,
    api_key_id: Option<String>,
    user_credential_id: Option<String>,
    access_key_id: Option<String>,
    source_access_key_id: Option<String>,
    session_id: Option<String>,
    route_policy_id: Option<String>,
    provider_account_id: Option<String>,
    protocol_family: String,
    endpoint_kind: String,
    requested_model: Option<String>,
    resolved_model: Option<String>,
    model_alias: Option<String>,
    stream: bool,
    status: String,
    upstream_status: Option<i32>,
    duration_ms: Option<i32>,
    prompt_tokens: Option<i32>,
    completion_tokens: Option<i32>,
    total_tokens: Option<i32>,
    cache_creation_input_tokens: Option<i32>,
    cache_read_input_tokens: Option<i32>,
    client_has_cache_control: bool,
    auto_cache_applied: bool,
    error_summary: Option<String>,
    route_trace: Option<Json<Value>>,
    analysis_profile: Option<Json<Value>>,
    request_artifact_object_key: Option<String>,
    response_artifact_object_key: Option<String>,
    response_id: String,
    previous_response_id: Option<String>,
    client_disconnected_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    completed_at: Option<OffsetDateTime>,
    updated_at: OffsetDateTime,
}

pub async fn list_request_audits(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<Vec<GatewayRequestAuditView>, GatewayError> {
    let (created_from, created_to) = parse_request_audit_created_range(filters)?;

    let limit = filters.limit.unwrap_or(200).clamp(1, 1000);
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, api_key_id, user_credential_id, access_key_id, source_access_key_id, session_id, route_policy_id, provider_account_id,
          protocol_family, endpoint_kind, requested_model, resolved_model, model_alias,
          stream, status, upstream_status, duration_ms, prompt_tokens, completion_tokens,
          total_tokens, cache_creation_input_tokens, cache_read_input_tokens, client_has_cache_control, auto_cache_applied,
          error_summary, route_trace, analysis_profile,
          request_artifact_object_key, response_artifact_object_key, response_id,
          previous_response_id, client_disconnected_at, created_at, completed_at, updated_at
        from gateway_request_audits
        where 1 = 1
        "#,
    );

    push_request_audit_filters(&mut builder, filters, created_from, created_to);

    builder
        .push(" order by created_at desc limit ")
        .push_bind(i64::try_from(limit).unwrap_or(1000));

    let rows = builder
        .build_query_as::<GatewayRequestAuditRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_request_audit_view).collect())
}

pub async fn get_request_audit(
    pool: &PgPool,
    request_audit_id: Option<&str>,
    response_id: Option<&str>,
) -> Result<Option<GatewayRequestAuditView>, GatewayError> {
    let request_audit_id = non_empty(request_audit_id);
    let response_id = non_empty(response_id);

    if request_audit_id.is_none() && response_id.is_none() {
        return Err(GatewayError::bad_request(
            "必须提供 requestAuditId 或 responseId",
        ));
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, api_key_id, user_credential_id, access_key_id, source_access_key_id, session_id, route_policy_id, provider_account_id,
          protocol_family, endpoint_kind, requested_model, resolved_model, model_alias,
          stream, status, upstream_status, duration_ms, prompt_tokens, completion_tokens,
          total_tokens, cache_creation_input_tokens, cache_read_input_tokens, client_has_cache_control, auto_cache_applied,
          error_summary, route_trace, analysis_profile,
          request_artifact_object_key, response_artifact_object_key, response_id,
          previous_response_id, client_disconnected_at, created_at, completed_at, updated_at
        from gateway_request_audits
        where
        "#,
    );

    if let Some(value) = request_audit_id {
        builder.push(" id = ").push_bind(value);
    } else if let Some(value) = response_id {
        builder.push(" response_id = ").push_bind(value);
    }

    builder.push(" limit 1");

    let row = builder
        .build_query_as::<GatewayRequestAuditRow>()
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?;

    Ok(row.map(to_request_audit_view))
}

fn to_request_audit_view(row: GatewayRequestAuditRow) -> GatewayRequestAuditView {
    GatewayRequestAuditView {
        id: row.id,
        project_id: row.project_id,
        api_key_id: row.api_key_id,
        user_credential_id: row.user_credential_id,
        access_key_id: row.access_key_id,
        source_access_key_id: row.source_access_key_id,
        session_id: row.session_id,
        route_policy_id: row.route_policy_id,
        provider_account_id: row.provider_account_id,
        protocol_family: row.protocol_family,
        endpoint_kind: row.endpoint_kind,
        requested_model: row.requested_model,
        resolved_model: row.resolved_model,
        model_alias: row.model_alias,
        stream: row.stream,
        status: row.status,
        upstream_status: row.upstream_status,
        duration_ms: row.duration_ms,
        prompt_tokens: row.prompt_tokens,
        completion_tokens: row.completion_tokens,
        total_tokens: row.total_tokens,
        cache_creation_input_tokens: row.cache_creation_input_tokens,
        cache_read_input_tokens: row.cache_read_input_tokens,
        client_has_cache_control: row.client_has_cache_control,
        auto_cache_applied: row.auto_cache_applied,
        error_summary: row.error_summary,
        route_trace: row.route_trace.map(|value| value.0),
        analysis_profile: row.analysis_profile.map(|value| value.0),
        request_artifact_object_key: row.request_artifact_object_key,
        response_artifact_object_key: row.response_artifact_object_key,
        response_id: row.response_id,
        previous_response_id: row.previous_response_id,
        client_disconnected_at: row.client_disconnected_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        completed_at: row.completed_at.map(format_timestamp),
        updated_at: format_timestamp(row.updated_at),
    }
}
