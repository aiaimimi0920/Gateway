use super::diff::build_analysis_export_diff_for_views;
use super::normalization::{trimmed_owned_ref, trimmed_owned_ref_opt};
use super::text::truncate_error_message;
use super::trends::{build_analysis_export_trend_point, build_analysis_export_trend_summary};
use super::*;

pub async fn get_persisted_analysis_export_diff(
    pool: &PgPool,
    left_export_id: &str,
    right_export_id: &str,
) -> Result<GatewayAnalysisExportDiffView, GatewayError> {
    let Some(left_export_id) = trimmed_owned_ref(left_export_id) else {
        return Err(GatewayError::conflict(
            "leftExportId 与 rightExportId 都不能为空。",
        ));
    };
    let Some(right_export_id) = trimmed_owned_ref(right_export_id) else {
        return Err(GatewayError::conflict(
            "leftExportId 与 rightExportId 都不能为空。",
        ));
    };
    let left_export = get_persisted_analysis_export(pool, left_export_id).await?;
    let right_export = get_persisted_analysis_export(pool, right_export_id).await?;
    build_analysis_export_diff_for_views(&left_export, &right_export).await
}

pub async fn get_analysis_export_baseline_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportBaselineReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 2, 50, 10);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let diff = if exports.len() >= 2 {
        build_analysis_export_diff_for_views(&exports[1], &exports[0])
            .await
            .ok()
    } else {
        None
    };
    Ok(GatewayAnalysisExportBaselineReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count: exports.len(),
        latest_export: exports.first().cloned(),
        previous_export: exports.get(1).cloned(),
        inventory_summary,
        diff,
    })
}

pub async fn get_analysis_export_timeline_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportTimelineReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 2, 20, 5);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let mut pair_comparisons = Vec::new();
    for index in 0..exports.len().saturating_sub(1) {
        let Some(newer_export) = exports.get(index).cloned() else {
            continue;
        };
        let Some(older_export) = exports.get(index + 1).cloned() else {
            continue;
        };
        match build_analysis_export_diff_for_views(&older_export, &newer_export).await {
            Ok(diff) => pair_comparisons.push(GatewayAnalysisExportTimelinePairView {
                newer_export,
                older_export,
                diff: Some(diff),
                diff_unavailable_reason: None,
            }),
            Err(error) => pair_comparisons.push(GatewayAnalysisExportTimelinePairView {
                newer_export,
                older_export,
                diff: None,
                diff_unavailable_reason: Some(truncate_error_message(&error.message, 240)),
            }),
        }
    }
    Ok(GatewayAnalysisExportTimelineReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count: exports.len(),
        window_size: normalized_filters.limit.unwrap_or(5),
        exports,
        inventory_summary,
        pair_comparisons,
    })
}

pub async fn get_analysis_export_trend_report(
    pool: &PgPool,
    filters: &GatewayPersistedAnalysisExportFilters,
) -> Result<GatewayAnalysisExportTrendReportView, GatewayError> {
    let normalized_filters = normalize_analysis_export_report_filters(filters, 1, 50, 10);
    let exports = list_persisted_analysis_exports(pool, &normalized_filters).await?;
    let inventory_summary = summarize_persisted_analysis_exports(pool, &normalized_filters).await?;
    let mut points = Vec::with_capacity(exports.len());
    for export in &exports {
        points.push(build_analysis_export_trend_point(export).await);
    }
    let matched_exports_count = points.len();
    let window_size = normalized_filters.limit.unwrap_or(10);
    Ok(GatewayAnalysisExportTrendReportView {
        generated_at: format_timestamp(OffsetDateTime::now_utc()),
        filters: to_baseline_report_filter_view(&normalized_filters),
        matched_exports_count,
        window_size,
        inventory_summary,
        summary: build_analysis_export_trend_summary(&points),
        points,
    })
}

fn normalize_analysis_export_report_filters(
    filters: &GatewayPersistedAnalysisExportFilters,
    min_limit: usize,
    max_limit: usize,
    default_limit: usize,
) -> GatewayPersistedAnalysisExportFilters {
    GatewayPersistedAnalysisExportFilters {
        export_id: None,
        label: filters.label.clone(),
        tag: filters.tag.clone(),
        project_id: filters.project_id.clone(),
        status: Some(
            trimmed_owned_ref_opt(filters.status.as_deref())
                .unwrap_or("active")
                .to_string(),
        ),
        text_mode: filters.text_mode.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit: Some(
            filters
                .limit
                .unwrap_or(default_limit)
                .clamp(min_limit, max_limit),
        ),
    }
}

fn to_baseline_report_filter_view(
    filters: &GatewayPersistedAnalysisExportFilters,
) -> GatewayAnalysisExportBaselineReportFilterView {
    GatewayAnalysisExportBaselineReportFilterView {
        label: filters.label.clone(),
        tag: filters.tag.clone(),
        project_id: filters.project_id.clone(),
        status: filters.status.clone(),
        text_mode: filters.text_mode.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
    }
}
