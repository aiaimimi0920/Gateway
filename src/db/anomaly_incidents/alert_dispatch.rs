use super::*;

pub async fn record_anomaly_incident_alert_dispatch(
    pool: &PgPool,
    actor_user_id: &str,
    incident_id: &str,
    input: RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let actor_user_id = trimmed_owned_ref(actor_user_id)
        .ok_or_else(|| GatewayError::bad_request("actorUserId 不能为空"))?;
    let existing = get_anomaly_incident_row(pool, incident_id).await?;
    let alert_timestamp = match input.alerted_at.as_deref().and_then(trimmed_owned_ref) {
        Some(value) => OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_| GatewayError::bad_request("alertedAt 必须是合法的 ISO 时间"))?,
        None => OffsetDateTime::now_utc(),
    };
    let mailbox_recipient_count = input.mailbox_recipient_count.unwrap_or_default().max(0);
    let webhook_dispatched = input.webhook_dispatched == Some(true);
    let delivery_succeeded = mailbox_recipient_count > 0 || webhook_dispatched;
    let alert_severity = normalize_alert_delivery_severity(
        input
            .alert_severity
            .as_deref()
            .or(existing.last_alert_severity.as_deref()),
    );

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          last_alert_attempt_at = $2,
          last_alerted_at = case when $3 then $2 else last_alerted_at end,
          last_alert_severity = case when $3 then $4 else last_alert_severity end,
          alert_delivery_count = case when $3 then alert_delivery_count + 1 else alert_delivery_count end,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(alert_timestamp)
    .bind(delivery_succeeded)
    .bind(alert_severity)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    if delivery_succeeded {
        let mut metadata = build_incident_snapshot_metadata_from_view(&updated);
        let history_note = trimmed_owned(input.note.as_deref())
            .unwrap_or_else(|| "Gateway anomaly alert dispatched.".to_string());
        if let Some(object) = metadata.as_object_mut() {
            object.insert("alertLevel".to_string(), json!(input.alert_level));
            object.insert(
                "mailboxRecipientCount".to_string(),
                json!(mailbox_recipient_count),
            );
            object.insert("webhookDispatched".to_string(), json!(webhook_dispatched));
            object.insert(
                "webhookSkippedReason".to_string(),
                json!(trimmed_owned(input.webhook_skipped_reason.as_deref())),
            );
            object.insert(
                "remediationActionKeys".to_string(),
                json!(input
                    .remediation_action_keys
                    .map(|items| {
                        items
                            .into_iter()
                            .filter_map(|item| trimmed_owned(Some(&item)))
                            .collect::<Vec<_>>()
                    })
                    .filter(|items| !items.is_empty())),
            );
        }
        append_anomaly_incident_history(
            pool,
            incident_id,
            "alert_dispatched",
            Some(actor_user_id),
            Some(history_note.as_str()),
            Some(metadata),
            Some(alert_timestamp),
        )
        .await?;
    }

    Ok(updated)
}

fn normalize_alert_delivery_severity(value: Option<&str>) -> &str {
    match value.and_then(trimmed_owned_ref) {
        Some(value) if value.eq_ignore_ascii_case("info") => "info",
        Some(value) if value.eq_ignore_ascii_case("danger") => "danger",
        _ => "warning",
    }
}
