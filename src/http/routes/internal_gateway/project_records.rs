//! Project/tenant/key row reads and management API view projection.

use crate::db;
use crate::error::GatewayError;
use sqlx::FromRow;
use time::OffsetDateTime;

#[derive(Debug, Clone, FromRow)]
pub(super) struct ProjectDetailRow {
    pub(super) id: String,
    pub(super) tenant_id: String,
    slug: String,
    pub(super) display_name: String,
    status: String,
    source_kind: String,
    source_key: String,
    default_route_policy_id: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct TenantDetailRow {
    pub(super) id: String,
    slug: String,
    display_name: String,
    status: String,
    owner_user_id: Option<String>,
    source_kind: String,
    source_key: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
pub(super) struct AccessKeyCompatRow {
    pub(super) id: String,
    pub(super) resolved_project_id: String,
    pub(super) resolved_tenant_id: String,
    pub(super) display_name: String,
    pub(super) status: String,
    pub(super) rotated_from_access_key_id: Option<String>,
    pub(super) revoked_at: Option<OffsetDateTime>,
    pub(super) created_at: OffsetDateTime,
}

pub(super) fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub(super) fn project_view_from_row(row: ProjectDetailRow) -> db::GatewayProjectView {
    db::GatewayProjectView {
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

pub(super) fn tenant_view_from_row(row: TenantDetailRow) -> db::GatewayTenantView {
    db::GatewayTenantView {
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

pub(super) async fn load_project_detail(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<ProjectDetailRow, GatewayError> {
    let row = sqlx::query_as::<_, ProjectDetailRow>(
        r#"
        select
          id, tenant_id, slug, display_name, status, source_kind, source_key,
          default_route_policy_id, created_at, updated_at
        from gateway_projects
        where id = $1
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load gateway project: {error}")))?;
    let Some(row) = row else {
        return Err(GatewayError::not_found("AI gateway project 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway project 未激活"));
    }
    Ok(row)
}

pub(super) async fn load_tenant_detail(
    pool: &sqlx::PgPool,
    tenant_id: &str,
) -> Result<TenantDetailRow, GatewayError> {
    let row = sqlx::query_as::<_, TenantDetailRow>(
        r#"
        select
          id, slug, display_name, status, owner_user_id, source_kind, source_key, created_at, updated_at
        from gateway_tenants
        where id = $1
        limit 1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load gateway tenant: {error}")))?;
    let Some(row) = row else {
        return Err(GatewayError::not_found("AI gateway tenant 不存在"));
    };
    if row.status != "active" {
        return Err(GatewayError::conflict("AI gateway tenant 未激活"));
    }
    Ok(row)
}

pub(super) async fn load_active_project_access_key(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<Option<AccessKeyCompatRow>, GatewayError> {
    sqlx::query_as::<_, AccessKeyCompatRow>(
        r#"
        select
          id,
          resolved_project_id,
          resolved_tenant_id,
          display_name,
          status,
          rotated_from_access_key_id,
          revoked_at,
          created_at
        from gateway_access_keys
        where owner_type = 'project'
          and owner_id = $1
          and key_kind = 'normal'
          and public_key_prefix = 'new_api'
          and status = 'active'
        order by created_at desc, id desc
        limit 1
        "#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("load project access key: {error}")))
}
