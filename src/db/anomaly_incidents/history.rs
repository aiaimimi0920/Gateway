use super::*;

pub async fn list_anomaly_incident_history(
    pool: &PgPool,
    incident_id: &str,
    limit: Option<usize>,
) -> Result<Vec<GatewayAnalysisAnomalyIncidentHistoryView>, GatewayError> {
    let incident = get_anomaly_incident_row(pool, incident_id).await?;
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    let rows = sqlx::query_as::<_, GatewayAnalysisAnomalyIncidentHistoryRow>(
        r#"
        select
          id,
          incident_id,
          event_type,
          actor_user_id,
          note,
          metadata,
          created_at
        from gateway_analysis_anomaly_incident_history
        where incident_id = $1
        order by created_at desc
        limit $2
        "#,
    )
    .bind(&incident.id)
    .bind(i64::try_from(limit).unwrap_or(1000))
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows
        .into_iter()
        .map(to_anomaly_incident_history_view)
        .collect())
}

pub(super) async fn append_anomaly_incident_history(
    pool: &PgPool,
    incident_id: &str,
    event_type: &str,
    actor_user_id: Option<&str>,
    note: Option<&str>,
    metadata: Option<Value>,
    created_at: Option<OffsetDateTime>,
) -> Result<(), GatewayError> {
    let history_id = Uuid::new_v4().to_string();
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
    .bind(history_id)
    .bind(incident_id)
    .bind(event_type)
    .bind(actor_user_id)
    .bind(note)
    .bind(metadata.map(sqlx::types::Json))
    .bind(created_at.unwrap_or_else(OffsetDateTime::now_utc))
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

fn to_anomaly_incident_history_view(
    row: GatewayAnalysisAnomalyIncidentHistoryRow,
) -> GatewayAnalysisAnomalyIncidentHistoryView {
    GatewayAnalysisAnomalyIncidentHistoryView {
        id: row.id,
        incident_id: row.incident_id,
        event_type: row.event_type,
        actor_user_id: row.actor_user_id,
        note: row.note,
        metadata: row.metadata.map(|value| value.0),
        created_at: format_timestamp(row.created_at),
    }
}
