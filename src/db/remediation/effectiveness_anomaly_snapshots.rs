use super::effectiveness_anomalies::normalize_profile_key;
use super::*;

pub async fn persist_anomaly_remediation_effectiveness_anomaly_snapshot(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
    profile_key: Option<&str>,
    overrides: GatewayAnalysisAnomalyRemediationEffectivenessAnomalyOverrides,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let lookback_hours = lookback_hours.map(|value| value.clamp(0, 24 * 365));
    let created_from = if filters.created_from.is_some() {
        filters.created_from.clone()
    } else if let Some(hours) = lookback_hours {
        Some(format_timestamp(
            timestamp - time::Duration::hours(i64::from(hours)),
        ))
    } else {
        None
    };
    let limit = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let report = get_anomaly_remediation_effectiveness_snapshot_anomaly_report(
        &normalized_filters,
        profile_key,
        overrides,
    )
    .await?;

    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let object_key = build_remediation_effectiveness_anomaly_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView {
        snapshot_id: snapshot_id.clone(),
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilterView {
            label: normalized_filters.label,
            route_policy_id: normalized_filters.route_policy_id,
            action_key: normalized_filters.action_key,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
            profile_key: report.profile_key.clone(),
        },
        report,
    };

    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize remediation effectiveness anomaly snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_anomaly_remediation_effectiveness_anomaly_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
) -> Result<Vec<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects("ai-gateway/remediation-effectiveness-anomaly-snapshots")
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys {
        if !object_key.ends_with("/snapshot.json") {
            continue;
        }
        let snapshot = gateway_object_storage()?
            .read_json(&object_key)
            .await
            .ok()
            .and_then(|value| {
                serde_json::from_value::<
                    GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
                >(value)
                .ok()
            });
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_remediation_effectiveness_anomaly_snapshot_filters(
            &snapshot,
            filters,
            created_from,
            created_to,
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn get_anomaly_remediation_effectiveness_anomaly_snapshot(
    snapshot_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(snapshot_id)
        .ok_or_else(|| GatewayError::bad_request("snapshotId 不能为空"))?;
    let object_key = build_remediation_effectiveness_anomaly_snapshot_object_key(snapshot_id);
    let value = gateway_object_storage()?
        .read_json(&object_key)
        .await
        .map_err(|_| {
            GatewayError::not_found("Gateway remediation effectiveness anomaly snapshot 不存在")
        })?;
    serde_json::from_value(value).map_err(|_| {
        GatewayError::not_found("Gateway remediation effectiveness anomaly snapshot 不存在")
    })
}

pub(super) fn matches_remediation_effectiveness_anomaly_snapshot_filters(
    snapshot: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotView,
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessAnomalySnapshotFilters,
    created_from: Option<OffsetDateTime>,
    created_to: Option<OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = filters.snapshot_id.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = filters.label.as_deref().and_then(trimmed_owned_ref) {
        let needle = label.to_lowercase();
        let haystack = snapshot.label.as_deref().unwrap_or_default().to_lowercase();
        if !haystack.contains(&needle) {
            return false;
        }
    }
    if let Some(route_policy_id) = filters
        .route_policy_id
        .as_deref()
        .and_then(trimmed_owned_ref)
    {
        if snapshot.filters.route_policy_id.as_deref() != Some(route_policy_id) {
            return false;
        }
    }
    if let Some(action_key) = filters.action_key.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.filters.action_key.as_deref() != Some(action_key) {
            return false;
        }
    }
    if let Some(profile_key) = filters.profile_key.as_deref().and_then(trimmed_owned_ref) {
        if snapshot.filters.profile_key != normalize_profile_key(Some(profile_key)) {
            return false;
        }
    }
    let created_at = OffsetDateTime::parse(&snapshot.created_at, &Rfc3339).ok();
    if let (Some(created_from), Some(created_at)) = (created_from, created_at) {
        if created_at < created_from {
            return false;
        }
    }
    if let (Some(created_to), Some(created_at)) = (created_to, created_at) {
        if created_at > created_to {
            return false;
        }
    }
    true
}

fn build_remediation_effectiveness_anomaly_snapshot_object_key(snapshot_id: &str) -> String {
    format!("ai-gateway/remediation-effectiveness-anomaly-snapshots/{snapshot_id}/snapshot.json")
}
