use super::dataset::read_analysis_export_dataset_for_export;
use super::normalization::trimmed_owned_ref;
use super::*;

pub(super) async fn build_analysis_export_diff_for_views(
    left_export: &GatewayPersistedAnalysisExportView,
    right_export: &GatewayPersistedAnalysisExportView,
) -> Result<GatewayAnalysisExportDiffView, GatewayError> {
    let left_rows = read_analysis_export_dataset_for_export(left_export)
        .await
        .map_err(|_| {
            GatewayError::conflict(format!(
                "leftExportId={} 的 dataset.jsonl 不可用，无法执行 diff。",
                left_export.export_id
            ))
        })?;
    let right_rows = read_analysis_export_dataset_for_export(right_export)
        .await
        .map_err(|_| {
            GatewayError::conflict(format!(
                "rightExportId={} 的 dataset.jsonl 不可用，无法执行 diff。",
                right_export.export_id
            ))
        })?;
    Ok(build_analysis_export_diff(
        left_export.clone(),
        right_export.clone(),
        &left_rows,
        &right_rows,
    ))
}

fn build_analysis_export_diff(
    left_export: GatewayPersistedAnalysisExportView,
    right_export: GatewayPersistedAnalysisExportView,
    left_rows: &[GatewayAnalysisExportRowView],
    right_rows: &[GatewayAnalysisExportRowView],
) -> GatewayAnalysisExportDiffView {
    let left_request_ids = left_rows
        .iter()
        .map(|row| row.request_audit_id.clone())
        .collect::<HashSet<_>>();
    let right_request_ids = right_rows
        .iter()
        .map(|row| row.request_audit_id.clone())
        .collect::<HashSet<_>>();
    let overlap_request_count = left_request_ids
        .iter()
        .filter(|request_id| right_request_ids.contains(*request_id))
        .count();
    GatewayAnalysisExportDiffView {
        left_export,
        right_export,
        overlap_request_count,
        left_only_request_count: left_request_ids.len().saturating_sub(overlap_request_count),
        right_only_request_count: right_request_ids
            .len()
            .saturating_sub(overlap_request_count),
        sample_count: build_metric_delta(
            Some(left_rows.len() as f64),
            Some(right_rows.len() as f64),
        ),
        request_artifact_count: build_metric_delta(
            Some(
                left_rows
                    .iter()
                    .filter(|row| row.request_artifact_available)
                    .count() as f64,
            ),
            Some(
                right_rows
                    .iter()
                    .filter(|row| row.request_artifact_available)
                    .count() as f64,
            ),
        ),
        response_artifact_count: build_metric_delta(
            Some(
                left_rows
                    .iter()
                    .filter(|row| row.response_artifact_available)
                    .count() as f64,
            ),
            Some(
                right_rows
                    .iter()
                    .filter(|row| row.response_artifact_available)
                    .count() as f64,
            ),
        ),
        prompt_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.prompt_tokens),
            sum_metric(right_rows, |row| row.prompt_tokens),
        ),
        completion_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.completion_tokens),
            sum_metric(right_rows, |row| row.completion_tokens),
        ),
        total_tokens: build_metric_delta(
            sum_metric(left_rows, |row| row.total_tokens),
            sum_metric(right_rows, |row| row.total_tokens),
        ),
        by_status: build_bucket_delta(left_rows, right_rows, |row| Some(row.status.as_str())),
        by_protocol_family: build_bucket_delta(left_rows, right_rows, |row| {
            Some(row.protocol_family.as_str())
        }),
        by_endpoint_kind: build_bucket_delta(left_rows, right_rows, |row| {
            Some(row.endpoint_kind.as_str())
        }),
        by_resolved_model: build_bucket_delta(left_rows, right_rows, |row| {
            row.resolved_model.as_deref()
        }),
        by_provider_account: build_bucket_delta(left_rows, right_rows, |row| {
            row.provider_account_id.as_deref()
        }),
    }
}

fn build_metric_delta(
    left_value: Option<f64>,
    right_value: Option<f64>,
) -> GatewayAnalysisExportMetricDeltaView {
    GatewayAnalysisExportMetricDeltaView {
        left_value,
        right_value,
        delta_value: match (left_value, right_value) {
            (Some(left), Some(right)) => Some(right - left),
            _ => None,
        },
    }
}

fn build_bucket_delta<F>(
    left_rows: &[GatewayAnalysisExportRowView],
    right_rows: &[GatewayAnalysisExportRowView],
    selector: F,
) -> Vec<GatewayAnalysisExportBucketDeltaView>
where
    F: Fn(&GatewayAnalysisExportRowView) -> Option<&str>,
{
    let mut left_counts = HashMap::new();
    let mut right_counts = HashMap::new();

    for row in left_rows {
        if let Some(key) = selector(row).and_then(trimmed_owned_ref) {
            *left_counts.entry(key.to_string()).or_insert(0usize) += 1;
        }
    }
    for row in right_rows {
        if let Some(key) = selector(row).and_then(trimmed_owned_ref) {
            *right_counts.entry(key.to_string()).or_insert(0usize) += 1;
        }
    }

    let mut items = left_counts
        .keys()
        .chain(right_counts.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|key| {
            let left_count = left_counts.get(&key).copied().unwrap_or_default();
            let right_count = right_counts.get(&key).copied().unwrap_or_default();
            GatewayAnalysisExportBucketDeltaView {
                key,
                left_count,
                right_count,
                delta_count: right_count as i64 - left_count as i64,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .delta_count
            .abs()
            .cmp(&left.delta_count.abs())
            .then_with(|| left.key.cmp(&right.key))
    });
    items
}

fn sum_metric<F>(rows: &[GatewayAnalysisExportRowView], selector: F) -> Option<f64>
where
    F: Fn(&GatewayAnalysisExportRowView) -> Option<i32>,
{
    Some(
        rows.iter()
            .map(|row| i64::from(selector(row).unwrap_or_default()))
            .sum::<i64>() as f64,
    )
}
