use super::manifest::{build_manifest_artifacts, try_read_analysis_export_manifest};
use super::normalization::{
    normalize_export_label, normalize_export_tags, normalize_tag_values, parse_optional_rfc3339,
    trimmed_owned_ref,
};
use super::record_views::to_persisted_analysis_export_view;
use super::*;

pub async fn update_persisted_analysis_export_metadata(
    pool: &PgPool,
    export_id: &str,
    input: GatewayAnalysisExportMetadataUpdateInput,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let Some(export_id) = trimmed_owned_ref(export_id) else {
        return Err(GatewayError::conflict("exportId 不能为空。"));
    };
    let row = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where id = $1
        limit 1
        "#,
    )
    .bind(export_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Gateway analysis export 不存在。"))?;

    if row.status == "deleted" {
        return Err(GatewayError::conflict(
            "已删除的 export 不允许继续修改 metadata。",
        ));
    }

    let next_label = match input.label {
        Some(value) => normalize_export_label(value.as_deref())?,
        None => row.label.clone(),
    };
    let next_tags = match input.tags {
        Some(value) => normalize_export_tags(value.as_ref())?,
        None => normalize_tag_values(&row.tags.0),
    };
    let next_retention_expires_at = match input.retention_expires_at {
        Some(value) => parse_optional_rfc3339(value.as_deref(), "retentionExpiresAt")?,
        None => row.retention_expires_at,
    };

    let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
    let dataset_file = manifest
        .as_ref()
        .and_then(|item| {
            item.files
                .iter()
                .find(|file| file.kind == "dataset_jsonl")
                .cloned()
        })
        .unwrap_or_else(|| GatewayAnalysisExportFileView {
            kind: "dataset_jsonl".to_string(),
            object_key: row.dataset_object_key.clone(),
            content_type: "application/x-ndjson".to_string(),
            size_bytes: 0,
            sha256: String::new(),
            line_count: Some(usize::try_from(row.sample_count).unwrap_or_default()),
        });
    let filters = serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
        .map_err(|error| {
            GatewayError::server_error(format!("parse analysis export filters: {error}"))
        })?;
    let manifest_artifacts = build_manifest_artifacts(
        &row.id,
        next_label.clone(),
        next_tags.clone(),
        format_timestamp(row.created_at),
        next_retention_expires_at.map(format_timestamp),
        filters,
        usize::try_from(row.sample_count).unwrap_or_default(),
        usize::try_from(row.request_artifact_count).unwrap_or_default(),
        usize::try_from(row.response_artifact_count).unwrap_or_default(),
        dataset_file,
        row.manifest_object_key.clone(),
    )?;
    gateway_object_storage()?
        .put_bytes(
            &row.manifest_object_key,
            manifest_artifacts.manifest_body.clone(),
            "application/json",
        )
        .await?;

    let updated_at = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_exports
        set label = $2,
            tags = $3,
            retention_expires_at = $4,
            updated_at = $5
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_label.as_deref())
    .bind(Json(next_tags.clone()))
    .bind(next_retention_expires_at)
    .bind(updated_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated_row = GatewayAnalysisExportRow {
        label: next_label,
        tags: Json(next_tags),
        retention_expires_at: next_retention_expires_at,
        updated_at,
        ..row
    };
    to_persisted_analysis_export_view(updated_row, Some(manifest_artifacts.manifest))
}
