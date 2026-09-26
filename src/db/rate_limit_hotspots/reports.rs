use super::*;

pub async fn summarize_rate_limit_hotspots(
    pool: &PgPool,
    filters: &RequestAuditFilters,
) -> Result<GatewayRateLimitHotspotSummaryView, GatewayError> {
    let rows = load_rate_limit_audit_rows(pool, filters, 200, 1000).await?;
    Ok(build_rate_limit_hotspot_summary(&rows))
}

pub async fn get_rate_limit_hotspot_trend_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    window_size: Option<usize>,
    bucket_size_minutes: Option<usize>,
) -> Result<GatewayRateLimitHotspotTrendReportView, GatewayError> {
    let limit = filters.limit.unwrap_or(1000).clamp(1, 1000);
    let rows = load_rate_limit_audit_rows(
        pool,
        &RequestAuditFilters {
            limit: Some(limit),
            ..filters.clone()
        },
        1000,
        1000,
    )
    .await?;
    let window_size = window_size.unwrap_or(12).clamp(1, 168);
    let bucket_size_minutes = bucket_size_minutes.unwrap_or(60).clamp(1, 1440);
    let filter_view =
        to_rate_limit_hotspot_filter_view(filters, limit, window_size, bucket_size_minutes);
    Ok(build_rate_limit_hotspot_trend_report(
        format_timestamp(OffsetDateTime::now_utc()),
        filter_view,
        &rows,
    ))
}

pub async fn get_rate_limit_hotspot_anomaly_report(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    window_size: Option<usize>,
    bucket_size_minutes: Option<usize>,
    profile_key: Option<&str>,
    overrides: GatewayRateLimitHotspotAnomalyOverrides,
) -> Result<GatewayRateLimitHotspotAnomalyReportView, GatewayError> {
    let normalized_profile_key = normalize_profile_key(profile_key);
    let trend_report =
        get_rate_limit_hotspot_trend_report(pool, filters, window_size, bucket_size_minutes)
            .await?;
    let thresholds =
        build_rate_limit_hotspot_anomaly_thresholds(&normalized_profile_key, overrides);
    Ok(build_rate_limit_hotspot_anomaly_report(
        format_timestamp(OffsetDateTime::now_utc()),
        trend_report,
        normalized_profile_key,
        thresholds,
    ))
}

async fn load_rate_limit_audit_rows(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    default_limit: usize,
    max_limit: usize,
) -> Result<Vec<GatewayRequestAuditView>, GatewayError> {
    let limit = filters.limit.unwrap_or(default_limit).clamp(1, max_limit);
    list_request_audits(
        pool,
        &RequestAuditFilters {
            status: Some("failed".to_string()),
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await
}

fn to_rate_limit_hotspot_filter_view(
    filters: &RequestAuditFilters,
    limit: usize,
    window_size: usize,
    bucket_size_minutes: usize,
) -> GatewayRateLimitHotspotFilterView {
    GatewayRateLimitHotspotFilterView {
        project_id: filters.project_id.clone(),
        route_policy_id: filters.route_policy_id.clone(),
        provider_account_id: filters.provider_account_id.clone(),
        session_id: filters.session_id.clone(),
        api_key_id: filters.api_key_id.clone(),
        response_id: filters.response_id.clone(),
        protocol_family: filters.protocol_family.clone(),
        endpoint_kind: filters.endpoint_kind.clone(),
        error_code: filters.error_code.clone(),
        created_from: filters.created_from.clone(),
        created_to: filters.created_to.clone(),
        limit,
        window_size,
        bucket_size_minutes,
    }
}
