use super::*;

pub async fn acknowledge_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'acknowledged',
          follow_up_status = 'investigating',
          acknowledged_at = $2,
          last_action_at = $2,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    append_anomaly_incident_history(
        pool,
        incident_id,
        "acknowledged",
        None,
        Some("Operator acknowledged incident."),
        Some(build_incident_snapshot_metadata_from_view(&updated)),
        Some(timestamp),
    )
    .await?;
    Ok(updated)
}

pub async fn resolve_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let existing = get_anomaly_incident_row(pool, incident_id).await?;
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          follow_up_status = 'done',
          escalation_status = $2,
          resolved_at = $3,
          last_action_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(if previous_escalation_status == "escalated" {
        "resolved"
    } else {
        existing.escalation_status.as_str()
    })
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    append_anomaly_incident_history(
        pool,
        incident_id,
        "resolved",
        None,
        Some(
            updated
                .resolution_note
                .as_deref()
                .unwrap_or("Operator resolved incident."),
        ),
        Some(build_incident_snapshot_metadata_from_view(&updated)),
        Some(timestamp),
    )
    .await?;

    if previous_escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            incident_id,
            "escalation_cleared",
            None,
            Some(
                updated
                    .escalation_reason
                    .as_deref()
                    .unwrap_or("Escalation cleared by operator resolution."),
            ),
            Some(build_incident_snapshot_metadata_from_view(&updated)),
            Some(timestamp),
        )
        .await?;
    }

    Ok(updated)
}

pub async fn update_anomaly_incident_follow_up(
    pool: &PgPool,
    incident_id: &str,
    input: GatewayAnalysisAnomalyIncidentFollowUpInput,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let existing = get_anomaly_incident_row(pool, incident_id).await?;
    let timestamp = OffsetDateTime::now_utc();
    let next_owner_user_id = if input.owner_user_id.is_some() {
        trimmed_owned(input.owner_user_id.as_deref())
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status = if let Some(value) = input.follow_up_status.as_deref() {
        normalize_incident_follow_up_status(value).to_string()
    } else {
        normalize_incident_follow_up_status(&existing.follow_up_status).to_string()
    };
    let next_note = if input.note.is_some() {
        trimmed_owned(input.note.as_deref())
    } else {
        existing.latest_note.clone()
    };
    let next_resolution_note = if input.resolution_note.is_some() {
        trimmed_owned(input.resolution_note.as_deref())
    } else {
        existing.resolution_note.clone()
    };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          owner_user_id = $2,
          follow_up_status = $3,
          latest_note = $4,
          resolution_note = $5,
          last_action_at = $6,
          updated_at = $6
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_note.as_deref())
    .bind(next_resolution_note.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    let changed_fields = [
        input.owner_user_id.as_ref().map(|_| "ownerUserId"),
        input.follow_up_status.as_ref().map(|_| "followUpStatus"),
        input.note.as_ref().map(|_| "note"),
        input.resolution_note.as_ref().map(|_| "resolutionNote"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let mut metadata = build_incident_snapshot_metadata_from_view(&updated);
    if let Some(object) = metadata.as_object_mut() {
        object.insert("changedFields".to_string(), json!(changed_fields));
        object.insert(
            "previousOwnerUserId".to_string(),
            json!(existing.owner_user_id.as_deref()),
        );
        object.insert(
            "previousFollowUpStatus".to_string(),
            json!(normalize_incident_follow_up_status(
                &existing.follow_up_status
            )),
        );
        object.insert(
            "previousSyncHitCount".to_string(),
            json!(existing.sync_hit_count),
        );
        object.insert(
            "previousEscalationStatus".to_string(),
            json!(normalize_incident_escalation_status(
                &existing.escalation_status
            )),
        );
        object.insert(
            "previousEscalatedAt".to_string(),
            json!(existing.escalated_at.map(format_timestamp)),
        );
        object.insert(
            "previousEscalationReason".to_string(),
            json!(existing.escalation_reason.as_deref()),
        );
        object.insert(
            "previousLatestNote".to_string(),
            json!(existing.latest_note.as_deref()),
        );
        object.insert(
            "previousResolutionNote".to_string(),
            json!(existing.resolution_note.as_deref()),
        );
    }

    append_anomaly_incident_history(
        pool,
        incident_id,
        "follow_up_updated",
        None,
        Some(
            updated
                .resolution_note
                .as_deref()
                .or(updated.latest_note.as_deref())
                .unwrap_or("Operator updated incident follow-up."),
        ),
        Some(metadata),
        Some(timestamp),
    )
    .await?;
    Ok(updated)
}

pub(super) fn build_incident_snapshot_metadata_from_view(
    incident: &GatewayAnalysisAnomalyIncidentView,
) -> Value {
    json!({
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
        "lastAlertAttemptAt": incident.last_alert_attempt_at,
        "lastAlertedAt": incident.last_alerted_at,
        "lastAlertSeverity": incident.last_alert_severity,
        "alertDeliveryCount": incident.alert_delivery_count,
        "latestExportId": incident.latest_export_id,
        "previousExportId": incident.previous_export_id,
        "latestValue": incident.latest_value,
        "previousValue": incident.previous_value,
        "deltaValue": incident.delta_value,
        "deltaRatio": incident.delta_ratio,
        "thresholdValue": incident.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}
