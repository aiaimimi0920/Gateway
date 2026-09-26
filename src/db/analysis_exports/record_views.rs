use super::*;

pub(super) fn to_manifest_backed_persisted_analysis_export_view(
    manifest: GatewayAnalysisExportManifest,
) -> GatewayPersistedAnalysisExportView {
    GatewayPersistedAnalysisExportView {
        export_id: manifest.export_id.clone(),
        label: manifest.label.clone(),
        tags: Vec::new(),
        status: "active".to_string(),
        created_at: manifest.created_at.clone(),
        updated_at: manifest.created_at.clone(),
        object_prefix: build_gateway_analysis_export_prefix(&manifest.export_id),
        filters: manifest.filters.clone(),
        sample_count: manifest.sample_count,
        request_artifact_count: manifest.request_artifact_count,
        response_artifact_count: manifest.response_artifact_count,
        retention_expires_at: None,
        cleaned_up_at: None,
        last_cleanup_error: None,
        files: manifest.files.clone(),
        manifest,
    }
}

pub(super) fn build_synthetic_analysis_export_manifest(
    row: &GatewayAnalysisExportRow,
    filters: GatewayAnalysisExportFilterView,
) -> GatewayAnalysisExportManifest {
    GatewayAnalysisExportManifest {
        schema_version: 1,
        export_id: row.id.clone(),
        label: row.label.clone(),
        tags: row.tags.0.clone(),
        created_at: format_timestamp(row.created_at),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        filters,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        files: Vec::new(),
    }
}

pub(super) fn to_persisted_analysis_export_view(
    row: GatewayAnalysisExportRow,
    manifest: Option<GatewayAnalysisExportManifest>,
) -> Result<GatewayPersistedAnalysisExportView, GatewayError> {
    let filters = serde_json::from_value::<GatewayAnalysisExportFilterView>(row.filters.0.clone())
        .map_err(|error| {
            GatewayError::server_error(format!("parse analysis export filters: {error}"))
        })?;
    let manifest = match manifest {
        Some(mut manifest) => {
            manifest.label = row.label.clone();
            manifest.tags = row.tags.0.clone();
            manifest.filters = filters.clone();
            manifest.sample_count = usize::try_from(row.sample_count).unwrap_or_default();
            manifest.request_artifact_count =
                usize::try_from(row.request_artifact_count).unwrap_or_default();
            manifest.response_artifact_count =
                usize::try_from(row.response_artifact_count).unwrap_or_default();
            manifest.retention_expires_at = row.retention_expires_at.map(format_timestamp);
            manifest
        }
        None => build_synthetic_analysis_export_manifest(&row, filters.clone()),
    };
    Ok(GatewayPersistedAnalysisExportView {
        export_id: row.id,
        label: row.label,
        tags: row.tags.0,
        status: row.status,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        object_prefix: row.object_prefix,
        filters,
        sample_count: usize::try_from(row.sample_count).unwrap_or_default(),
        request_artifact_count: usize::try_from(row.request_artifact_count).unwrap_or_default(),
        response_artifact_count: usize::try_from(row.response_artifact_count).unwrap_or_default(),
        retention_expires_at: row.retention_expires_at.map(format_timestamp),
        cleaned_up_at: row.cleaned_up_at.map(format_timestamp),
        last_cleanup_error: row.last_cleanup_error,
        files: manifest.files.clone(),
        manifest,
    })
}
