use super::route_policy_patch::{apply_route_policy_patch, build_route_policy_patch};
use super::runs::to_remediation_run_view;
use super::*;

pub async fn execute_anomaly_incident_remediation(
    pool: &PgPool,
    actor_user_id: &str,
    incident_id: &str,
    input: ExecuteGatewayAnalysisAnomalyIncidentRemediationInput,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    let actor_user_id = trimmed_owned_ref(actor_user_id)
        .ok_or_else(|| GatewayError::bad_request("actorUserId 不能为空"))?;
    let action_key = trimmed_owned_ref(&input.action_key)
        .ok_or_else(|| GatewayError::bad_request("actionKey 不能为空"))?;
    let plan = get_anomaly_incident_remediation_plan(pool, incident_id).await?;
    let action = plan
        .actions
        .iter()
        .find(|item| item.action_key == action_key)
        .ok_or_else(|| {
            GatewayError::conflict(format!(
                "incident 当前不存在 remediation action: {action_key}"
            ))
        })?
        .clone();
    if !action.executable || action.execution_mode == "informational" {
        return Err(GatewayError::conflict(format!(
            "remediation action {} 仅提供建议，当前不支持直接执行。",
            action.action_key
        )));
    }

    let timestamp = OffsetDateTime::now_utc();
    let dry_run = input.dry_run == Some(true);
    let run_id = uuid::Uuid::new_v4().to_string();
    let note = trimmed_owned(input.note.as_deref());
    let before_incident = plan.incident.clone();
    let before_route_policy = plan.route_policy.clone();

    let mut after_incident = Some(before_incident.clone());
    let mut after_route_policy = before_route_policy.clone();
    let mut result = None;
    let mut error_summary = None;
    let mut status = if dry_run { "dry_run" } else { "applied" }.to_string();

    let execution_result: Result<(), GatewayError> = match action.execution_mode.as_str() {
        "incident_follow_up" => {
            let follow_up_input =
                build_follow_up_input(&before_incident, input.incident_follow_up.clone());
            if dry_run {
                after_incident = Some(simulate_follow_up_update(
                    &before_incident,
                    &follow_up_input,
                    timestamp,
                ));
            } else {
                after_incident = Some(
                    update_anomaly_incident_follow_up(pool, incident_id, follow_up_input).await?,
                );
            }
            result = Some(json!({
                "actionKey": action.action_key,
                "executionMode": action.execution_mode,
                "status": status,
                "changedFields": ["ownerUserId", "followUpStatus", "note", "resolutionNote"],
                "summary": "Updated incident ownership and follow-up fields."
            }));
            Ok(())
        }
        "route_policy_patch" => {
            let before = before_route_policy.clone().ok_or_else(|| {
                GatewayError::conflict("当前 remediation action 缺少可用 route policy")
            })?;
            let patch = build_route_policy_patch(
                &action.action_key,
                &before,
                input.route_policy_patch.clone(),
            )?;
            if dry_run {
                after_route_policy = Some(apply_route_policy_patch(&before, &patch));
            } else {
                let next_policy = apply_route_policy_patch(&before, &patch);
                after_route_policy = Some(
                    save_route_policy(
                        pool,
                        Some(&before.id),
                        SaveRoutePolicyInput {
                            project_id: before.project_id.clone(),
                            name: before.name.clone(),
                            is_default: before.is_default,
                            enabled: before.enabled,
                            config: next_policy.config.clone(),
                        },
                    )
                    .await?,
                );
            }
            result = Some(json!({
                "actionKey": action.action_key,
                "executionMode": action.execution_mode,
                "status": status,
                "changedFields": patch.changed_fields,
                "summary": patch.summary
            }));
            Ok(())
        }
        other => Err(GatewayError::conflict(format!(
            "当前尚未支持 remediation executionMode: {other}"
        ))),
    };

    if let Err(error) = execution_result {
        status = "failed".to_string();
        error_summary = Some(error.to_string());
        result = Some(json!({
            "actionKey": action.action_key,
            "executionMode": action.execution_mode,
            "status": status,
            "changedFields": [],
            "summary": error_summary
        }));
    }

    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        insert into gateway_analysis_anomaly_remediation_runs (
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9,
          $10, $11, $12, $13, $14, $15, $16, $17,
          $18, $19, $19
        )
        returning
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        "#,
    )
    .bind(&run_id)
    .bind(incident_id.trim())
    .bind(Option::<String>::None)
    .bind(after_route_policy.as_ref().map(|item| item.id.as_str()).or(before_route_policy.as_ref().map(|item| item.id.as_str())))
    .bind(&action.action_key)
    .bind(&action.title)
    .bind(&action.execution_mode)
    .bind(&status)
    .bind(dry_run)
    .bind(actor_user_id)
    .bind(note.as_deref())
    .bind(Some(sqlx::types::Json(serde_json::to_value(&input).map_err(|error| GatewayError::server_error(format!("serialize remediation input: {error}")))?)))
    .bind(result.clone().map(sqlx::types::Json))
    .bind(Some(sqlx::types::Json(serde_json::to_value(&before_incident).map_err(|error| GatewayError::server_error(format!("serialize remediation before incident: {error}")))?)))
    .bind(after_incident.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(before_route_policy.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(after_route_policy.as_ref().map(|value| serde_json::to_value(value).ok()).flatten().map(sqlx::types::Json))
    .bind(error_summary.as_deref())
    .bind(timestamp)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    append_remediation_history(
        pool,
        incident_id.trim(),
        actor_user_id,
        &action,
        &status,
        dry_run,
        note.as_deref(),
        after_incident.as_ref().unwrap_or(&before_incident),
        after_route_policy.as_ref().or(before_route_policy.as_ref()),
        &run_id,
        result.as_ref(),
        timestamp,
    )
    .await?;

    to_remediation_run_view(row)
}

fn build_follow_up_input(
    incident: &GatewayAnalysisAnomalyIncidentView,
    requested: Option<GatewayAnalysisAnomalyIncidentFollowUpInput>,
) -> GatewayAnalysisAnomalyIncidentFollowUpInput {
    let requested = requested.unwrap_or_default();
    GatewayAnalysisAnomalyIncidentFollowUpInput {
        owner_user_id: Some(
            requested
                .owner_user_id
                .or_else(|| incident.owner_user_id.clone())
                .unwrap_or_default(),
        ),
        follow_up_status: Some(requested.follow_up_status.unwrap_or_else(|| {
            if incident.follow_up_status == "pending" {
                "investigating".to_string()
            } else {
                incident.follow_up_status.clone()
            }
        })),
        note: Some(
            requested
                .note
                .or_else(|| incident.latest_note.clone())
                .unwrap_or_default(),
        ),
        resolution_note: Some(
            requested
                .resolution_note
                .or_else(|| incident.resolution_note.clone())
                .unwrap_or_default(),
        ),
    }
}

fn simulate_follow_up_update(
    incident: &GatewayAnalysisAnomalyIncidentView,
    input: &GatewayAnalysisAnomalyIncidentFollowUpInput,
    timestamp: OffsetDateTime,
) -> GatewayAnalysisAnomalyIncidentView {
    let mut updated = incident.clone();
    if let Some(owner_user_id) = input.owner_user_id.as_deref() {
        updated.owner_user_id = trimmed_owned(Some(owner_user_id));
    }
    if let Some(follow_up_status) = input.follow_up_status.as_deref() {
        updated.follow_up_status = follow_up_status.trim().to_string();
    }
    if let Some(note) = input.note.as_deref() {
        updated.latest_note = trimmed_owned(Some(note));
    }
    if let Some(resolution_note) = input.resolution_note.as_deref() {
        updated.resolution_note = trimmed_owned(Some(resolution_note));
    }
    updated.last_action_at = Some(format_timestamp(timestamp));
    updated.updated_at = format_timestamp(timestamp);
    updated
}

async fn append_remediation_history(
    pool: &PgPool,
    incident_id: &str,
    actor_user_id: &str,
    action: &GatewayAnalysisAnomalyIncidentRemediationActionView,
    status: &str,
    dry_run: bool,
    note: Option<&str>,
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
    remediation_run_id: &str,
    result: Option<&Value>,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let event_type = if status == "failed" {
        "remediation_failed"
    } else if dry_run {
        "remediation_dry_run"
    } else {
        "remediation_applied"
    };
    let mut metadata = json!({
        "policyId": incident.policy_id,
        "projectId": incident.project_id,
        "routePolicyId": incident.route_policy_id,
        "tag": incident.tag,
        "textMode": incident.text_mode,
        "code": incident.code,
        "severity": incident.severity,
        "status": incident.status,
        "ownerUserId": incident.owner_user_id,
        "followUpStatus": incident.follow_up_status,
        "syncHitCount": incident.sync_hit_count,
        "escalationStatus": incident.escalation_status,
        "escalatedAt": incident.escalated_at,
        "escalationReason": incident.escalation_reason,
        "latestExportId": incident.latest_export_id,
        "previousExportId": incident.previous_export_id,
        "latestValue": incident.latest_value,
        "previousValue": incident.previous_value,
        "deltaValue": incident.delta_value,
        "deltaRatio": incident.delta_ratio,
        "thresholdValue": incident.threshold_value,
        "remediationRunId": remediation_run_id,
        "actionKey": action.action_key,
        "executionMode": action.execution_mode,
        "runStatus": status,
        "dryRun": dry_run,
        "routePolicy": route_policy,
        "result": result
    });
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "routePolicyId".to_string(),
            json!(route_policy
                .map(|item| item.id.clone())
                .or_else(|| incident.route_policy_id.clone())),
        );
    }
    append_incident_history(
        pool,
        incident_id,
        event_type,
        Some(actor_user_id),
        note.or(Some(action.title.as_str())),
        Some(metadata),
        timestamp,
    )
    .await
}

pub(super) async fn append_incident_history(
    pool: &PgPool,
    incident_id: &str,
    event_type: &str,
    actor_user_id: Option<&str>,
    note: Option<&str>,
    metadata: Option<Value>,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incident_history (
          id,
          incident_id,
          event_type,
          actor_user_id,
          note,
          metadata,
          created_at
        ) values ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(incident_id)
    .bind(event_type)
    .bind(actor_user_id)
    .bind(note)
    .bind(metadata.map(sqlx::types::Json))
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}
