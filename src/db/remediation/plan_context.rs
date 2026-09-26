use super::plan::build_incident_remediation_plan;
use super::policies::find_anomaly_policy_by_id;
use super::*;

pub async fn get_anomaly_incident_remediation_plan(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationPlanView, GatewayError> {
    let incident = get_incident_by_id(pool, incident_id).await?;
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
    Ok(build_incident_remediation_plan(
        OffsetDateTime::now_utc(),
        incident,
        policy,
        route_policy,
        incident_context,
    ))
}

pub(super) async fn load_incident_latest_sync_context(
    pool: &PgPool,
    incident_id: &str,
) -> Result<IncidentSyncContext, GatewayError> {
    let incident_id = trimmed_owned_ref(incident_id)
        .ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))?;
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyIncidentHistoryLookupRow>(
        r#"
        select metadata
        from gateway_analysis_anomaly_incident_history
        where incident_id = $1
          and event_type = any($2)
        order by created_at desc
        limit 1
        "#,
    )
    .bind(incident_id)
    .bind(vec!["sync_opened".to_string(), "sync_updated".to_string()])
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    let Some(metadata) = row.and_then(|value| value.metadata.map(|item| item.0)) else {
        return Ok(IncidentSyncContext::default());
    };
    let object = metadata.as_object();
    Ok(IncidentSyncContext {
        entity_key: object
            .and_then(|item| item.get("entityKey"))
            .and_then(Value::as_str)
            .and_then(|item| trimmed_owned(Some(item))),
        snapshot_id: object
            .and_then(|item| item.get("snapshotId"))
            .and_then(Value::as_str)
            .and_then(|item| trimmed_owned(Some(item))),
    })
}

async fn get_incident_by_id(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = trimmed_owned_ref(incident_id)
        .ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))?;
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            incident_id: Some(incident_id.to_string()),
            limit: Some(1),
            ..GatewayAnalysisAnomalyIncidentFilters::default()
        },
    )
    .await?;
    incidents
        .into_iter()
        .next()
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly incident 不存在"))
}

pub(super) async fn find_route_policy_by_id(
    pool: &PgPool,
    route_policy_id: &str,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let Some(route_policy_id) = trimmed_owned_ref(route_policy_id) else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, GatewayRoutePolicyRow>(
        r#"
        select id, project_id, name, is_default, enabled, config, created_at, updated_at
        from gateway_route_policies
        where id = $1
        limit 1
        "#,
    )
    .bind(route_policy_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    row.map(route_policy_view_from_row).transpose()
}

fn route_policy_view_from_row(
    row: GatewayRoutePolicyRow,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    let config =
        serde_json::from_value::<GatewayRoutePolicyConfig>(row.config.0).map_err(|error| {
            GatewayError::server_error(format!("parse route policy config: {error}"))
        })?;
    Ok(GatewayRoutePolicyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        is_default: row.is_default,
        enabled: row.enabled,
        config,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentHistoryLookupRow {
    metadata: Option<Json<Value>>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayRoutePolicyRow {
    id: String,
    project_id: String,
    name: String,
    is_default: bool,
    enabled: bool,
    config: Json<Value>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}
