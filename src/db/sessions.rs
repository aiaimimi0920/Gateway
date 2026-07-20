use serde::Serialize;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

use crate::error::GatewayError;

use super::format_timestamp;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionView {
    pub id: String,
    pub project_id: String,
    pub session_key: String,
    pub protocol_family: String,
    pub provider_account_id: String,
    pub latest_response_id: Option<String>,
    pub upstream_session_id: Option<String>,
    pub runtime_state_object_key: Option<String>,
    pub active_request_audit_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertGatewaySessionInput {
    pub project_id: String,
    pub session_key: String,
    pub protocol_family: String,
    pub provider_account_id: String,
    pub upstream_session_id: Option<String>,
    pub runtime_state_object_key: Option<String>,
    pub latest_response_id: Option<String>,
    pub active_request_audit_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct GatewaySessionRow {
    id: String,
    project_id: String,
    session_key: String,
    protocol_family: String,
    provider_account_id: String,
    latest_response_id: Option<String>,
    upstream_session_id: Option<String>,
    runtime_state_object_key: Option<String>,
    active_request_audit_id: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    last_used_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
}

fn session_view_from_row(row: GatewaySessionRow) -> GatewaySessionView {
    GatewaySessionView {
        id: row.id,
        project_id: row.project_id,
        session_key: row.session_key,
        protocol_family: row.protocol_family,
        provider_account_id: row.provider_account_id,
        latest_response_id: row.latest_response_id,
        upstream_session_id: row.upstream_session_id,
        runtime_state_object_key: row.runtime_state_object_key,
        active_request_audit_id: row.active_request_audit_id,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        last_used_at: format_timestamp(row.last_used_at),
        revoked_at: row.revoked_at.map(format_timestamp),
    }
}

pub async fn resolve_gateway_session(
    pg_pool: &PgPool,
    project_id: &str,
    session_key: Option<&str>,
    previous_response_id: Option<&str>,
) -> Result<Option<GatewaySessionView>, GatewayError> {
    if let Some(session_key) = session_key.map(str::trim).filter(|value| !value.is_empty()) {
        let row = sqlx::query_as::<_, GatewaySessionRow>(
            r#"
            select
              id,
              project_id,
              session_key,
              protocol_family,
              provider_account_id,
              latest_response_id,
              upstream_session_id,
              runtime_state_object_key,
              active_request_audit_id,
              created_at,
              updated_at,
              last_used_at,
              revoked_at
            from gateway_sessions
            where project_id = $1
              and session_key = $2
            limit 1
            "#,
        )
        .bind(project_id.trim())
        .bind(session_key)
        .fetch_optional(pg_pool)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("读取 gateway session 失败: {error}"))
        })?;
        return Ok(row.map(session_view_from_row));
    }

    let Some(previous_response_id) = previous_response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let session_id = sqlx::query_scalar::<_, Option<String>>(
        r#"
        select session_id
        from gateway_request_audits
        where project_id = $1
          and response_id = $2
        limit 1
        "#,
    )
    .bind(project_id.trim())
    .bind(previous_response_id)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| {
        GatewayError::server_error(format!("读取 request audit session 失败: {error}"))
    })?
    .flatten();

    let Some(session_id) = session_id else {
        return Ok(None);
    };

    let row = sqlx::query_as::<_, GatewaySessionRow>(
        r#"
        select
          id,
          project_id,
          session_key,
          protocol_family,
          provider_account_id,
          latest_response_id,
          upstream_session_id,
          runtime_state_object_key,
          active_request_audit_id,
          created_at,
          updated_at,
          last_used_at,
          revoked_at
        from gateway_sessions
        where id = $1
        limit 1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| {
        GatewayError::server_error(format!("读取 gateway session 详情失败: {error}"))
    })?;

    Ok(row.map(session_view_from_row))
}

pub async fn upsert_gateway_session(
    pg_pool: &PgPool,
    input: UpsertGatewaySessionInput,
) -> Result<GatewaySessionView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let existing = sqlx::query_as::<_, GatewaySessionRow>(
        r#"
        select
          id,
          project_id,
          session_key,
          protocol_family,
          provider_account_id,
          latest_response_id,
          upstream_session_id,
          runtime_state_object_key,
          active_request_audit_id,
          created_at,
          updated_at,
          last_used_at,
          revoked_at
        from gateway_sessions
        where project_id = $1
          and session_key = $2
        limit 1
        "#,
    )
    .bind(input.project_id.trim())
    .bind(input.session_key.trim())
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("查询 gateway session 失败: {error}")))?;

    if let Some(existing) = existing {
        let row = sqlx::query_as::<_, GatewaySessionRow>(
            r#"
            update gateway_sessions
            set
              protocol_family = $2,
              provider_account_id = $3,
              upstream_session_id = coalesce($4, upstream_session_id),
              runtime_state_object_key = coalesce($5, runtime_state_object_key),
              latest_response_id = coalesce($6, latest_response_id),
              active_request_audit_id = coalesce($7, active_request_audit_id),
              updated_at = $8,
              last_used_at = $8,
              revoked_at = null
            where id = $1
            returning
              id,
              project_id,
              session_key,
              protocol_family,
              provider_account_id,
              latest_response_id,
              upstream_session_id,
              runtime_state_object_key,
              active_request_audit_id,
              created_at,
              updated_at,
              last_used_at,
              revoked_at
            "#,
        )
        .bind(existing.id)
        .bind(input.protocol_family.trim())
        .bind(input.provider_account_id.trim())
        .bind(input.upstream_session_id.as_deref())
        .bind(input.runtime_state_object_key.as_deref())
        .bind(input.latest_response_id.as_deref())
        .bind(input.active_request_audit_id.as_deref())
        .bind(timestamp)
        .fetch_one(pg_pool)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("更新 gateway session 失败: {error}"))
        })?;

        return Ok(session_view_from_row(row));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let row = sqlx::query_as::<_, GatewaySessionRow>(
        r#"
        insert into gateway_sessions (
          id,
          project_id,
          session_key,
          protocol_family,
          provider_account_id,
          latest_response_id,
          upstream_session_id,
          runtime_state_object_key,
          active_request_audit_id,
          created_at,
          updated_at,
          last_used_at,
          revoked_at
        ) values (
          $1,
          $2,
          $3,
          $4,
          $5,
          $6,
          $7,
          $8,
          $9,
          $10,
          $10,
          $10,
          null
        )
        returning
          id,
          project_id,
          session_key,
          protocol_family,
          provider_account_id,
          latest_response_id,
          upstream_session_id,
          runtime_state_object_key,
          active_request_audit_id,
          created_at,
          updated_at,
          last_used_at,
          revoked_at
        "#,
    )
    .bind(session_id)
    .bind(input.project_id.trim())
    .bind(input.session_key.trim())
    .bind(input.protocol_family.trim())
    .bind(input.provider_account_id.trim())
    .bind(input.latest_response_id.as_deref())
    .bind(input.upstream_session_id.as_deref())
    .bind(input.runtime_state_object_key.as_deref())
    .bind(input.active_request_audit_id.as_deref())
    .bind(timestamp)
    .fetch_one(pg_pool)
    .await
    .map_err(|error| GatewayError::server_error(format!("创建 gateway session 失败: {error}")))?;

    Ok(session_view_from_row(row))
}
