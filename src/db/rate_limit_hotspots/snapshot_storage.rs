use super::*;

pub(super) fn rate_limit_hotspot_snapshot_prefix() -> &'static str {
    "ai-gateway/rate-limit-hotspot-snapshots"
}

pub(super) fn rate_limit_hotspot_anomaly_snapshot_prefix() -> &'static str {
    "ai-gateway/rate-limit-hotspot-anomaly-snapshots"
}

pub(super) fn build_rate_limit_hotspot_snapshot_object_key(snapshot_id: &str) -> String {
    format!(
        "{}/{}/snapshot.json",
        rate_limit_hotspot_snapshot_prefix(),
        snapshot_id.trim()
    )
}

pub(super) fn build_rate_limit_hotspot_anomaly_snapshot_object_key(snapshot_id: &str) -> String {
    format!(
        "{}/{}/snapshot.json",
        rate_limit_hotspot_anomaly_snapshot_prefix(),
        snapshot_id.trim()
    )
}

pub(super) async fn read_rate_limit_hotspot_snapshot<T>(object_key: &str) -> Result<T, GatewayError>
where
    T: DeserializeOwned,
{
    let payload = gateway_object_storage()?.read_json(object_key).await?;
    serde_json::from_value(payload).map_err(|error| {
        GatewayError::server_error(format!("parse hotspot snapshot payload: {error}"))
    })
}

pub(super) fn matches_rate_limit_hotspot_snapshot_filters(
    snapshot: &GatewayRateLimitHotspotSnapshotView,
    filters: &GatewayRateLimitHotspotSnapshotFilters,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = trimmed_owned_ref(filters.snapshot_id.as_deref()) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = trimmed_owned_ref(filters.label.as_deref()) {
        let haystack = snapshot
            .label
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !haystack.contains(&label.to_ascii_lowercase()) {
            return false;
        }
    }
    if !matches_optional_filter(
        snapshot.filters.project_id.as_deref(),
        filters.project_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.route_policy_id.as_deref(),
        filters.route_policy_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.api_key_id.as_deref(),
        filters.api_key_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.endpoint_kind.as_deref(),
        filters.endpoint_kind.as_deref(),
    ) {
        return false;
    }
    matches_created_at_window(snapshot.created_at.as_str(), created_from, created_to)
}

pub(super) fn matches_rate_limit_hotspot_anomaly_snapshot_filters(
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    filters: &GatewayRateLimitHotspotAnomalySnapshotFilters,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    if let Some(snapshot_id) = trimmed_owned_ref(filters.snapshot_id.as_deref()) {
        if snapshot.snapshot_id != snapshot_id {
            return false;
        }
    }
    if let Some(label) = trimmed_owned_ref(filters.label.as_deref()) {
        let haystack = snapshot
            .label
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !haystack.contains(&label.to_ascii_lowercase()) {
            return false;
        }
    }
    if !matches_optional_filter(
        snapshot.filters.project_id.as_deref(),
        filters.project_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.route_policy_id.as_deref(),
        filters.route_policy_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.api_key_id.as_deref(),
        filters.api_key_id.as_deref(),
    ) || !matches_optional_filter(
        snapshot.filters.endpoint_kind.as_deref(),
        filters.endpoint_kind.as_deref(),
    ) || !matches_optional_filter(
        Some(snapshot.filters.profile_key.as_str()),
        filters.profile_key.as_deref(),
    ) {
        return false;
    }
    matches_created_at_window(snapshot.created_at.as_str(), created_from, created_to)
}

fn matches_optional_filter(value: Option<&str>, filter: Option<&str>) -> bool {
    let Some(filter) = trimmed_owned_ref(filter) else {
        return true;
    };
    value.is_some_and(|current| current.trim() == filter)
}

fn matches_created_at_window(
    created_at: &str,
    created_from: Option<&OffsetDateTime>,
    created_to: Option<&OffsetDateTime>,
) -> bool {
    let Some(created_at) = parse_filter_timestamp(Some(created_at), "createdAt")
        .ok()
        .flatten()
    else {
        return false;
    };
    if let Some(started_at) = created_from {
        if &created_at < started_at {
            return false;
        }
    }
    if let Some(ended_at) = created_to {
        if &created_at > ended_at {
            return false;
        }
    }
    true
}

pub(super) fn parse_filter_timestamp(
    value: Option<&str>,
    field: &str,
) -> Result<Option<OffsetDateTime>, GatewayError> {
    let Some(value) = trimmed_owned_ref(value) else {
        return Ok(None);
    };
    OffsetDateTime::parse(value, &Rfc3339)
        .map(Some)
        .map_err(|_| GatewayError::conflict(format!("{field} 不是合法的 RFC3339 时间戳")))
}

pub(super) fn normalize_lookback_hours(value: Option<i32>) -> Option<i32> {
    value.map(|hours| hours.clamp(0, 24 * 365))
}

pub(super) fn trimmed_owned(value: Option<&str>) -> Option<String> {
    trimmed_owned_ref(value).map(ToString::to_string)
}

pub(super) fn trimmed_owned_ref(value: Option<&str>) -> Option<&str> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
