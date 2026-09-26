use super::*;

pub async fn list_anomaly_incidents(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyIncidentFilters,
) -> Result<Vec<GatewayAnalysisAnomalyIncidentView>, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        from gateway_analysis_anomaly_incidents
        where 1 = 1
        "#,
    );

    push_optional_filter(&mut builder, "id", filters.incident_id.as_deref());
    push_optional_filter(&mut builder, "policy_id", filters.policy_id.as_deref());
    push_optional_filter(&mut builder, "project_id", filters.project_id.as_deref());
    push_optional_filter(
        &mut builder,
        "route_policy_id",
        filters.route_policy_id.as_deref(),
    );
    push_optional_filter(
        &mut builder,
        "owner_user_id",
        filters.owner_user_id.as_deref(),
    );
    push_optional_filter(&mut builder, "tag", filters.tag.as_deref());
    push_optional_filter(&mut builder, "text_mode", filters.text_mode.as_deref());
    push_optional_filter(&mut builder, "status", filters.status.as_deref());
    push_optional_filter(
        &mut builder,
        "follow_up_status",
        filters.follow_up_status.as_deref(),
    );
    push_optional_filter(
        &mut builder,
        "escalation_status",
        filters.escalation_status.as_deref(),
    );
    push_optional_filter(&mut builder, "code", filters.code.as_deref());
    push_optional_filter(&mut builder, "severity", filters.severity.as_deref());

    builder
        .push(" order by updated_at desc limit ")
        .push_bind(i64::try_from(limit).unwrap_or(500));

    let rows = builder
        .build_query_as::<GatewayAnalysisAnomalyIncidentRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_anomaly_incident_view).collect())
}

pub(super) async fn get_anomaly_incident_row(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentRow, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    sqlx::query_as::<_, GatewayAnalysisAnomalyIncidentRow>(
        r#"
        select
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        from gateway_analysis_anomaly_incidents
        where id = $1
        limit 1
        "#,
    )
    .bind(incident_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly incident 不存在"))
}

pub(super) async fn get_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    Ok(to_anomaly_incident_view(
        get_anomaly_incident_row(pool, incident_id).await?,
    ))
}

pub(super) async fn fetch_scope_incidents(
    pool: &PgPool,
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
) -> Result<Vec<GatewayAnalysisAnomalyIncidentRow>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        from gateway_analysis_anomaly_incidents
        where 1 = 1
        "#,
    );
    push_scope_filter(&mut builder, "policy_id", policy_id);
    push_scope_filter(&mut builder, "project_id", project_id);
    push_scope_filter(&mut builder, "route_policy_id", route_policy_id);
    push_scope_filter(&mut builder, "tag", tag);
    push_scope_filter(&mut builder, "text_mode", text_mode);

    builder
        .build_query_as::<GatewayAnalysisAnomalyIncidentRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

fn to_anomaly_incident_view(
    row: GatewayAnalysisAnomalyIncidentRow,
) -> GatewayAnalysisAnomalyIncidentView {
    GatewayAnalysisAnomalyIncidentView {
        id: row.id,
        policy_id: row.policy_id,
        fingerprint: row.fingerprint,
        project_id: row.project_id,
        route_policy_id: row.route_policy_id,
        tag: row.tag,
        text_mode: trimmed_owned(row.text_mode.as_deref()),
        code: row.code,
        severity: row.severity,
        status: normalize_incident_status(&row.status).to_string(),
        owner_user_id: row.owner_user_id,
        follow_up_status: normalize_incident_follow_up_status(&row.follow_up_status).to_string(),
        sync_hit_count: row.sync_hit_count,
        escalation_status: normalize_incident_escalation_status(&row.escalation_status).to_string(),
        escalated_at: row.escalated_at.map(format_timestamp),
        escalation_reason: row.escalation_reason,
        latest_note: row.latest_note,
        resolution_note: row.resolution_note,
        last_action_at: row.last_action_at.map(format_timestamp),
        last_alert_attempt_at: row.last_alert_attempt_at.map(format_timestamp),
        last_alerted_at: row.last_alerted_at.map(format_timestamp),
        last_alert_severity: trimmed_owned(row.last_alert_severity.as_deref()),
        alert_delivery_count: row.alert_delivery_count,
        summary: row.summary,
        latest_export_id: row.latest_export_id,
        previous_export_id: row.previous_export_id,
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value,
        first_seen_at: format_timestamp(row.first_seen_at),
        last_seen_at: format_timestamp(row.last_seen_at),
        acknowledged_at: row.acknowledged_at.map(format_timestamp),
        resolved_at: row.resolved_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn push_optional_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    column: &str,
    value: Option<&str>,
) {
    if let Some(value) = trimmed_owned(value) {
        builder
            .push(" and ")
            .push(column)
            .push(" = ")
            .push_bind(value);
    }
}

fn push_scope_filter(builder: &mut QueryBuilder<'_, Postgres>, column: &str, value: Option<&str>) {
    builder.push(" and ").push(column);
    if let Some(value) = trimmed_owned(value) {
        builder.push(" = ").push_bind(value);
    } else {
        builder.push(" is null");
    }
}
