//! Provider status counts for readiness reporting.

use super::*;

pub async fn get_readiness_provider_stats(
    pool: &PgPool,
) -> Result<GatewayReadinessProviderStatsView, GatewayError> {
    let rows = sqlx::query_as::<_, CountByStatusRow>(
        r#"
        select status, count(*)::bigint as count
        from gateway_provider_accounts
        group by status
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let mut stats = GatewayReadinessProviderStatsView::default();
    for row in rows {
        match row.status.as_str() {
            "active" => stats.active_providers = row.count.max(0) as usize,
            "cooling" => stats.cooling_providers = row.count.max(0) as usize,
            "disabled" => stats.disabled_providers = row.count.max(0) as usize,
            _ => {}
        }
    }
    Ok(stats)
}
