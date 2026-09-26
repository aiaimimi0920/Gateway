use super::normalization::{normalize_tag_values, trimmed_owned_ref_opt};
use super::record_views::build_synthetic_analysis_export_manifest;
use super::*;

pub(super) fn matches_persisted_analysis_export_filters(
    row: &GatewayAnalysisExportRow,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    let filter_view =
        match serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone()) {
            Ok(filters) => filters,
            Err(_) => return false,
        };
    let item = GatewayPersistedAnalysisExportView {
        export_id: row.id.clone(),
        label: row.label.clone(),
        tags: row.tags.0.clone(),
        status: row.status.clone(),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        object_prefix: row.object_prefix.clone(),
        filters: filter_view,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        cleaned_up_at: row.cleaned_up_at.map(format_timestamp),
        last_cleanup_error: row.last_cleanup_error.clone(),
        files: Vec::new(),
        manifest: build_synthetic_analysis_export_manifest(
            row,
            serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
                .unwrap_or(GatewayAnalysisExportFilterView {
                    project_id: None,
                    route_policy_id: None,
                    provider_account_id: None,
                    session_id: None,
                    api_key_id: None,
                    response_id: None,
                    protocol_family: None,
                    status: None,
                    endpoint_kind: None,
                    stream: None,
                    error_code: None,
                    fallback_eligible: None,
                    created_from: None,
                    created_to: None,
                    artifact_available: None,
                    limit: DEFAULT_EXPORT_LIMIT,
                    text_mode: DEFAULT_TEXT_MODE.to_string(),
                    max_text_chars: DEFAULT_MAX_TEXT_CHARS,
                }),
        ),
    };
    matches_persisted_analysis_export_view_filters(&item, filters, created_from, created_to)
}

pub(super) fn matches_persisted_analysis_export_view_filters(
    item: &GatewayPersistedAnalysisExportView,
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    if let Some(label) = trimmed_owned_ref_opt(filters.label.as_deref()) {
        let haystack = item
            .label
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if !haystack.contains(&label.to_lowercase()) {
            return false;
        }
    }
    if let Some(tag) = trimmed_owned_ref_opt(filters.tag.as_deref()) {
        let tag = tag.to_lowercase();
        if !normalize_tag_values(&item.tags)
            .iter()
            .any(|value| value == &tag)
        {
            return false;
        }
    }
    if let Some(project_id) = trimmed_owned_ref_opt(filters.project_id.as_deref()) {
        if item.filters.project_id.as_deref() != Some(project_id) {
            return false;
        }
    }
    if let Some(status) = trimmed_owned_ref_opt(filters.status.as_deref()) {
        if item.status.trim() != status {
            return false;
        }
    }
    if let Some(text_mode) = trimmed_owned_ref_opt(filters.text_mode.as_deref()) {
        if item.filters.text_mode.trim() != text_mode {
            return false;
        }
    }
    let created_at = match OffsetDateTime::parse(&item.created_at, &Rfc3339) {
        Ok(timestamp) => timestamp,
        Err(_) => return false,
    };
    if let Some(created_from) = created_from {
        if created_at < created_from {
            return false;
        }
    }
    if let Some(created_to) = created_to {
        if created_at > created_to {
            return false;
        }
    }
    true
}
