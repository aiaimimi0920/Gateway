use super::auto_remediation::{
    read_route_policy_health_degraded, resolve_incident_auto_remediation_config,
};
use super::plan::build_incident_remediation_plan;
use super::plan_context::{find_route_policy_by_id, load_incident_latest_sync_context};
use super::policies::find_anomaly_policy_by_id;
use super::runs::{count_anomaly_remediation_runs_by_status, find_latest_anomaly_remediation_run};
use super::*;

pub async fn list_anomaly_remediation_queue(
    pool: &PgPool,
    redis_pool: &RedisPool,
    filters: &GatewayAnalysisAnomalyRemediationQueueFilters,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationQueueView, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let due_only = filters.due_only == Some(true);
    let reference_time = OffsetDateTime::now_utc();
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            incident_id: filters.incident_id.clone(),
            policy_id: filters.policy_id.clone(),
            project_id: filters.project_id.clone(),
            route_policy_id: filters.route_policy_id.clone(),
            owner_user_id: filters.owner_user_id.clone(),
            tag: filters.tag.clone(),
            text_mode: filters.text_mode.clone(),
            status: filters.status.clone(),
            follow_up_status: filters.follow_up_status.clone(),
            escalation_status: Some("escalated".to_string()),
            code: filters.code.clone(),
            severity: filters.severity.clone(),
            due_only: None,
            limit: Some(limit.max(200)),
        },
    )
    .await?;

    let mut items = Vec::new();
    for incident in incidents {
        let policy = if let Some(policy_id) = incident.policy_id.as_deref() {
            find_anomaly_policy_by_id(pool, policy_id).await?
        } else {
            None
        };
        let resolved_route_policy_id = policy
            .as_ref()
            .and_then(|item| item.route_policy_id.clone())
            .or_else(|| incident.route_policy_id.clone());
        let route_policy = if let Some(route_policy_id) = resolved_route_policy_id.as_deref() {
            find_route_policy_by_id(pool, route_policy_id).await?
        } else {
            None
        };
        let incident_context = load_incident_latest_sync_context(pool, &incident.id).await?;
        let plan = build_incident_remediation_plan(
            reference_time,
            incident.clone(),
            policy.clone(),
            route_policy.clone(),
            incident_context,
        );
        let policy_config = resolve_incident_auto_remediation_config(
            policy.as_ref(),
            &incident,
            route_policy.as_ref(),
        );
        for action in plan.actions {
            if !action.executable {
                continue;
            }
            if let Some(action_key) = filters.action_key.as_deref().and_then(trimmed_owned_ref) {
                if action.action_key != action_key {
                    continue;
                }
            }
            if let Some(execution_mode) = filters
                .execution_mode
                .as_deref()
                .and_then(trimmed_owned_ref)
            {
                if action.execution_mode != execution_mode {
                    continue;
                }
            }
            let action_allowed = policy_config
                .auto_remediation_action_keys
                .as_ref()
                .map(|items| items.iter().any(|item| item == &action.action_key))
                .unwrap_or(true);
            let latest_run =
                find_latest_anomaly_remediation_run(pool, &incident.id, &action.action_key).await?;
            let applied_run_count = count_anomaly_remediation_runs_by_status(
                pool,
                &incident.id,
                &action.action_key,
                "applied",
            )
            .await?;
            let provider_health_degraded =
                read_route_policy_health_degraded(pool, redis_pool, route_policy.as_ref()).await?;
            let schedule = resolve_anomaly_remediation_schedule(
                &incident,
                &policy_config,
                action_allowed,
                applied_run_count,
                provider_health_degraded,
                latest_run.as_ref(),
                reference_time,
            );
            if due_only && !schedule.remediation_due {
                continue;
            }
            items.push(GatewayAnalysisAnomalyIncidentRemediationQueueItemView {
                incident: incident.clone(),
                policy: policy
                    .as_ref()
                    .and_then(|value| serde_json::to_value(value).ok()),
                route_policy: route_policy.clone(),
                action,
                remediation_due: schedule.remediation_due,
                next_execution_status: schedule.next_execution_status,
                next_run_due_at: schedule.next_run_due_at,
                blocked_reason: schedule.blocked_reason,
                latest_run,
            });
        }
    }

    items.sort_by(|left, right| {
        if left.remediation_due != right.remediation_due {
            return if left.remediation_due {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        if left.incident.severity != right.incident.severity {
            return if left.incident.severity == "critical" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        right.incident.updated_at.cmp(&left.incident.updated_at)
    });
    items.truncate(limit);

    Ok(GatewayAnalysisAnomalyIncidentRemediationQueueView {
        generated_at: format_timestamp(reference_time),
        limit,
        due_only,
        item_count: items.len(),
        due_count: items.iter().filter(|item| item.remediation_due).count(),
        items,
    })
}

pub(super) fn resolve_anomaly_remediation_schedule(
    incident: &GatewayAnalysisAnomalyIncidentView,
    config: &AutoRemediationConfig,
    action_enabled: bool,
    applied_run_count: i32,
    provider_health_degraded: bool,
    latest_run: Option<&GatewayAnalysisAnomalyIncidentRemediationRunView>,
    now: OffsetDateTime,
) -> AutoRemediationSchedule {
    if incident.status == "resolved"
        || incident.escalation_status != "escalated"
        || !config.auto_remediation_enabled
        || !action_enabled
    {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: if !config.auto_remediation_enabled {
                Some("policy_disabled".to_string())
            } else if !action_enabled {
                Some("action_not_allowed".to_string())
            } else {
                None
            },
        };
    }

    let latest_reference = latest_run
        .and_then(|item| item.completed_at.as_ref())
        .or_else(|| latest_run.map(|item| &item.created_at))
        .cloned();
    if latest_reference.is_none() {
        let next_execution_status = if config.auto_remediation_dry_run_first {
            "dry_run".to_string()
        } else {
            "applied".to_string()
        };
        return build_schedule_without_reference(
            &next_execution_status,
            config,
            incident.last_alerted_at.as_deref(),
            applied_run_count,
            provider_health_degraded,
        );
    }

    let latest_run = latest_run.expect("reference implies latest run exists");
    if latest_run.status == "failed" {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: Some("previous_failure".to_string()),
        };
    }
    if latest_run.status == "applied" {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: None,
            next_run_due_at: None,
            blocked_reason: Some("already_applied".to_string()),
        };
    }

    let reference_time =
        match OffsetDateTime::parse(latest_reference.as_deref().unwrap_or_default(), &Rfc3339) {
            Ok(value) => value,
            Err(_) => {
                return AutoRemediationSchedule {
                    remediation_due: false,
                    next_execution_status: None,
                    next_run_due_at: None,
                    blocked_reason: Some("invalid_reference_time".to_string()),
                }
            }
        };
    let next_run_due_at = reference_time
        + time::Duration::minutes(i64::from(config.auto_remediation_interval_minutes));
    let next_execution_status =
        if latest_run.status == "dry_run" && config.auto_remediation_dry_run_first {
            "applied".to_string()
        } else if config.auto_remediation_dry_run_first && latest_run.dry_run {
            "applied".to_string()
        } else {
            "dry_run".to_string()
        };

    if let Some(blocked_reason) = remediation_blocked_reason(
        &next_execution_status,
        config,
        incident.last_alerted_at.as_deref(),
        applied_run_count,
        provider_health_degraded,
    ) {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: Some(next_execution_status),
            next_run_due_at: Some(format_timestamp(next_run_due_at)),
            blocked_reason: Some(blocked_reason),
        };
    }

    AutoRemediationSchedule {
        remediation_due: next_run_due_at <= now,
        next_execution_status: Some(next_execution_status),
        next_run_due_at: Some(format_timestamp(next_run_due_at)),
        blocked_reason: None,
    }
}

fn build_schedule_without_reference(
    next_execution_status: &str,
    config: &AutoRemediationConfig,
    last_alerted_at: Option<&str>,
    applied_run_count: i32,
    provider_health_degraded: bool,
) -> AutoRemediationSchedule {
    if let Some(blocked_reason) = remediation_blocked_reason(
        next_execution_status,
        config,
        last_alerted_at,
        applied_run_count,
        provider_health_degraded,
    ) {
        return AutoRemediationSchedule {
            remediation_due: false,
            next_execution_status: Some(next_execution_status.to_string()),
            next_run_due_at: None,
            blocked_reason: Some(blocked_reason),
        };
    }
    AutoRemediationSchedule {
        remediation_due: true,
        next_execution_status: Some(next_execution_status.to_string()),
        next_run_due_at: None,
        blocked_reason: None,
    }
}

fn remediation_blocked_reason(
    next_execution_status: &str,
    config: &AutoRemediationConfig,
    last_alerted_at: Option<&str>,
    applied_run_count: i32,
    provider_health_degraded: bool,
) -> Option<String> {
    if next_execution_status == "applied"
        && config.auto_remediation_require_alert_before_apply
        && trimmed_owned_ref_opt(last_alerted_at).is_none()
    {
        return Some("alert_pending".to_string());
    }
    if next_execution_status == "applied"
        && config
            .auto_remediation_max_apply_runs_per_incident
            .is_some_and(|cap| applied_run_count >= cap)
    {
        return Some("apply_cap_reached".to_string());
    }
    if next_execution_status == "applied"
        && config.auto_remediation_freeze_on_provider_health_degrade
        && provider_health_degraded
    {
        return Some("provider_health_degraded".to_string());
    }
    None
}
