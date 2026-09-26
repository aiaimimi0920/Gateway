use super::policies::find_anomaly_policy_by_id;
use super::policy_sync::truncate_error_summary;
use super::*;

pub async fn sweep_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<GatewayAnalysisAnomalyPolicySweepView, GatewayError> {
    let started_at = OffsetDateTime::now_utc();
    let limit = filters.limit.unwrap_or(20).clamp(1, 100);
    let candidates = list_anomaly_policies(
        pool,
        &GatewayAnalysisAnomalyPolicyFilters {
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await?;
    let mut items = Vec::with_capacity(candidates.len());
    let mut ok_count = 0usize;
    let mut error_count = 0usize;
    let mut skipped_count = 0usize;

    for policy in candidates {
        if policy.status != "enabled" || !policy.auto_sync_enabled || !policy.sync_due {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                policy_id: policy.id,
                policy_name: policy.name,
                status: "skipped".to_string(),
                error: None,
                last_synced_at: policy.last_synced_at,
                next_sync_due_at: policy.next_sync_due_at,
                sync_due: policy.sync_due,
                anomaly_count: 0,
                opened_incident_count: 0,
                updated_incident_count: 0,
                resolved_incident_count: 0,
            });
            continue;
        }

        match sync_anomaly_policy(pool, &policy.id).await {
            Ok(result) => {
                ok_count += 1;
                items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                    policy_id: result.policy.id,
                    policy_name: result.policy.name,
                    status: "ok".to_string(),
                    error: None,
                    last_synced_at: result.policy.last_synced_at,
                    next_sync_due_at: result.policy.next_sync_due_at,
                    sync_due: result.policy.sync_due,
                    anomaly_count: result.anomaly_count,
                    opened_incident_count: result.opened_incident_count,
                    updated_incident_count: result.updated_incident_count,
                    resolved_incident_count: result.resolved_incident_count,
                });
            }
            Err(error) => {
                error_count += 1;
                let refreshed_policy = find_anomaly_policy_by_id(pool, &policy.id).await?;
                let (last_synced_at, next_sync_due_at, sync_due) =
                    if let Some(refreshed) = refreshed_policy {
                        (
                            refreshed.last_synced_at,
                            refreshed.next_sync_due_at,
                            refreshed.sync_due,
                        )
                    } else {
                        (
                            policy.last_synced_at,
                            policy.next_sync_due_at,
                            policy.sync_due,
                        )
                    };
                items.push(GatewayAnalysisAnomalyPolicySweepItemView {
                    policy_id: policy.id,
                    policy_name: policy.name,
                    status: "error".to_string(),
                    error: Some(truncate_error_summary(&error.to_string(), 240)),
                    last_synced_at,
                    next_sync_due_at,
                    sync_due,
                    anomaly_count: 0,
                    opened_incident_count: 0,
                    updated_incident_count: 0,
                    resolved_incident_count: 0,
                });
            }
        }
    }

    Ok(GatewayAnalysisAnomalyPolicySweepView {
        started_at: format_timestamp(started_at),
        completed_at: format_timestamp(OffsetDateTime::now_utc()),
        limit,
        attempted_count: items.len(),
        ok_count,
        error_count,
        skipped_count,
        items,
    })
}
