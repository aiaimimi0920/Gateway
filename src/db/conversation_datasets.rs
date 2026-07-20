use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

use crate::conversation_dataset::can_publish_dataset;
use crate::error::GatewayError;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone)]
pub struct CreateConversationDatasetExportInput {
    pub id: String,
    pub filter: Value,
    pub sample_size: Option<i32>,
    pub row_count: i32,
    pub dataset_object_key: String,
    pub manifest_object_key: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConversationDatasetExportInput {
    pub action: String,
    pub reviewer_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConversationDatasetExportView {
    pub id: String,
    pub status: String,
    pub filter: Value,
    pub sample_size: Option<i32>,
    pub row_count: i32,
    pub dataset_object_key: String,
    pub manifest_object_key: String,
    pub created_by: Option<String>,
    pub reviewer_id: Option<String>,
    pub approval_note: Option<String>,
    pub rejected_reason: Option<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayConversationDatasetExportRow {
    id: String,
    status: String,
    filter: Json<Value>,
    sample_size: Option<i32>,
    row_count: i32,
    dataset_object_key: String,
    manifest_object_key: String,
    created_by: Option<String>,
    reviewer_id: Option<String>,
    approval_note: Option<String>,
    rejected_reason: Option<String>,
    published_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub async fn create_conversation_dataset_export(
    pool: &PgPool,
    input: CreateConversationDatasetExportInput,
) -> Result<GatewayConversationDatasetExportView, GatewayError> {
    let row = sqlx::query_as::<_, GatewayConversationDatasetExportRow>(
        r#"
        insert into gateway_conversation_dataset_exports (
          id,
          status,
          filter,
          sample_size,
          row_count,
          dataset_object_key,
          manifest_object_key,
          created_by,
          created_at,
          updated_at
        ) values (
          $1, 'review_pending', $2, $3, $4, $5, $6, $7, now(), now()
        )
        returning *
        "#,
    )
    .bind(input.id)
    .bind(Json(input.filter))
    .bind(input.sample_size)
    .bind(input.row_count)
    .bind(input.dataset_object_key)
    .bind(input.manifest_object_key)
    .bind(input.created_by)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_view(row))
}

pub async fn list_conversation_dataset_exports(
    pool: &PgPool,
) -> Result<Vec<GatewayConversationDatasetExportView>, GatewayError> {
    let rows = sqlx::query_as::<_, GatewayConversationDatasetExportRow>(
        r#"
        select *
        from gateway_conversation_dataset_exports
        order by created_at desc
        limit 200
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_view).collect())
}

pub async fn get_conversation_dataset_export(
    pool: &PgPool,
    dataset_id: &str,
) -> Result<Option<GatewayConversationDatasetExportView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayConversationDatasetExportRow>(
        r#"
        select *
        from gateway_conversation_dataset_exports
        where id = $1
        "#,
    )
    .bind(dataset_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(to_view))
}

pub async fn review_conversation_dataset_export(
    pool: &PgPool,
    dataset_id: &str,
    input: ReviewConversationDatasetExportInput,
) -> Result<GatewayConversationDatasetExportView, GatewayError> {
    let action = input.action.trim();
    let (status, approval_note, rejected_reason) = match action {
        "approve" => ("approved", input.note.clone(), None),
        "reject" => ("rejected", None, input.note.clone()),
        _ => {
            return Err(GatewayError::bad_request(
                "dataset review action must be approve or reject",
            ))
        }
    };

    let row = sqlx::query_as::<_, GatewayConversationDatasetExportRow>(
        r#"
        update gateway_conversation_dataset_exports
        set
          status = $2,
          reviewer_id = $3,
          approval_note = $4,
          rejected_reason = $5,
          updated_at = now()
        where id = $1
        returning *
        "#,
    )
    .bind(dataset_id)
    .bind(status)
    .bind(input.reviewer_id)
    .bind(approval_note)
    .bind(rejected_reason)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_view(row))
}

pub async fn publish_conversation_dataset_export(
    pool: &PgPool,
    dataset_id: &str,
) -> Result<GatewayConversationDatasetExportView, GatewayError> {
    let current = get_conversation_dataset_export(pool, dataset_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("conversation dataset export not found"))?;
    if !can_publish_dataset(&current.status) {
        return Err(GatewayError::conflict(
            "conversation dataset export must be approved before publish",
        ));
    }

    let row = sqlx::query_as::<_, GatewayConversationDatasetExportRow>(
        r#"
        update gateway_conversation_dataset_exports
        set status = 'published',
            published_at = now(),
            updated_at = now()
        where id = $1
        returning *
        "#,
    )
    .bind(dataset_id)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_view(row))
}

fn to_view(row: GatewayConversationDatasetExportRow) -> GatewayConversationDatasetExportView {
    GatewayConversationDatasetExportView {
        id: row.id,
        status: row.status,
        filter: row.filter.0,
        sample_size: row.sample_size,
        row_count: row.row_count,
        dataset_object_key: row.dataset_object_key,
        manifest_object_key: row.manifest_object_key,
        created_by: row.created_by,
        reviewer_id: row.reviewer_id,
        approval_note: row.approval_note,
        rejected_reason: row.rejected_reason,
        published_at: row.published_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}
