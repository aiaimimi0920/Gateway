//! Tenant/project detail reads and public-view conversion.

use super::*;

pub(super) fn gateway_tenant_view_from_row(row: GatewayTenantDetailRow) -> GatewayTenantView {
    GatewayTenantView {
        id: row.id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        owner_user_id: row.owner_user_id,
        source_kind: row.source_kind,
        source_key: row.source_key,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn gateway_project_view_from_row(row: GatewayProjectDetailRow) -> GatewayProjectView {
    GatewayProjectView {
        id: row.id,
        tenant_id: row.tenant_id,
        slug: row.slug,
        display_name: row.display_name,
        status: row.status,
        source_kind: row.source_kind,
        source_key: row.source_key,
        default_route_policy_id: row.default_route_policy_id,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) async fn get_project_detail(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<GatewayProjectDetailRow>, GatewayError> {
    sqlx::query_as::<_, GatewayProjectDetailRow>(
        r#"
        select
          id,
          tenant_id,
          slug,
          display_name,
          status,
          source_kind,
          source_key,
          default_route_policy_id,
          created_at,
          updated_at
        from gateway_projects
        where id = $1
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

pub(super) async fn get_active_project_detail(
    pool: &PgPool,
    project_id: &str,
) -> Result<GatewayProjectDetailRow, GatewayError> {
    let row = get_project_detail(pool, project_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway project 不存在"))?;
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway project 未激活"));
    }
    Ok(row)
}

async fn get_tenant_detail(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<Option<GatewayTenantDetailRow>, GatewayError> {
    sqlx::query_as::<_, GatewayTenantDetailRow>(
        r#"
        select id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where id = $1
        limit 1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
}

pub(super) async fn get_active_tenant_detail(
    pool: &PgPool,
    tenant_id: &str,
) -> Result<GatewayTenantDetailRow, GatewayError> {
    let row = get_tenant_detail(pool, tenant_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("AI gateway tenant 不存在"))?;
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway tenant 未激活"));
    }
    Ok(row)
}
