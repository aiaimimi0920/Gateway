use super::plan::build_incident_remediation_plan;
use super::plan_context::{find_route_policy_by_id, load_incident_latest_sync_context};
use super::policies::find_anomaly_policy_by_id;
use super::*;

pub async fn list_anomaly_incident_alert_queue(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyIncidentFilters,
) -> Result<GatewayAnalysisAnomalyIncidentAlertQueueView, GatewayError> {
    let limit = filters.limit.unwrap_or(50).clamp(1, 200);
    let due_only = filters.due_only == Some(true);
    let reference_time = OffsetDateTime::now_utc();
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            escalation_status: Some("escalated".to_string()),
            limit: Some(limit.max(200)),
            ..filters.clone()
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
        let alert_config = resolve_anomaly_policy_alert_config(policy.as_ref());
        let schedule = resolve_anomaly_incident_alert_schedule(
            &incident.status,
            &incident.escalation_status,
            &alert_config,
            incident.last_alert_attempt_at.as_deref(),
            reference_time,
        );
        if due_only && !schedule.alert_due {
            continue;
        }
        let delivery_profile = resolve_anomaly_alert_delivery_profile(&incident.severity);
        let plan = build_incident_remediation_plan(
            reference_time,
            incident.clone(),
            policy.clone(),
            route_policy.clone(),
            incident_context,
        );
        items.push(GatewayAnalysisAnomalyIncidentAlertQueueItemView {
            incident,
            policy: policy
                .as_ref()
                .and_then(|value| serde_json::to_value(value).ok()),
            route_policy,
            alert_interval_minutes: alert_config.alert_interval_minutes,
            alert_due: schedule.alert_due,
            next_alert_due_at: schedule.next_alert_due_at,
            notify_operators: alert_config.notify_operators,
            notify_owner: alert_config.notify_owner,
            alert_level: delivery_profile.alert_level,
            webhook_severity: delivery_profile.webhook_severity,
            remediation_action_keys: plan
                .actions
                .into_iter()
                .map(|action| action.action_key)
                .collect(),
        });
    }

    items.sort_by(|left, right| {
        if left.alert_due != right.alert_due {
            return if left.alert_due {
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
        right.incident.last_seen_at.cmp(&left.incident.last_seen_at)
    });
    items.truncate(limit);

    Ok(GatewayAnalysisAnomalyIncidentAlertQueueView {
        generated_at: format_timestamp(reference_time),
        limit,
        due_only,
        incident_count: items.len(),
        due_count: items.iter().filter(|item| item.alert_due).count(),
        items,
    })
}

fn resolve_anomaly_policy_alert_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
) -> AlertConfig {
    AlertConfig {
        alerting_enabled: policy.map(|item| item.alerting_enabled).unwrap_or(true),
        alert_interval_minutes: policy
            .and_then(|item| item.alert_interval_minutes)
            .unwrap_or(DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES),
        notify_operators: policy
            .map(|item| item.notify_operators_on_escalation)
            .unwrap_or(true),
        notify_owner: policy
            .map(|item| item.notify_owner_on_escalation)
            .unwrap_or(true),
    }
}

pub(super) fn resolve_anomaly_incident_alert_schedule(
    status: &str,
    escalation_status: &str,
    config: &AlertConfig,
    last_alert_attempt_at: Option<&str>,
    now: OffsetDateTime,
) -> AlertSchedule {
    let normalized_status = trimmed_owned_ref_opt(Some(status))
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "open".to_string());
    let normalized_escalation_status = trimmed_owned_ref_opt(Some(escalation_status))
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "none".to_string());
    if !config.alerting_enabled
        || normalized_escalation_status != "escalated"
        || !matches!(normalized_status.as_str(), "open" | "acknowledged")
    {
        return AlertSchedule {
            next_alert_due_at: None,
            alert_due: false,
        };
    }

    let next_alert_due_at = last_alert_attempt_at
        .and_then(trimmed_owned_ref)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        .map(|value| value + time::Duration::minutes(i64::from(config.alert_interval_minutes)));

    if let Some(next_due_at) = next_alert_due_at {
        return AlertSchedule {
            next_alert_due_at: Some(format_timestamp(next_due_at)),
            alert_due: next_due_at <= now,
        };
    }

    AlertSchedule {
        next_alert_due_at: None,
        alert_due: true,
    }
}

pub(super) fn resolve_anomaly_alert_delivery_profile(severity: &str) -> AlertDeliveryProfile {
    if trimmed_owned_ref_opt(Some(severity))
        .is_some_and(|value| value.eq_ignore_ascii_case("critical"))
    {
        return AlertDeliveryProfile {
            alert_level: 3,
            webhook_severity: "danger".to_string(),
        };
    }

    AlertDeliveryProfile {
        alert_level: 2,
        webhook_severity: "warning".to_string(),
    }
}
