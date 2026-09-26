use super::normalization::{has_pinned_tag, trimmed_owned_ref};
use super::text::truncate_error_message;
use super::*;

pub async fn cleanup_expired_analysis_exports(
    pool: &PgPool,
    input: GatewayAnalysisExportCleanupInput,
) -> Result<GatewayAnalysisExportCleanupResult, GatewayError> {
    let limit = input.limit.unwrap_or(50).clamp(1, 500);
    let scan_time = OffsetDateTime::now_utc();
    let rows = sqlx::query_as::<_, GatewayAnalysisExportRow>(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where status = 'active'
          and retention_expires_at is not null
          and retention_expires_at <= $1
        order by retention_expires_at asc, created_at asc
        limit $2
        "#,
    )
    .bind(scan_time)
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    let include_pinned = input.include_pinned.unwrap_or(false);
    let dry_run = input.dry_run.unwrap_or(false);
    let rows = rows
        .into_iter()
        .filter(|row| include_pinned || !has_pinned_tag(&row.tags.0))
        .take(limit)
        .collect::<Vec<_>>();
    let storage = gateway_object_storage()?;
    let mut results = Vec::with_capacity(rows.len());

    for row in &rows {
        let listed_keys = storage
            .list_objects(&row.object_prefix)
            .await
            .unwrap_or_default();
        let mut keys_to_delete = vec![
            row.manifest_object_key.clone(),
            row.dataset_object_key.clone(),
        ];
        keys_to_delete.extend(listed_keys);
        keys_to_delete.sort();
        keys_to_delete.dedup();
        keys_to_delete.retain(|value| trimmed_owned_ref(value).is_some());

        if dry_run {
            results.push(GatewayAnalysisExportCleanupEntryView {
                export_id: row.id.clone(),
                status: "deleted".to_string(),
                deleted_object_count: keys_to_delete.len(),
                error_message: None,
            });
            continue;
        }

        let mut error_message = None;
        for object_key in &keys_to_delete {
            if let Err(error) = storage.delete_object(object_key).await {
                error_message = Some(truncate_error_message(&error.message, 500));
                break;
            }
        }

        if let Some(error_message) = error_message {
            sqlx::query(
                r#"
                update gateway_analysis_exports
                set last_cleanup_error = $2,
                    updated_at = $3
                where id = $1
                "#,
            )
            .bind(&row.id)
            .bind(&error_message)
            .bind(scan_time)
            .execute(pool)
            .await
            .map_err(map_db_error)?;
            results.push(GatewayAnalysisExportCleanupEntryView {
                export_id: row.id.clone(),
                status: "failed".to_string(),
                deleted_object_count: 0,
                error_message: Some(error_message),
            });
            continue;
        }

        sqlx::query(
            r#"
            update gateway_analysis_exports
            set status = 'deleted',
                cleaned_up_at = $2,
                last_cleanup_error = null,
                updated_at = $2
            where id = $1
            "#,
        )
        .bind(&row.id)
        .bind(scan_time)
        .execute(pool)
        .await
        .map_err(map_db_error)?;
        results.push(GatewayAnalysisExportCleanupEntryView {
            export_id: row.id.clone(),
            status: "deleted".to_string(),
            deleted_object_count: keys_to_delete.len(),
            error_message: None,
        });
    }

    Ok(GatewayAnalysisExportCleanupResult {
        scanned_count: rows.len(),
        deleted_count: results
            .iter()
            .filter(|entry| entry.status == "deleted")
            .count(),
        failed_count: results
            .iter()
            .filter(|entry| entry.status == "failed")
            .count(),
        results,
    })
}
