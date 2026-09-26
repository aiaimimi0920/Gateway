use super::*;

pub async fn create_request_audit(
    pool: &PgPool,
    input: CreateRequestAuditInput,
) -> Result<String, GatewayError> {
    let request_audit_id = Uuid::new_v4().to_string();
    let timestamp = OffsetDateTime::now_utc();

    if input.api_key_id.is_none()
        && input.user_credential_id.is_none()
        && input.access_key_id.is_none()
    {
        return Err(GatewayError::bad_request(
            "request audit requires api_key_id, user_credential_id, or access_key_id",
        ));
    }

    sqlx::query(
        r#"
        insert into gateway_request_audits (
          id,
          project_id,
          api_key_id,
          user_credential_id,
          access_key_id,
          source_access_key_id,
          session_id,
          route_policy_id,
          provider_account_id,
          protocol_family,
          endpoint_kind,
          requested_model,
          resolved_model,
          model_alias,
          stream,
          route_attempt_count,
          status,
          upstream_status,
          duration_ms,
          prompt_tokens,
          completion_tokens,
          total_tokens,
          cache_creation_input_tokens,
          cache_read_input_tokens,
          error_summary,
          route_trace,
          analysis_profile,
          request_artifact_object_key,
          response_artifact_object_key,
          response_id,
          previous_response_id,
          client_disconnected_at,
          created_at,
          completed_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, 'running', null, null, null, null, null, null, null, null,
          $17, null, null, null, $18, $19, null, $20, null, $20
        )
        "#,
    )
    .bind(&request_audit_id)
    .bind(&input.project_id)
    .bind(input.api_key_id.as_deref())
    .bind(input.user_credential_id.as_deref())
    .bind(input.access_key_id.as_deref())
    .bind(input.source_access_key_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.route_policy_id.as_deref())
    .bind(input.provider_account_id.as_deref())
    .bind(&input.protocol_family)
    .bind(&input.endpoint_kind)
    .bind(input.requested_model.as_deref())
    .bind(input.resolved_model.as_deref())
    .bind(input.model_alias.as_deref())
    .bind(input.stream)
    .bind(i32::try_from(input.route_attempt_count.max(1)).unwrap_or(i32::MAX))
    .bind(input.route_trace.map(Json))
    .bind(&input.response_id)
    .bind(input.previous_response_id.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(request_audit_id)
}

pub async fn finalize_request_audit(
    pool: &PgPool,
    request_audit_id: &str,
    input: FinalizeRequestAuditInput,
) -> Result<(), GatewayError> {
    let timestamp = OffsetDateTime::now_utc();

    let result = sqlx::query(
        r#"
        update gateway_request_audits
        set
          status = $2,
          upstream_status = $3,
          duration_ms = $4,
          prompt_tokens = $5,
          completion_tokens = $6,
          total_tokens = $7,
          cache_creation_input_tokens = $8,
          cache_read_input_tokens = $9,
          client_has_cache_control = $10,
          auto_cache_applied = $11,
          error_summary = $12,
          route_trace = coalesce($13, route_trace),
          access_key_id = coalesce($14, access_key_id),
          source_access_key_id = coalesce($15, source_access_key_id),
          session_id = coalesce($16, session_id),
          route_policy_id = coalesce($17, route_policy_id),
          provider_account_id = coalesce($18, provider_account_id),
          resolved_model = coalesce($19, resolved_model),
          model_alias = coalesce($20, model_alias),
          route_attempt_count = $21,
          response_id = coalesce($22, response_id),
          completed_at = $23,
          updated_at = $23
        where id = $1
        "#,
    )
    .bind(request_audit_id)
    .bind(&input.status)
    .bind(input.upstream_status.map(i32::from))
    .bind(to_i32(input.duration_ms))
    .bind(input.prompt_tokens.map(to_i32))
    .bind(input.completion_tokens.map(to_i32))
    .bind(input.total_tokens.map(to_i32))
    .bind(input.cache_creation_input_tokens.map(to_i32))
    .bind(input.cache_read_input_tokens.map(to_i32))
    .bind(input.client_has_cache_control)
    .bind(input.auto_cache_applied)
    .bind(input.error_summary.as_deref())
    .bind(input.route_trace.map(Json))
    .bind(input.access_key_id.as_deref())
    .bind(input.source_access_key_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.route_policy_id.as_deref())
    .bind(input.provider_account_id.as_deref())
    .bind(input.resolved_model.as_deref())
    .bind(input.model_alias.as_deref())
    .bind(i32::try_from(input.route_attempt_count).unwrap_or(i32::MAX))
    .bind(input.response_id.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("AI gateway request audit 不存在"));
    }

    Ok(())
}

pub async fn update_request_audit_artifact_keys(
    pool: &PgPool,
    request_audit_id: &str,
    request_artifact_object_key: Option<&str>,
    response_artifact_object_key: Option<&str>,
) -> Result<(), GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_request_audits
        set
          request_artifact_object_key = coalesce($2, request_artifact_object_key),
          response_artifact_object_key = coalesce($3, response_artifact_object_key),
          updated_at = $4
        where id = $1
        "#,
    )
    .bind(request_audit_id)
    .bind(request_artifact_object_key)
    .bind(response_artifact_object_key)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(())
}

fn to_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
