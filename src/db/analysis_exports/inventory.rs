use super::normalization::{has_pinned_tag, trimmed_owned_ref};
use super::*;

pub(super) fn build_analysis_export_inventory_summary(
    exports: &[GatewayPersistedAnalysisExportView],
    now: OffsetDateTime,
) -> GatewayAnalysisExportInventorySummaryView {
    let expiring_threshold = now + time::Duration::hours(24);
    let mut by_status = HashMap::new();
    let mut by_text_mode = HashMap::new();
    let mut by_tag = HashMap::new();
    let mut by_project = HashMap::new();
    let mut active_exports = 0usize;
    let mut deleted_exports = 0usize;
    let mut pinned_exports = 0usize;
    let mut expiring_within_24_hours = 0usize;
    let mut expired_active_exports = 0usize;
    let mut total_sample_count = 0usize;
    let mut total_request_artifact_count = 0usize;
    let mut total_response_artifact_count = 0usize;

    for item in exports {
        total_sample_count += item.sample_count;
        total_request_artifact_count += item.request_artifact_count;
        total_response_artifact_count += item.response_artifact_count;
        accumulate_bucket(&mut by_status, &item.status);
        accumulate_bucket(&mut by_text_mode, &item.filters.text_mode);
        if let Some(project_id) = item.filters.project_id.as_deref() {
            accumulate_bucket(&mut by_project, project_id);
        }
        for tag in &item.tags {
            accumulate_bucket(&mut by_tag, tag);
        }

        match item.status.as_str() {
            "active" => active_exports += 1,
            "deleted" => deleted_exports += 1,
            _ => {}
        }
        if has_pinned_tag(&item.tags) {
            pinned_exports += 1;
        }
        if item.status == "active" {
            if let Some(retention_expires_at) = item.retention_expires_at.as_deref() {
                if let Ok(expires_at) = OffsetDateTime::parse(retention_expires_at, &Rfc3339) {
                    if expires_at <= now {
                        expired_active_exports += 1;
                    } else if expires_at <= expiring_threshold {
                        expiring_within_24_hours += 1;
                    }
                }
            }
        }
    }

    GatewayAnalysisExportInventorySummaryView {
        total_exports: exports.len(),
        active_exports,
        deleted_exports,
        pinned_exports,
        expiring_within_24_hours,
        expired_active_exports,
        total_sample_count,
        total_request_artifact_count,
        total_response_artifact_count,
        by_status: into_key_buckets(by_status),
        by_text_mode: into_key_buckets(by_text_mode),
        by_tag: into_key_buckets(by_tag),
        by_project: into_key_buckets(by_project),
    }
}

pub(super) fn accumulate_bucket(buckets: &mut std::collections::HashMap<String, usize>, key: &str) {
    let Some(key) = trimmed_owned_ref(key) else {
        return;
    };
    *buckets.entry(key.to_string()).or_default() += 1;
}

pub(super) fn into_key_buckets(
    buckets: std::collections::HashMap<String, usize>,
) -> Vec<GatewaySummaryBucketKeyView> {
    let mut items = buckets
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    items
}
