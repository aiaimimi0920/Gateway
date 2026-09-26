use super::*;

pub(super) async fn create_provider_routing_incident(
    pool: &PgPool,
    fingerprint: &str,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let sync_hit_count = 1;
    let escalation = resolve_provider_routing_auto_escalation(&anomaly.severity, sync_hit_count);
    let escalation_status = if escalation.should_escalate {
        "escalated"
    } else {
        "none"
    };
    let follow_up_status = escalation
        .follow_up_status
        .clone()
        .unwrap_or_else(|| "pending".to_string());
    let incident_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incidents (
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        ) values (
          $1, null, $2, $3, $4, $5, null, $6, $7, 'open',
          $8, $9, $10, $11, $12, $13,
          null, null, null, null, null, null,
          0, $14, null, null, $15, $16, $17, $18, $19, $20, $20, null, null, $20, $20
        )
        "#,
    )
    .bind(&incident_id)
    .bind(fingerprint)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(escalation.owner_user_id.as_deref())
    .bind(&follow_up_status)
    .bind(sync_hit_count)
    .bind(escalation_status)
    .bind(escalation.should_escalate.then_some(timestamp))
    .bind(escalation.reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &incident_id,
        "sync_opened",
        None,
        Some(&anomaly.message),
        Some(build_incident_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            anomaly,
            "open",
            escalation.owner_user_id.as_deref(),
            Some(&follow_up_status),
            sync_hit_count,
            Some(escalation_status),
            escalation.should_escalate.then_some(&timestamp),
            escalation.reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            &incident_id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_incident_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                anomaly,
                "open",
                escalation.owner_user_id.as_deref(),
                Some(&follow_up_status),
                sync_hit_count,
                Some(escalation_status),
                Some(&timestamp),
                escalation.reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(incident_id)
}

pub(super) async fn update_provider_routing_incident(
    pool: &PgPool,
    existing: &GatewayAnalysisAnomalyIncidentRow,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let previous_status = normalize_incident_status(&existing.status);
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let previous_follow_up_status = normalize_incident_follow_up_status(&existing.follow_up_status);
    let was_resolved = previous_status == "resolved";
    let next_status = if previous_status == "acknowledged" {
        "acknowledged"
    } else {
        "open"
    };
    let next_sync_hit_count = if was_resolved {
        1
    } else {
        existing.sync_hit_count.max(0) + 1
    };
    let escalation =
        resolve_provider_routing_auto_escalation(&anomaly.severity, next_sync_hit_count);
    let escalation_transitioned =
        escalation.should_escalate && previous_escalation_status != "escalated";
    let next_escalation_status = if escalation.should_escalate {
        "escalated"
    } else if was_resolved {
        "none"
    } else {
        previous_escalation_status
    };
    let next_escalated_at = if escalation.should_escalate {
        existing.escalated_at.unwrap_or(timestamp)
    } else {
        existing.escalated_at.unwrap_or(timestamp)
    };
    let next_escalation_reason = if escalation.should_escalate {
        escalation.reason.clone()
    } else if was_resolved {
        None
    } else {
        existing.escalation_reason.clone()
    };
    let next_owner_user_id = if escalation_transitioned && existing.owner_user_id.is_none() {
        escalation.owner_user_id.clone()
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status =
        if escalation_transitioned && (was_resolved || previous_follow_up_status == "pending") {
            escalation
                .follow_up_status
                .clone()
                .unwrap_or_else(|| previous_follow_up_status.to_string())
        } else {
            previous_follow_up_status.to_string()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          policy_id = null,
          project_id = $2,
          route_policy_id = $3,
          tag = $4,
          text_mode = null,
          code = $5,
          severity = $6,
          status = $7,
          owner_user_id = $8,
          follow_up_status = $9,
          sync_hit_count = $10,
          escalation_status = $11,
          escalated_at = $12,
          escalation_reason = $13,
          summary = $14,
          latest_export_id = null,
          previous_export_id = null,
          latest_value = $15,
          previous_value = $16,
          delta_value = $17,
          delta_ratio = $18,
          threshold_value = $19,
          last_seen_at = $20,
          resolved_at = null,
          updated_at = $20
        where id = $1
        "#,
    )
    .bind(&existing.id)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(next_status)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_sync_hit_count)
    .bind(next_escalation_status)
    .bind(if escalation.should_escalate {
        Some(next_escalated_at)
    } else if was_resolved {
        None
    } else {
        existing.escalated_at
    })
    .bind(next_escalation_reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &existing.id,
        "sync_updated",
        None,
        Some(&anomaly.message),
        Some(build_incident_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            anomaly,
            next_status,
            next_owner_user_id.as_deref(),
            Some(&next_follow_up_status),
            next_sync_hit_count,
            Some(next_escalation_status),
            if escalation.should_escalate {
                Some(&next_escalated_at)
            } else {
                existing.escalated_at.as_ref()
            },
            next_escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_transitioned {
        append_anomaly_incident_history(
            pool,
            &existing.id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_incident_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                anomaly,
                next_status,
                next_owner_user_id.as_deref(),
                Some(&next_follow_up_status),
                next_sync_hit_count,
                Some(next_escalation_status),
                Some(&next_escalated_at),
                next_escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(existing.id.clone())
}

pub(super) async fn resolve_provider_routing_incident(
    pool: &PgPool,
    row: &GatewayAnalysisAnomalyIncidentRow,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let next_escalation_status =
        if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
            "resolved"
        } else {
            row.escalation_status.as_str()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          sync_hit_count = 0,
          escalation_status = $2,
          resolved_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_escalation_status)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let anomaly = build_row_anomaly_view(row);
    append_anomaly_incident_history(
        pool,
        &row.id,
        "sync_resolved",
        None,
        Some(&row.summary),
        Some(build_incident_snapshot_metadata(
            row.project_id.as_deref(),
            row.route_policy_id.as_deref(),
            row.tag.as_deref(),
            &anomaly,
            "resolved",
            row.owner_user_id.as_deref(),
            Some(row.follow_up_status.as_str()),
            0,
            Some(next_escalation_status),
            row.escalated_at.as_ref(),
            row.escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
        append_anomaly_incident_history(
            pool,
            &row.id,
            "escalation_cleared",
            None,
            Some(row.escalation_reason.as_deref().unwrap_or(
                "Escalation cleared because provider routing anomaly no longer matched.",
            )),
            Some(build_incident_snapshot_metadata(
                row.project_id.as_deref(),
                row.route_policy_id.as_deref(),
                row.tag.as_deref(),
                &anomaly,
                "resolved",
                row.owner_user_id.as_deref(),
                Some(row.follow_up_status.as_str()),
                0,
                Some("resolved"),
                row.escalated_at.as_ref(),
                row.escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(())
}

fn build_incident_snapshot_metadata(
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    status: &str,
    owner_user_id: Option<&str>,
    follow_up_status: Option<&str>,
    sync_hit_count: i32,
    escalation_status: Option<&str>,
    escalated_at: Option<&OffsetDateTime>,
    escalation_reason: Option<&str>,
) -> Value {
    json!({
        "policyId": Value::Null,
        "projectId": project_id.and_then(trimmed_owned_ref),
        "routePolicyId": route_policy_id.and_then(trimmed_owned_ref),
        "tag": tag.and_then(trimmed_owned_ref),
        "textMode": Value::Null,
        "code": anomaly.code,
        "severity": anomaly.severity,
        "status": status,
        "ownerUserId": owner_user_id.and_then(trimmed_owned_ref),
        "followUpStatus": follow_up_status.and_then(trimmed_owned_ref),
        "syncHitCount": sync_hit_count,
        "escalationStatus": escalation_status.and_then(trimmed_owned_ref),
        "escalatedAt": escalated_at.map(|value| format_timestamp(*value)),
        "escalationReason": escalation_reason.and_then(trimmed_owned_ref),
        "lastAlertAttemptAt": Value::Null,
        "lastAlertedAt": Value::Null,
        "lastAlertSeverity": Value::Null,
        "alertDeliveryCount": Value::Null,
        "latestExportId": Value::Null,
        "previousExportId": Value::Null,
        "latestValue": anomaly.latest_value,
        "previousValue": anomaly.previous_value,
        "deltaValue": anomaly.delta_value,
        "deltaRatio": anomaly.delta_ratio,
        "thresholdValue": anomaly.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}

fn build_row_anomaly_view(
    row: &GatewayAnalysisAnomalyIncidentRow,
) -> GatewayProviderRoutingAnalysisAnomalyView {
    GatewayProviderRoutingAnalysisAnomalyView {
        code: row.code.clone(),
        severity: row.severity.clone(),
        message: row.summary.clone(),
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value,
    }
}
