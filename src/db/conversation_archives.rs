use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone)]
pub struct CreateConversationArchiveInput {
    pub id: String,
    pub request_audit_id: Option<String>,
    pub request_id: String,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_family: String,
    pub protocol_profile: Option<String>,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: String,
    pub upstream_status: Option<u16>,
    pub failure_class: Option<String>,
    pub failure_scope: Option<String>,
    pub request_object_key: Option<String>,
    pub response_object_key: Option<String>,
    pub redaction_version: String,
    pub truncated_request: bool,
    pub truncated_response: bool,
    pub archive_error: Option<String>,
    pub retention_expires_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationArchiveFilters {
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_family: Option<String>,
    pub protocol_profile: Option<String>,
    pub endpoint_kind: Option<String>,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: Option<String>,
    pub failure_class: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConversationArchiveView {
    pub id: String,
    pub request_audit_id: Option<String>,
    pub request_id: String,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_family: String,
    pub protocol_profile: Option<String>,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: String,
    pub upstream_status: Option<i32>,
    pub failure_class: Option<String>,
    pub failure_scope: Option<String>,
    pub request_object_key: Option<String>,
    pub response_object_key: Option<String>,
    pub redaction_version: String,
    pub truncated_request: bool,
    pub truncated_response: bool,
    pub archive_error: Option<String>,
    pub retention_expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConversationArchiveExportView {
    pub export_id: String,
    pub dataset_object_key: String,
    pub row_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayConversationArchiveRow {
    id: String,
    request_audit_id: Option<String>,
    request_id: String,
    project_id: Option<String>,
    user_id: Option<String>,
    session_id: Option<String>,
    provider_account_id: Option<String>,
    provider_credential_ref: Option<String>,
    protocol_family: String,
    protocol_profile: Option<String>,
    endpoint_kind: String,
    requested_model: Option<String>,
    resolved_model: Option<String>,
    status: String,
    upstream_status: Option<i32>,
    failure_class: Option<String>,
    failure_scope: Option<String>,
    request_object_key: Option<String>,
    response_object_key: Option<String>,
    redaction_version: String,
    truncated_request: bool,
    truncated_response: bool,
    archive_error: Option<String>,
    retention_expires_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub async fn create_conversation_archive(
    pool: &PgPool,
    input: CreateConversationArchiveInput,
) -> Result<GatewayConversationArchiveView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let row = sqlx::query_as::<_, GatewayConversationArchiveRow>(
        r#"
        insert into gateway_conversation_archives (
          id,
          request_audit_id,
          request_id,
          project_id,
          user_id,
          session_id,
          provider_account_id,
          provider_credential_ref,
          protocol_family,
          protocol_profile,
          endpoint_kind,
          requested_model,
          resolved_model,
          status,
          upstream_status,
          failure_class,
          failure_scope,
          request_object_key,
          response_object_key,
          redaction_version,
          truncated_request,
          truncated_response,
          archive_error,
          retention_expires_at,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
          $21, $22, $23, $24, $25, $25
        )
        returning *
        "#,
    )
    .bind(&input.id)
    .bind(input.request_audit_id.as_deref())
    .bind(&input.request_id)
    .bind(input.project_id.as_deref())
    .bind(input.user_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.provider_account_id.as_deref())
    .bind(input.provider_credential_ref.as_deref())
    .bind(&input.protocol_family)
    .bind(input.protocol_profile.as_deref())
    .bind(&input.endpoint_kind)
    .bind(input.requested_model.as_deref())
    .bind(input.resolved_model.as_deref())
    .bind(&input.status)
    .bind(input.upstream_status.map(i32::from))
    .bind(input.failure_class.as_deref())
    .bind(input.failure_scope.as_deref())
    .bind(input.request_object_key.as_deref())
    .bind(input.response_object_key.as_deref())
    .bind(&input.redaction_version)
    .bind(input.truncated_request)
    .bind(input.truncated_response)
    .bind(input.archive_error.as_deref())
    .bind(input.retention_expires_at)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_view(row))
}

pub async fn get_conversation_archive(
    pool: &PgPool,
    archive_id: &str,
) -> Result<Option<GatewayConversationArchiveView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayConversationArchiveRow>(
        r#"
        select *
        from gateway_conversation_archives
        where id = $1
        "#,
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(to_view))
}

pub async fn list_conversation_archives(
    pool: &PgPool,
    filters: ConversationArchiveFilters,
) -> Result<Vec<GatewayConversationArchiveView>, GatewayError> {
    let mut builder =
        QueryBuilder::<Postgres>::new("select * from gateway_conversation_archives where 1 = 1");

    push_archive_filters(&mut builder, &filters)?;

    builder.push(" order by created_at desc limit ");
    builder.push_bind(i64::try_from(filters.limit.unwrap_or(100).clamp(1, 1000)).unwrap_or(100));

    let rows = builder
        .build_query_as::<GatewayConversationArchiveRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_view).collect())
}

pub async fn export_conversation_archives(
    pool: &PgPool,
    filters: ConversationArchiveFilters,
) -> Result<(String, Vec<GatewayConversationArchiveView>), GatewayError> {
    let rows = list_conversation_archives(pool, filters).await?;
    let export_id = Uuid::new_v4().to_string();
    Ok((export_id, rows))
}

fn push_archive_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    filters: &ConversationArchiveFilters,
) -> Result<(), GatewayError> {
    push_optional_text_filter(builder, "project_id", filters.project_id.as_deref());
    push_optional_text_filter(builder, "user_id", filters.user_id.as_deref());
    push_optional_text_filter(
        builder,
        "provider_account_id",
        filters.provider_account_id.as_deref(),
    );
    push_optional_text_filter(
        builder,
        "provider_credential_ref",
        filters.provider_credential_ref.as_deref(),
    );
    push_optional_text_filter(
        builder,
        "protocol_family",
        filters.protocol_family.as_deref(),
    );
    push_optional_text_filter(
        builder,
        "protocol_profile",
        filters.protocol_profile.as_deref(),
    );
    push_optional_text_filter(builder, "endpoint_kind", filters.endpoint_kind.as_deref());
    push_optional_text_filter(
        builder,
        "requested_model",
        filters.requested_model.as_deref(),
    );
    push_optional_text_filter(builder, "resolved_model", filters.resolved_model.as_deref());
    push_optional_text_filter(builder, "status", filters.status.as_deref());
    push_optional_text_filter(builder, "failure_class", filters.failure_class.as_deref());

    if let Some(created_from) = filters.created_from.as_deref().and_then(trim_nonempty) {
        let parsed = OffsetDateTime::parse(created_from, &Rfc3339).map_err(|error| {
            GatewayError::bad_request(format!("invalid createdFrom timestamp: {error}"))
        })?;
        builder.push(" and created_at >= ");
        builder.push_bind(parsed);
    }
    if let Some(created_to) = filters.created_to.as_deref().and_then(trim_nonempty) {
        let parsed = OffsetDateTime::parse(created_to, &Rfc3339).map_err(|error| {
            GatewayError::bad_request(format!("invalid createdTo timestamp: {error}"))
        })?;
        builder.push(" and created_at <= ");
        builder.push_bind(parsed);
    }

    Ok(())
}

fn push_optional_text_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    column_name: &'static str,
    value: Option<&str>,
) {
    let Some(value) = value.and_then(trim_nonempty) else {
        return;
    };
    builder.push(" and ");
    builder.push(column_name);
    builder.push(" = ");
    builder.push_bind(value.to_string());
}

fn trim_nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn to_view(row: GatewayConversationArchiveRow) -> GatewayConversationArchiveView {
    GatewayConversationArchiveView {
        id: row.id,
        request_audit_id: row.request_audit_id,
        request_id: row.request_id,
        project_id: row.project_id,
        user_id: row.user_id,
        session_id: row.session_id,
        provider_account_id: row.provider_account_id,
        provider_credential_ref: row.provider_credential_ref,
        protocol_family: row.protocol_family,
        protocol_profile: row.protocol_profile,
        endpoint_kind: row.endpoint_kind,
        requested_model: row.requested_model,
        resolved_model: row.resolved_model,
        status: row.status,
        upstream_status: row.upstream_status,
        failure_class: row.failure_class,
        failure_scope: row.failure_scope,
        request_object_key: row.request_object_key,
        response_object_key: row.response_object_key,
        redaction_version: row.redaction_version,
        truncated_request: row.truncated_request,
        truncated_response: row.truncated_response,
        archive_error: row.archive_error,
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}
