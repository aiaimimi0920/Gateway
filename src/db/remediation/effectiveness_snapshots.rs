use super::impact::normalize_window_minutes;
use super::*;

pub async fn persist_anomaly_remediation_effectiveness_snapshot(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
    label: Option<&str>,
    window_minutes: Option<i32>,
    lookback_hours: Option<i32>,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let window_minutes = normalize_window_minutes(window_minutes);
    let lookback_hours = lookback_hours.map(|value| value.clamp(0, 24 * 365));
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let created_from = if filters.created_from.is_some() {
        filters.created_from.clone()
    } else if let Some(hours) = lookback_hours {
        Some(format_timestamp(
            timestamp - time::Duration::hours(i64::from(hours)),
        ))
    } else {
        None
    };

    let normalized_filters = GatewayAnalysisAnomalyRemediationRunFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let summary =
        get_anomaly_remediation_effectiveness(pool, &normalized_filters, Some(window_minutes))
            .await?;

    let snapshot_id = uuid::Uuid::new_v4().to_string();
    let object_key = build_remediation_effectiveness_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView {
        snapshot_id: snapshot_id.clone(),
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilterView {
            incident_id: normalized_filters.incident_id,
            policy_id: normalized_filters.policy_id,
            route_policy_id: normalized_filters.route_policy_id,
            action_key: normalized_filters.action_key,
            status: normalized_filters.status,
            execution_mode: normalized_filters.execution_mode,
            dry_run: normalized_filters.dry_run,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
            window_minutes,
        },
        summary,
    };

    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize remediation effectiveness snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_anomaly_remediation_effectiveness_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<Vec<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects("ai-gateway/remediation-effectiveness-snapshots")
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
                        GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
                    >(value)
                    .ok()
            });
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_remediation_effectiveness_snapshot_filters(
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

pub async fn summarize_anomaly_remediation_effectiveness_snapshots(
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView, GatewayError>
{
    let snapshots = list_anomaly_remediation_effectiveness_snapshots(
        &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters {
            limit: Some(filters.limit.unwrap_or(500).clamp(1, 500)),
            ..filters.clone()
        },
    )
    .await?;
    Ok(build_remediation_effectiveness_snapshot_inventory_summary(
        &snapshots,
    ))
}

pub async fn get_anomaly_remediation_effectiveness_snapshot(
    snapshot_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(snapshot_id)
        .ok_or_else(|| GatewayError::bad_request("snapshotId 不能为空"))?;
    let object_key = build_remediation_effectiveness_snapshot_object_key(snapshot_id);
    let value = gateway_object_storage()?
        .read_json(&object_key)
        .await
        .map_err(|_| {
            GatewayError::not_found("Gateway remediation effectiveness snapshot 不存在")
        })?;
    serde_json::from_value(value)
        .map_err(|_| GatewayError::not_found("Gateway remediation effectiveness snapshot 不存在"))
}

fn build_remediation_effectiveness_snapshot_inventory_summary(
    snapshots: &[GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView],
) -> GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
    let mut by_route_policy_id = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_label = BTreeMap::new();
    let mut total_runs = 0;
    let mut total_impacted_runs = 0;
    let mut total_unavailable_runs = 0;

    for snapshot in snapshots {
        total_runs += snapshot.summary.total_runs;
        total_impacted_runs += snapshot.summary.impacted_runs;
        total_unavailable_runs += snapshot.summary.unavailable_runs;
        accumulate_key_bucket(
            &mut by_route_policy_id,
            snapshot.filters.route_policy_id.as_deref(),
        );
        accumulate_key_bucket(&mut by_action_key, snapshot.filters.action_key.as_deref());
        accumulate_key_bucket(
            &mut by_execution_mode,
            snapshot.filters.execution_mode.as_deref(),
        );
        accumulate_key_bucket(&mut by_label, snapshot.label.as_deref());
    }

    GatewayAnalysisAnomalyRemediationEffectivenessSnapshotInventorySummaryView {
        total_snapshots: snapshots.len(),
        total_runs,
        total_impacted_runs,
        total_unavailable_runs,
        by_route_policy_id: into_key_buckets(by_route_policy_id),
        by_action_key: into_key_buckets(by_action_key),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_label: into_key_buckets(by_label),
    }
}

fn matches_remediation_effectiveness_snapshot_filters(
    snapshot: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotView,
    filters: &GatewayAnalysisAnomalyRemediationEffectivenessSnapshotFilters,
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

fn build_remediation_effectiveness_snapshot_object_key(snapshot_id: &str) -> String {
    format!("ai-gateway/remediation-effectiveness-snapshots/{snapshot_id}/snapshot.json")
}
