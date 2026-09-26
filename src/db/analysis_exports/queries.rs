use super::filters::matches_persisted_analysis_export_filters;
use super::inventory::build_analysis_export_inventory_summary;
use super::manifest::{list_fallback_analysis_export_manifests, try_read_analysis_export_manifest};
use super::normalization::{
    is_non_active_status_filter, parse_optional_rfc3339, trimmed_owned_ref, trimmed_owned_ref_opt,
    validate_created_range,
};
use super::record_views::{
    to_manifest_backed_persisted_analysis_export_view, to_persisted_analysis_export_view,
};
use super::*;

pub async fn list_persisted_analysis_exports(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<Vec<GatewayPersistedAnalysisExportView>, GatewayError> {
    let created_from = parse_optional_rfc3339(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_rfc3339(filters.created_to.as_deref(), "createdTo")?;
    validate_created_range(created_from, created_to)?;
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let rows = query_persisted_analysis_export_rows(
        pool,
        filters,
        created_from,
        created_to,
        Some(limit * 2),
    )
    .await?;
    let matched_rows = rows
        .into_iter()
        .filter(|row| {
            matches_persisted_analysis_export_filters(row, filters, created_from, created_to)
        })
        .collect::<Vec<_>>();

    if !matched_rows.is_empty() {
        let mut exports = Vec::with_capacity(matched_rows.len().min(limit));
        for row in matched_rows.into_iter().take(limit) {
            let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
            exports.push(to_persisted_analysis_export_view(row, manifest)?);
        }
        return Ok(exports);
    }

    if is_non_active_status_filter(filters.status.as_deref())
        || trimmed_owned_ref_opt(filters.tag.as_deref()).is_some()
    {
        return Ok(Vec::new());
    }

    let mut fallback =
        list_fallback_analysis_export_manifests(filters, created_from, created_to).await?;
    fallback.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    fallback.truncate(limit);
    Ok(fallback)
}

pub async fn get_persisted_analysis_export(
    pool: &PgPool,
    export_id: &str,
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
    .map_err(map_db_error)?;

    if let Some(row) = row {
        let manifest = try_read_analysis_export_manifest(&row.manifest_object_key).await;
        return to_persisted_analysis_export_view(row, manifest);
    }

    let manifest_object_key = build_gateway_analysis_export_manifest_object_key(export_id);
    let manifest = try_read_analysis_export_manifest(&manifest_object_key)
        .await
        .ok_or_else(|| GatewayError::not_found("Gateway analysis export 不存在。"))?;
    Ok(to_manifest_backed_persisted_analysis_export_view(manifest))
}

pub async fn summarize_persisted_analysis_exports(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportInventorySummaryView, GatewayError> {
    let created_from = parse_optional_rfc3339(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_optional_rfc3339(filters.created_to.as_deref(), "createdTo")?;
    validate_created_range(created_from, created_to)?;
    let rows =
        query_persisted_analysis_export_rows(pool, filters, created_from, created_to, None).await?;
    let row_count = rows.len();

    let persisted = rows
        .into_iter()
        .filter(|row| {
            matches_persisted_analysis_export_filters(row, filters, created_from, created_to)
        })
        .map(|row| to_persisted_analysis_export_view(row, None))
        .collect::<Result<Vec<_>, _>>()?;

    if !persisted.is_empty()
        || row_count > 0
        || trimmed_owned_ref_opt(filters.status.as_deref()).is_some()
        || trimmed_owned_ref_opt(filters.tag.as_deref()).is_some()
    {
        return Ok(build_analysis_export_inventory_summary(
            &persisted,
            OffsetDateTime::now_utc(),
        ));
    }

    let fallback =
        list_fallback_analysis_export_manifests(filters, created_from, created_to).await?;
    Ok(build_analysis_export_inventory_summary(
        &fallback,
        OffsetDateTime::now_utc(),
    ))
}

async fn query_persisted_analysis_export_rows(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
    limit: Option<usize>,
) -> Result<Vec<GatewayAnalysisExportRow>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, project_id, label, tags, status, text_mode, max_text_chars, filters,
          object_prefix, manifest_object_key, dataset_object_key, sample_count,
          request_artifact_count, response_artifact_count, retention_expires_at,
          cleaned_up_at, last_cleanup_error, created_at, updated_at
        from gateway_analysis_exports
        where 1 = 1
        "#,
    );
    if let Some(export_id) = trimmed_owned_ref_opt(filters.export_id.as_deref()) {
        builder.push(" and id = ").push_bind(export_id);
    }
    if let Some(project_id) = trimmed_owned_ref_opt(filters.project_id.as_deref()) {
        builder.push(" and project_id = ").push_bind(project_id);
    }
    if let Some(status) = trimmed_owned_ref_opt(filters.status.as_deref()) {
        builder.push(" and status = ").push_bind(status);
    }
    if let Some(text_mode) = trimmed_owned_ref_opt(filters.text_mode.as_deref()) {
        builder.push(" and text_mode = ").push_bind(text_mode);
    }
    if let Some(created_from) = created_from {
        builder.push(" and created_at >= ").push_bind(created_from);
    }
    if let Some(created_to) = created_to {
        builder.push(" and created_at <= ").push_bind(created_to);
    }
    builder.push(" order by created_at desc");
    if let Some(limit) = limit {
        builder
            .push(" limit ")
            .push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
    }
    builder
        .build_query_as::<GatewayAnalysisExportRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}
