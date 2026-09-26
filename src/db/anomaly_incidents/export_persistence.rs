use super::export_metadata::{
    build_analysis_export_incident_snapshot_metadata, build_row_analysis_export_anomaly_view,
};
use super::*;

pub(super) async fn create_analysis_export_incident(
    pool: &PgPool,
    fingerprint: &str,
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    auto_escalation: &GatewayAnalysisExportAutoEscalationConfig,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let sync_hit_count = 1;
    let escalation =
        resolve_analysis_export_auto_escalation(auto_escalation, &anomaly.severity, sync_hit_count);
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
          $1, $2, $3, $4, $5, $6, $7, $8, $9, 'open',
          $10, $11, $12, $13, $14, $15,
          null, null, null, null, null, null,
          0, $16, $17, $18, $19, $20, $21, $22, $23, $24, $24, null, null, $24, $24
        )
        "#,
    )
    .bind(&incident_id)
    .bind(policy_id)
    .bind(fingerprint)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(text_mode)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(escalation.owner_user_id.as_deref())
    .bind(&follow_up_status)
    .bind(sync_hit_count)
    .bind(escalation_status)
    .bind(escalation.should_escalate.then_some(timestamp))
    .bind(escalation.reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_export_id.as_deref())
    .bind(anomaly.previous_export_id.as_deref())
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
        Some(build_analysis_export_incident_snapshot_metadata(
            policy_id,
            project_id,
            route_policy_id,
            tag,
            text_mode,
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
            Some(build_analysis_export_incident_snapshot_metadata(
                policy_id,
                project_id,
                route_policy_id,
                tag,
                text_mode,
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

pub(super) async fn update_analysis_export_incident(
    pool: &PgPool,
    existing: &GatewayAnalysisAnomalyIncidentRow,
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    auto_escalation: &GatewayAnalysisExportAutoEscalationConfig,
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
    let escalation = resolve_analysis_export_auto_escalation(
        auto_escalation,
        &anomaly.severity,
        next_sync_hit_count,
    );
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
          policy_id = $2,
          project_id = $3,
          route_policy_id = $4,
          tag = $5,
          text_mode = $6,
          code = $7,
          severity = $8,
          status = $9,
          owner_user_id = $10,
          follow_up_status = $11,
          sync_hit_count = $12,
          escalation_status = $13,
          escalated_at = $14,
          escalation_reason = $15,
          summary = $16,
          latest_export_id = $17,
          previous_export_id = $18,
          latest_value = $19,
          previous_value = $20,
          delta_value = $21,
          delta_ratio = $22,
          threshold_value = $23,
          last_seen_at = $24,
          resolved_at = null,
          updated_at = $24
        where id = $1
        "#,
    )
    .bind(&existing.id)
    .bind(policy_id)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(text_mode)
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
    .bind(anomaly.latest_export_id.as_deref())
    .bind(anomaly.previous_export_id.as_deref())
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
        Some(build_analysis_export_incident_snapshot_metadata(
            policy_id,
            project_id,
            route_policy_id,
            tag,
            text_mode,
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
            Some(build_analysis_export_incident_snapshot_metadata(
                policy_id,
                project_id,
                route_policy_id,
                tag,
                text_mode,
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

pub(super) async fn resolve_analysis_export_incident(
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

    let anomaly = build_row_analysis_export_anomaly_view(row);
    append_anomaly_incident_history(
        pool,
        &row.id,
        "sync_resolved",
        None,
        Some(&row.summary),
        Some(build_analysis_export_incident_snapshot_metadata(
            row.policy_id.as_deref(),
            row.project_id.as_deref(),
            row.route_policy_id.as_deref(),
            row.tag.as_deref(),
            row.text_mode.as_deref(),
            &anomaly,
            "resolved",
            row.owner_user_id.as_deref(),
            Some(&row.follow_up_status),
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
                "Escalation cleared because analysis export anomaly no longer matched.",
            )),
            Some(build_analysis_export_incident_snapshot_metadata(
                row.policy_id.as_deref(),
                row.project_id.as_deref(),
                row.route_policy_id.as_deref(),
                row.tag.as_deref(),
                row.text_mode.as_deref(),
                &anomaly,
                "resolved",
                row.owner_user_id.as_deref(),
                Some(&row.follow_up_status),
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
