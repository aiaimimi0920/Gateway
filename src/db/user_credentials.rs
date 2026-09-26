//! User credential lookup, verification, issuance and revocation.

use super::*;

pub async fn find_user_credential(
    pool: &PgPool,
    credential_key: &str,
) -> Result<Option<GatewayUserCredentialCacheEntry>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayUserCredentialJoinRow>(
        r#"
        select
          c.id,
          c.user_id,
          c.project_id,
          c.credential_key,
          c.credential_type,
          c.status,
          c.expires_at,
          c.scope,
          c.metadata,
          p.tenant_id,
          p.status as project_status,
          t.status as tenant_status
        from gateway_user_credentials c
        join gateway_projects p on p.id = c.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where c.credential_key = $1
        limit 1
        "#,
    )
    .bind(credential_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(|row| GatewayUserCredentialCacheEntry {
        id: row.id,
        user_id: row.user_id,
        project_id: row.project_id,
        scope: row.scope.0,
        expires_at: format_timestamp(row.expires_at),
        status: row.status,
        tenant_id: row.tenant_id,
    }))
}

pub async fn verify_user_credential(
    pool: &PgPool,
    credential_key: &str,
    required_scope: Option<&str>,
) -> Result<VerifiedUserCredential, GatewayError> {
    let row = sqlx::query_as::<_, GatewayUserCredentialJoinRow>(
        r#"
        select
          c.id,
          c.user_id,
          c.project_id,
          c.credential_key,
          c.credential_type,
          c.status,
          c.expires_at,
          c.scope,
          c.metadata,
          p.tenant_id,
          p.status as project_status,
          t.status as tenant_status
        from gateway_user_credentials c
        join gateway_projects p on p.id = c.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where c.credential_key = $1
        limit 1
        "#,
    )
    .bind(credential_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证不存在".to_string()),
        });
    };

    if row.status != "active" {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some(format!("凭证状态为 {}", row.status)),
        });
    }
    if row.project_status != "active" || row.tenant_status != "active" {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证所属 project 或 tenant 不可用".to_string()),
        });
    }
    if row.expires_at <= OffsetDateTime::now_utc() {
        return Ok(VerifiedUserCredential {
            valid: false,
            credential: None,
            reason: Some("凭证已过期".to_string()),
        });
    }
    if let Some(scope) = required_scope {
        if !row.scope.0.iter().any(|item| item == scope) {
            return Ok(VerifiedUserCredential {
                valid: false,
                credential: None,
                reason: Some(format!("凭证没有 {scope} 权限")),
            });
        }
    }

    let credential = GatewayUserCredentialCacheEntry {
        id: row.id.clone(),
        user_id: row.user_id.clone(),
        project_id: row.project_id.clone(),
        scope: row.scope.0.clone(),
        expires_at: format_timestamp(row.expires_at),
        status: row.status.clone(),
        tenant_id: row.tenant_id.clone(),
    };

    sqlx::query(
        r#"
        update gateway_user_credentials
        set last_used_at = $2, updated_at = $2
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(VerifiedUserCredential {
        valid: true,
        credential: Some(credential),
        reason: None,
    })
}

pub async fn issue_user_credential(
    pool: &PgPool,
    input: UserCredentialIssueInput,
) -> Result<IssuedUserCredential, GatewayError> {
    let project = get_active_project(pool, &input.project_id).await?;
    let tenant = get_active_tenant(pool, &project.tenant_id).await?;
    let now = OffsetDateTime::now_utc();
    let expires_at = now + time::Duration::days(input.duration_days.max(1));
    let credential_id = format!("cred-{}", Uuid::new_v4().simple());
    let credential_key = format!("gw-user-{}", Uuid::new_v4().simple());

    sqlx::query(
        r#"
        insert into gateway_user_credentials (
          id,
          user_id,
          project_id,
          credential_key,
          credential_type,
          status,
          expires_at,
          scope,
          metadata,
          created_at,
          updated_at,
          last_used_at,
          revoked_at,
          revoke_reason
        ) values ($1, $2, $3, $4, $5, 'active', $6, $7, $8, $9, $9, null, null, null)
        "#,
    )
    .bind(&credential_id)
    .bind(&input.user_id)
    .bind(&input.project_id)
    .bind(&credential_key)
    .bind(&input.credential_type)
    .bind(expires_at)
    .bind(Json(input.scope.clone()))
    .bind(input.metadata.clone().map(Json))
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    Ok(IssuedUserCredential {
        id: credential_id,
        credential_key,
        expires_at: format_timestamp(expires_at),
        scope: input.scope,
        user_id: input.user_id,
        project_id: project.id,
        tenant_id: tenant.id,
        credential_type: input.credential_type,
    })
}

pub async fn revoke_user_credential(
    pool: &PgPool,
    credential_key: &str,
    reason: Option<&str>,
) -> Result<(), GatewayError> {
    let now = OffsetDateTime::now_utc();
    let result = sqlx::query(
        r#"
        update gateway_user_credentials
        set
          status = 'revoked',
          revoked_at = $2,
          revoke_reason = $3,
          updated_at = $2
        where credential_key = $1
        "#,
    )
    .bind(credential_key)
    .bind(now)
    .bind(reason)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("凭证不存在"));
    }
    Ok(())
}
