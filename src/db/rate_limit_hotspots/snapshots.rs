use super::*;

pub async fn persist_rate_limit_hotspot_snapshot(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    label: Option<&str>,
    lookback_hours: Option<i32>,
) -> Result<GatewayRateLimitHotspotSnapshotView, GatewayError> {
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
    let limit = filters.limit.unwrap_or(1000).clamp(1, 1000);
    let normalized_filters = RequestAuditFilters {
        created_from: created_from.clone(),
        limit: Some(limit),
        ..filters.clone()
    };
    let summary = summarize_rate_limit_hotspots(pool, &normalized_filters).await?;
    let snapshot_id = Uuid::new_v4().to_string();
    let object_key = build_rate_limit_hotspot_snapshot_object_key(&snapshot_id);
    let snapshot = GatewayRateLimitHotspotSnapshotView {
        snapshot_id,
        label: trimmed_owned(label),
        created_at: format_timestamp(timestamp),
        object_key: object_key.clone(),
        filters: GatewayRateLimitHotspotSnapshotFilterView {
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            provider_account_id: normalized_filters.provider_account_id,
            session_id: normalized_filters.session_id,
            api_key_id: normalized_filters.api_key_id,
            response_id: normalized_filters.response_id,
            protocol_family: normalized_filters.protocol_family,
            endpoint_kind: normalized_filters.endpoint_kind,
            error_code: normalized_filters.error_code,
            created_from,
            created_to: normalized_filters.created_to,
            limit,
            lookback_hours,
        },
        summary,
    };
    gateway_object_storage()?
        .put_json(
            &object_key,
            &serde_json::to_value(&snapshot).map_err(|error| {
                GatewayError::server_error(format!(
                    "serialize rate-limit hotspot snapshot: {error}"
                ))
            })?,
        )
        .await?;
    Ok(snapshot)
}

pub async fn list_rate_limit_hotspot_snapshots(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<Vec<GatewayRateLimitHotspotSnapshotView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(started_at), Some(ended_at)) = (created_from.as_ref(), created_to.as_ref()) {
        if started_at > ended_at {
            return Err(GatewayError::conflict("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let object_keys = gateway_object_storage()?
        .list_objects(rate_limit_hotspot_snapshot_prefix())
        .await?;
    let mut snapshots = Vec::new();
    for object_key in object_keys
        .into_iter()
        .filter(|key| key.ends_with("/snapshot.json"))
    {
        let snapshot =
            read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotSnapshotView>(&object_key)
                .await
                .ok();
        let Some(snapshot) = snapshot else {
            continue;
        };
        if !matches_rate_limit_hotspot_snapshot_filters(
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

pub async fn get_rate_limit_hotspot_snapshot(
    snapshot_id: &str,
) -> Result<GatewayRateLimitHotspotSnapshotView, GatewayError> {
    let snapshot_id = trimmed_owned_ref(Some(snapshot_id))
        .ok_or_else(|| GatewayError::conflict("snapshotId 不能为空"))?;
    read_rate_limit_hotspot_snapshot::<GatewayRateLimitHotspotSnapshotView>(
        &build_rate_limit_hotspot_snapshot_object_key(snapshot_id),
    )
    .await
}

pub async fn summarize_rate_limit_hotspot_snapshot_inventory(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<GatewayRateLimitHotspotSnapshotInventorySummaryView, GatewayError> {
    let snapshots = list_rate_limit_hotspot_snapshots(&GatewayRateLimitHotspotSnapshotFilters {
        limit: Some(filters.limit.unwrap_or(500).clamp(1, 500)),
        ..filters.clone()
    })
    .await?;
    Ok(build_rate_limit_hotspot_snapshot_inventory_summary(
        &snapshots,
    ))
}

pub async fn get_rate_limit_hotspot_snapshot_trend_report(
    filters: &GatewayRateLimitHotspotSnapshotFilters,
) -> Result<GatewayRateLimitHotspotSnapshotTrendReportView, GatewayError> {
    let window_size = filters.limit.unwrap_or(10).clamp(1, 50);
    let normalized_filters = GatewayRateLimitHotspotSnapshotFilters {
        limit: Some(window_size),
        ..filters.clone()
    };
    let snapshots = list_rate_limit_hotspot_snapshots(&normalized_filters).await?;
    let inventory_summary =
        summarize_rate_limit_hotspot_snapshot_inventory(&GatewayRateLimitHotspotSnapshotFilters {
            limit: Some(500),
            ..normalized_filters.clone()
        })
        .await?;
    let points = snapshots
        .into_iter()
        .map(build_rate_limit_hotspot_snapshot_trend_point)
        .collect::<Vec<_>>();
    Ok(build_rate_limit_hotspot_snapshot_trend_report(
        format_timestamp(OffsetDateTime::now_utc()),
        GatewayRateLimitHotspotSnapshotReportFilterView {
            label: normalized_filters.label,
            project_id: normalized_filters.project_id,
            route_policy_id: normalized_filters.route_policy_id,
            api_key_id: normalized_filters.api_key_id,
            endpoint_kind: normalized_filters.endpoint_kind,
            created_from: normalized_filters.created_from,
            created_to: normalized_filters.created_to,
        },
        window_size,
        inventory_summary,
        points,
    ))
}
