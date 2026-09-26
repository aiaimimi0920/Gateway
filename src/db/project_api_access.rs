//! Project API-key lookup, creation and rotation.

use super::*;

pub async fn find_gateway_api_key_auth(
    pool: &PgPool,
    api_key_id: &str,
) -> Result<Option<GatewayApiKeyAuthRow>, GatewayError> {
    sqlx::query_as::<_, GatewayApiKeyAuthRow>(
        r#"
        select
          ak.id as api_key_id,
          ak.project_id,
          p.tenant_id,
          ak.status as api_key_status,
          p.status as project_status,
          t.status as tenant_status
        from gateway_api_keys ak
        join gateway_projects p on p.id = ak.project_id
        join gateway_tenants t on t.id = p.tenant_id
        where ak.id = $1
        limit 1
        "#,
    )
    .bind(api_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

pub async fn resolve_or_create_gateway_api_access(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    api_key_secret: &str,
) -> Result<GatewayProjectApiAccessView, GatewayError> {
    let project = get_active_project_detail(pool, project_id).await?;
    let tenant = get_active_tenant_detail(pool, &project.tenant_id).await?;

    let existing = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        select id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        from gateway_api_keys
        where project_id = $1 and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let api_key = if let Some(row) = existing {
        row
    } else {
        let now = OffsetDateTime::now_utc();
        let api_key_id = Uuid::new_v4().to_string();
        sqlx::query_as::<_, GatewayApiKeyDetailRow>(
            r#"
            insert into gateway_api_keys (
              id,
              project_id,
              name,
              status,
              rotated_from_api_key_id,
              revoked_at,
              revoked_by_user_id,
              revoke_reason,
              created_at,
              updated_at
            ) values ($1, $2, $3, 'active', null, null, null, null, $4, $4)
            returning id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
            "#,
        )
        .bind(&api_key_id)
        .bind(project_id)
        .bind(name)
        .bind(now)
        .fetch_one(pool)
        .await
        .map_err(map_db_error)?
    };

    let token = crate::gateway_api_key::build_gateway_project_api_key(
        &api_key.id,
        &project.id,
        &tenant.id,
        api_key_secret,
    );

    Ok(GatewayProjectApiAccessView {
        project: gateway_project_view_from_row(project),
        tenant: gateway_tenant_view_from_row(tenant),
        api_key: gateway_api_key_view_from_row(api_key),
        token,
    })
}

pub async fn rotate_gateway_api_access(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    actor_user_id: Option<&str>,
    api_key_secret: &str,
) -> Result<GatewayProjectApiAccessView, GatewayError> {
    let project = get_active_project_detail(pool, project_id).await?;
    let tenant = get_active_tenant_detail(pool, &project.tenant_id).await?;
    let now = OffsetDateTime::now_utc();

    let current = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        select id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        from gateway_api_keys
        where project_id = $1 and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let rotated_from = if let Some(row) = current {
        sqlx::query(
            r#"
            update gateway_api_keys
            set
              status = 'revoked',
              revoked_at = $2,
              revoked_by_user_id = $3,
              revoke_reason = 'rotated',
              updated_at = $2
            where id = $1
            "#,
        )
        .bind(&row.id)
        .bind(now)
        .bind(actor_user_id)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
        Some(row.id)
    } else {
        None
    };

    let next_id = Uuid::new_v4().to_string();
    let next = sqlx::query_as::<_, GatewayApiKeyDetailRow>(
        r#"
        insert into gateway_api_keys (
          id,
          project_id,
          name,
          status,
          rotated_from_api_key_id,
          revoked_at,
          revoked_by_user_id,
          revoke_reason,
          created_at,
          updated_at
        ) values ($1, $2, $3, 'active', $4, null, null, null, $5, $5)
        returning id, project_id, name, status, rotated_from_api_key_id, revoked_at, created_at
        "#,
    )
    .bind(&next_id)
    .bind(project_id)
    .bind(name)
    .bind(rotated_from.as_deref())
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    let token = crate::gateway_api_key::build_gateway_project_api_key(
        &next.id,
        &project.id,
        &tenant.id,
        api_key_secret,
    );

    Ok(GatewayProjectApiAccessView {
        project: gateway_project_view_from_row(project),
        tenant: gateway_tenant_view_from_row(tenant),
        api_key: gateway_api_key_view_from_row(next),
        token,
    })
}

fn gateway_api_key_view_from_row(row: GatewayApiKeyDetailRow) -> GatewayApiKeyView {
    GatewayApiKeyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        status: row.status,
        issued_at: format_timestamp(row.created_at),
        revoked_at: row.revoked_at.map(format_timestamp),
        rotated_from_api_key_id: row.rotated_from_api_key_id,
    }
}
