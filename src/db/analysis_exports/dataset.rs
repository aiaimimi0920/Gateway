use super::*;

pub(super) fn build_analysis_dataset_jsonl(
    rows: &[GatewayAnalysisExportRowView],
) -> Result<Vec<u8>, GatewayError> {
    let mut body = String::new();
    for row in rows {
        let line = serde_json::to_string(row).map_err(|error| {
            GatewayError::server_error(format!("serialize analysis export row: {error}"))
        })?;
        body.push_str(&line);
        body.push('\n');
    }
    Ok(body.into_bytes())
}

pub(super) async fn read_analysis_export_dataset_for_export(
    export: &GatewayPersistedAnalysisExportView,
) -> Result<Vec<GatewayAnalysisExportRowView>, GatewayError> {
    let dataset_object_key = export
        .files
        .iter()
        .find(|file| file.kind == "dataset_jsonl")
        .map(|file| file.object_key.clone())
        .unwrap_or_else(|| build_gateway_analysis_export_dataset_object_key(&export.export_id));
    read_analysis_export_dataset(&dataset_object_key).await
}

async fn read_analysis_export_dataset(
    object_key: &str,
) -> Result<Vec<GatewayAnalysisExportRowView>, GatewayError> {
    let bytes = gateway_object_storage()?.read_bytes(object_key).await?;
    let body = String::from_utf8(bytes).map_err(|error| {
        GatewayError::server_error(format!("parse analysis export dataset utf8: {error}"))
    })?;
    let mut rows = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let row =
            serde_json::from_str::<GatewayAnalysisExportRowView>(trimmed).map_err(|error| {
                GatewayError::server_error(format!("parse analysis export dataset row: {error}"))
            })?;
        rows.push(row);
    }
    Ok(rows)
}
