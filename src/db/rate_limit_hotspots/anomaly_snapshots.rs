use super::*;

pub async fn persist_rate_limit_hotspot_anomaly_snapshot(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
    profile_key: Option<&str>,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> Result<GatewayRateLimitHotspotAnomalySnapshotView, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let lookback_hours = normalize_lookback_hours(lookback_hours);
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
    let normalized_filters = RequestAuditFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let report = get_rate_limit_hotspot_anomaly_report(
        pool,
        &normalized_filters,
        None,
        None,
        profile_key,
        overrides,
    )
    .await?;
    let snapshot_id = Uuid::new_v4().to_string();
    let object_key = build_rate_limit_hotspot_anomaly_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayRateLimitHotspotAnomalySnapshotView {
        snapshot_id,
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayRateLimitHotspotAnomalySnapshotFilterView {
            label: trimmed_owned(label),
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            api_key_id: normalized_filters.api_key_id,
            endpoint_kind: normalized_filters.endpoint_kind,
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
                    "serialize rate-limit hotspot anomaly snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_rate_limit_hotspot_anomaly_snapshots(
    filters: &GatewayRateLimitHotspotAnomalySnapshotFilters,
) -> Result<Vec<GatewayRateLimitHotspotAnomalySnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(started_at), Some(ended_at)) = (created_from.as_ref(), created_to.as_ref()) {
        if started_at > ended_at {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects(rate_limit_hotspot_anomaly_snapshot_prefix())
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys
        .into_iter()
        .filter(|key| key.ends_with("/snapshot.json"))
    {
        let snapshot =
            read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotAnomalySnapshotView>(
                &object_key,
            )
            .await
            .ok();
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_rate_limit_hotspot_anomaly_snapshot_filters(
            &snapshot,
            filters,
            created_from.as_ref(),
            created_to.as_ref(),
        ) {
            continue;
        }
        snapshots.push(snapshot);
    }
    snapshots.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    snapshots.truncate(limit);
    Ok(snapshots)
}

pub async fn get_rate_limit_hotspot_anomaly_snapshot(
    snapshot_id: &str,
) -> Result<GatewayRateLimitHotspotAnomalySnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(Some(snapshot_id))
        .ok_or_else(|| GatewayError::conflict("snapshotId 不能为空"))?;
    read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotAnomalySnapshotView>(
        &build_rate_limit_hotspot_anomaly_snapshot_object_key(snapshot_id),
    )
    .await
}
