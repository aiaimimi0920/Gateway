use super::*;

pub async fn find_access_key_auth_by_id(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<Option<AccessKeyAuthRecord>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    Ok(row.map(|row| AccessKeyAuthRecord {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        external_key: row.external_key,
        expires_at: row.expires_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        projection_version: format_timestamp(row.updated_at),
    }))
}

pub async fn find_access_key_auth_by_external_key(
    pool: &PgPool,
    external_key: &str,
) -> Result<Option<AccessKeyAuthRecord>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where external_key = $1
        limit 1
        "#,
    )
    .bind(external_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    Ok(row.map(|row| AccessKeyAuthRecord {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        external_key: row.external_key,
        expires_at: row.expires_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        projection_version: format_timestamp(row.updated_at),
    }))
}

pub fn validate_access_key_auth(record: &AccessKeyAuthRecord) -> Result<(), GatewayError> {
    if normalize_status(&record.status) != "active" {
        return Err(GatewayError::unauthorized("Access key is not active"));
    }
    if parse_optional_timestamp(record.expires_at.as_deref())
        .is_some_and(|value| value <= now_utc())
    {
        return Err(GatewayError::unauthorized("Access key expired"));
    }
    Ok(())
}

pub async fn touch_access_key_last_used(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_access_keys
        set last_used_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub fn scope_list_from_metadata(metadata: Option<&Value>) -> Vec<String> {
    metadata
        .and_then(|value| value.get("scope").or_else(|| value.get("scopes")))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["relay".to_string()])
}
