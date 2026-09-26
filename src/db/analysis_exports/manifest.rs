use super::filters::matches_persisted_analysis_export_view_filters;
use super::normalization::trimmed_owned_ref_opt;
use super::record_views::to_manifest_backed_persisted_analysis_export_view;
use super::*;
use hex::encode as hex_encode;
use sha2::{Digest, Sha256};

pub(super) async fn try_read_analysis_export_manifest(
    object_key: &str,
) -> Option<GatewayAnalysisExportManifest> {
    let storage = gateway_object_storage().ok()?;
    let value = storage.read_json(object_key).await.ok()?;
    serde_json::from_value(value).ok()
}

pub(super) async fn list_fallback_analysis_export_manifests(
    filters: &GatewayPersistedAnalysisExportFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> Result<Vec<GatewayPersistedAnalysisExportView>, GatewayError> {
    let storage = gateway_object_storage()?;
    let manifest_keys = storage
        .list_objects("ai-gateway/analysis-exports")
        .await?
        .into_iter()
        .filter(|key| key.ends_with("/manifest.json"))
        .collect::<Vec<_>>();

    let mut results = Vec::new();
    for manifest_key in manifest_keys {
        let Some(manifest) = try_read_analysis_export_manifest(&manifest_key).await else {
            continue;
        };
        let view = to_manifest_backed_persisted_analysis_export_view(manifest);
        if let Some(export_id) = trimmed_owned_ref_opt(filters.export_id.as_deref()) {
            if view.export_id != export_id {
                continue;
            }
        }
        if !matches_persisted_analysis_export_view_filters(&view, filters, created_from, created_to)
        {
            continue;
        }
        results.push(view);
    }
    Ok(results)
}

fn build_analysis_export_manifest(
    export_id: &str,
    label: Option<String>,
    tags: Vec<String>,
    created_at: String,
    retention_expires_at: Option<String>,
    filters: GatewayAnalysisExportFilterView,
    sample_count: usize,
    request_artifact_count: usize,
    response_artifact_count: usize,
    files: Vec<GatewayAnalysisExportFileView>,
) -> GatewayAnalysisExportManifest {
    GatewayAnalysisExportManifest {
        schema_version: 1,
        export_id: export_id.to_string(),
        label,
        tags,
        created_at,
        retention_expires_at,
        filters,
        sample_count,
        request_artifact_count,
        response_artifact_count,
        files,
    }
}

pub(super) fn build_manifest_artifacts(
    export_id: &str,
    label: Option<String>,
    tags: Vec<String>,
    created_at: String,
    retention_expires_at: Option<String>,
    filters: GatewayAnalysisExportFilterView,
    sample_count: usize,
    request_artifact_count: usize,
    response_artifact_count: usize,
    dataset_file: GatewayAnalysisExportFileView,
    manifest_object_key: String,
) -> Result<ManifestArtifacts, GatewayError> {
    let placeholder_manifest = build_analysis_export_manifest(
        export_id,
        label,
        tags,
        created_at,
        retention_expires_at,
        filters,
        sample_count,
        request_artifact_count,
        response_artifact_count,
        vec![
            dataset_file.clone(),
            GatewayAnalysisExportFileView {
                kind: "manifest".to_string(),
                object_key: manifest_object_key.clone(),
                content_type: "application/json".to_string(),
                size_bytes: 0,
                sha256: String::new(),
                line_count: None,
            },
        ],
    );
    let first_body = serde_json::to_vec_pretty(&placeholder_manifest).map_err(|error| {
        GatewayError::server_error(format!("serialize analysis export manifest: {error}"))
    })?;
    let manifest_file = build_analysis_export_file_view(
        "manifest",
        manifest_object_key,
        "application/json",
        &first_body,
        None,
    );
    let manifest = build_analysis_export_manifest(
        &placeholder_manifest.export_id,
        placeholder_manifest.label,
        placeholder_manifest.tags,
        placeholder_manifest.created_at,
        placeholder_manifest.retention_expires_at,
        placeholder_manifest.filters,
        placeholder_manifest.sample_count,
        placeholder_manifest.request_artifact_count,
        placeholder_manifest.response_artifact_count,
        vec![manifest_file, dataset_file],
    );
    let manifest_body = serde_json::to_vec_pretty(&manifest).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize finalized analysis export manifest: {error}"
        ))
    })?;
    Ok(ManifestArtifacts {
        manifest,
        manifest_body,
    })
}

pub(super) fn build_analysis_export_file_view(
    kind: &str,
    object_key: String,
    content_type: &str,
    body: &[u8],
    line_count: Option<usize>,
) -> GatewayAnalysisExportFileView {
    GatewayAnalysisExportFileView {
        kind: kind.to_string(),
        object_key,
        content_type: content_type.to_string(),
        size_bytes: body.len(),
        sha256: sha256_bytes(body),
        line_count,
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(hasher.finalize())
}
