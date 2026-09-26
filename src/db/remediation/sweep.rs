use super::policy_sync::truncate_error_summary;
use super::*;

pub async fn sweep_anomaly_remediations(
    pool: &PgPool,
    redis_pool: &RedisPool,
    actor_user_id: &str,
    filters: &GatewayAnalysisAnomalyRemediationQueueFilters,
) -> Result<GatewayAnalysisAnomalyRemediationSweepView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id)
        .ok_or_else(|| GatewayError::bad_request("actorUserId 不能为空"))?;
    let started_at = OffsetDateTime::now_utc();
    let limit = filters.limit.unwrap_or(20).clamp(1, 100);
    let queue = list_anomaly_remediation_queue(
        pool,
        redis_pool,
        &GatewayAnalysisAnomalyRemediationQueueFilters {
            due_only: Some(true),
            limit: Some(limit),
            ..filters.clone()
        },
    )
    .await?;

    let mut items = Vec::new();
    let mut dry_run_count = 0;
    let mut applied_count = 0;
    let mut error_count = 0;
    let mut skipped_count = 0;

    for item in queue.items {
        let Some(next_execution_status) = item.next_execution_status.as_deref() else {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                incident_id: item.incident.id,
                action_key: item.action.action_key,
                status: "skipped".to_string(),
                execution_status: None,
                run_id: None,
                error: None,
            });
            continue;
        };

        if !item.remediation_due {
            skipped_count += 1;
            items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                incident_id: item.incident.id,
                action_key: item.action.action_key,
                status: "skipped".to_string(),
                execution_status: Some(next_execution_status.to_string()),
                run_id: None,
                error: None,
            });
            continue;
        }

        let input = remediation_execution_input_from_action(&item.action, next_execution_status)?;
        match execute_anomaly_incident_remediation(pool, actor_user_id, &item.incident.id, input)
            .await
        {
            Ok(run) => {
                if run.status == "dry_run" {
                    dry_run_count += 1;
                } else {
                    applied_count += 1;
                }
                items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                    incident_id: item.incident.id,
                    action_key: item.action.action_key,
                    status: "ok".to_string(),
                    execution_status: Some(run.status.clone()),
                    run_id: Some(run.id),
                    error: None,
                });
            }
            Err(error) => {
                error_count += 1;
                items.push(GatewayAnalysisAnomalyRemediationSweepItemView {
                    incident_id: item.incident.id,
                    action_key: item.action.action_key,
                    status: "error".to_string(),
                    execution_status: Some(next_execution_status.to_string()),
                    run_id: None,
                    error: Some(truncate_error_summary(&error.to_string(), 240)),
                });
            }
        }
    }

    Ok(GatewayAnalysisAnomalyRemediationSweepView {
        started_at: format_timestamp(started_at),
        completed_at: format_timestamp(OffsetDateTime::now_utc()),
        limit,
        attempted_count: items.len(),
        dry_run_count,
        applied_count,
        error_count,
        skipped_count,
        items,
    })
}

fn remediation_execution_input_from_action(
    action: &GatewayAnalysisAnomalyIncidentRemediationActionView,
    status: &str,
) -> Result<ExecuteGatewayAnalysisAnomalyIncidentRemediationInput, GatewayError> {
    let mut input = action
        .default_execution_input
        .clone()
        .map(|value| {
            serde_json::from_value::<ExecuteGatewayAnalysisAnomalyIncidentRemediationInput>(value)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "parse remediation default execution input: {error}"
                    ))
                })
        })
        .transpose()?
        .unwrap_or_else(|| ExecuteGatewayAnalysisAnomalyIncidentRemediationInput {
            action_key: action.action_key.clone(),
            ..ExecuteGatewayAnalysisAnomalyIncidentRemediationInput::default()
        });
    input.action_key = action.action_key.clone();
    input.dry_run = Some(status == "dry_run");
    Ok(input)
}
