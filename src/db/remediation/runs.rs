use super::*;

pub async fn list_anomaly_incident_remediation_runs(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
) -> Result<Vec<GatewayAnalysisAnomalyIncidentRemediationRunView>, GatewayError> {
    let created_from = parse_filter_timestamp(filters.created_from.as_deref(), "createdFrom")?;
    let created_to = parse_filter_timestamp(filters.created_to.as_deref(), "createdTo")?;
    if let (Some(created_from), Some(created_to)) = (created_from, created_to) {
        if created_from > created_to {
            return Err(GatewayError::bad_request("createdFrom 不能晚于 createdTo"));
        }
    }
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);

    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        select
          id,
          incident_id,
          policy_id,
          route_policy_id,
          action_key,
          title,
          execution_mode,
          status,
          dry_run,
          actor_user_id,
          note,
          input,
          result,
          before_incident,
          after_incident,
          before_route_policy,
          after_route_policy,
          error_summary,
          created_at,
          completed_at
        from gateway_analysis_anomaly_remediation_runs
        where 1 = 1
        "#,
    );
    push_optional_filter(&mut builder, "incident_id", filters.incident_id.as_deref());
    push_optional_filter(&mut builder, "policy_id", filters.policy_id.as_deref());
    push_optional_filter(
        &mut builder,
        "route_policy_id",
        filters.route_policy_id.as_deref(),
    );
    push_optional_filter(&mut builder, "action_key", filters.action_key.as_deref());
    push_optional_filter(&mut builder, "status", filters.status.as_deref());
    push_optional_filter(
        &mut builder,
        "execution_mode",
        filters.execution_mode.as_deref(),
    );
    if let Some(dry_run) = filters.dry_run {
        builder.push(" and dry_run = ").push_bind(dry_run);
    }
    if let Some(created_from) = created_from {
        builder.push(" and created_at >= ").push_bind(created_from);
    }
    if let Some(created_to) = created_to {
        builder.push(" and created_at <= ").push_bind(created_to);
    }
    builder
        .push(" order by created_at desc limit ")
        .push_bind(i64::try_from(limit).unwrap_or(500));

    let rows = builder
        .build_query_as::<GatewayAnalysisAnomalyRemediationRunRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    rows.into_iter().map(to_remediation_run_view).collect()
}

pub async fn summarize_anomaly_incident_remediation_runs(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyRemediationRunFilters,
) -> Result<GatewayAnalysisAnomalyRemediationRunSummaryView, GatewayError> {
    let runs = list_anomaly_incident_remediation_runs(
        pool,
        &GatewayAnalysisAnomalyRemediationRunFilters {
            limit: Some(filters.limit.unwrap_or(500).max(500)),
            ..filters.clone()
        },
    )
    .await?;
    Ok(build_remediation_run_summary(&runs))
}

pub(super) fn build_remediation_run_summary(
    runs: &[GatewayAnalysisAnomalyIncidentRemediationRunView],
) -> GatewayAnalysisAnomalyRemediationRunSummaryView {
    let mut by_status = BTreeMap::new();
    let mut by_execution_mode = BTreeMap::new();
    let mut by_action_key = BTreeMap::new();
    let mut by_policy_id = BTreeMap::new();
    let mut by_route_policy_id = BTreeMap::new();
    let mut incident_ids = HashSet::new();
    let mut dry_run_runs = 0;
    let mut applied_runs = 0;
    let mut failed_runs = 0;
    let mut route_policy_changed_runs = 0;
    let mut incident_changed_runs = 0;

    for run in runs {
        accumulate_key_bucket(&mut by_status, Some(run.status.as_str()));
        accumulate_key_bucket(&mut by_execution_mode, Some(run.execution_mode.as_str()));
        accumulate_key_bucket(&mut by_action_key, Some(run.action_key.as_str()));
        accumulate_key_bucket(&mut by_policy_id, run.policy_id.as_deref());
        accumulate_key_bucket(&mut by_route_policy_id, run.route_policy_id.as_deref());
        incident_ids.insert(run.incident_id.clone());

        match run.status.as_str() {
            "dry_run" => dry_run_runs += 1,
            "applied" => applied_runs += 1,
            "failed" => failed_runs += 1,
            _ => {}
        }
        if has_route_policy_meaningful_changes(run) {
            route_policy_changed_runs += 1;
        }
        if has_incident_meaningful_changes(
            run.before_incident.as_ref(),
            run.after_incident.as_ref(),
        ) {
            incident_changed_runs += 1;
        }
    }

    GatewayAnalysisAnomalyRemediationRunSummaryView {
        total_runs: runs.len(),
        dry_run_runs,
        applied_runs,
        failed_runs,
        distinct_incident_count: incident_ids.len(),
        route_policy_changed_runs,
        incident_changed_runs,
        by_status: into_key_buckets(by_status),
        by_execution_mode: into_key_buckets(by_execution_mode),
        by_action_key: into_key_buckets(by_action_key),
        by_policy_id: into_key_buckets(by_policy_id),
        by_route_policy_id: into_key_buckets(by_route_policy_id),
    }
}

fn has_incident_meaningful_changes(
    before_incident: Option<&GatewayAnalysisAnomalyIncidentView>,
    after_incident: Option<&GatewayAnalysisAnomalyIncidentView>,
) -> bool {
    match (before_incident, after_incident) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(before), Some(after)) => {
            before.owner_user_id != after.owner_user_id
                || before.follow_up_status != after.follow_up_status
                || before.status != after.status
                || before.escalation_status != after.escalation_status
                || before.latest_note != after.latest_note
                || before.resolution_note != after.resolution_note
        }
    }
}

fn has_route_policy_meaningful_changes(
    run: &GatewayAnalysisAnomalyIncidentRemediationRunView,
) -> bool {
    match (&run.before_route_policy, &run.after_route_policy) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(before), Some(after)) => {
            before.enabled != after.enabled
                || before.is_default != after.is_default
                || before.config != after.config
        }
    }
}

pub(super) fn to_remediation_run_view(
    row: GatewayAnalysisAnomalyRemediationRunRow,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    Ok(GatewayAnalysisAnomalyIncidentRemediationRunView {
        id: row.id,
        incident_id: row.incident_id,
        policy_id: row.policy_id,
        route_policy_id: row.route_policy_id,
        action_key: row.action_key,
        title: row.title,
        execution_mode: row.execution_mode,
        status: row.status,
        dry_run: row.dry_run,
        actor_user_id: row.actor_user_id,
        note: row.note,
        input: row.input.map(|value| value.0),
        result: row.result.map(|value| value.0),
        before_incident: parse_optional_json_field(row.before_incident)?,
        after_incident: parse_optional_json_field(row.after_incident)?,
        before_route_policy: parse_optional_json_field(row.before_route_policy)?,
        after_route_policy: parse_optional_json_field(row.after_route_policy)?,
        error_summary: row.error_summary,
        created_at: format_timestamp(row.created_at),
        completed_at: row.completed_at.map(format_timestamp),
    })
}

pub(super) async fn get_remediation_run_row_by_id(
    pool: &PgPool,
    run_id: &str,
) -> Result<GatewayAnalysisAnomalyRemediationRunRow, GatewayError> {
    let run_id =
        trimmed_owned_ref(run_id).ok_or_else(|| GatewayError::bad_request("runId 不能为空"))?;
    sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        select
          id, incident_id, policy_id, route_policy_id, action_key, title, execution_mode, status, dry_run,
          actor_user_id, note, input, result, before_incident, after_incident, before_route_policy, after_route_policy,
          error_summary, created_at, completed_at
        from gateway_analysis_anomaly_remediation_runs
        where id = $1
        limit 1
        "#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("Gateway anomaly remediation run 不存在"))
}

pub(super) async fn get_remediation_run_view_by_id(
    pool: &PgPool,
    run_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentRemediationRunView, GatewayError> {
    to_remediation_run_view(get_remediation_run_row_by_id(pool, run_id).await?)
}

pub(super) async fn find_latest_anomaly_remediation_run(
    pool: &PgPool,
    incident_id: &str,
    action_key: &str,
) -> Result<Option<GatewayAnalysisAnomalyIncidentRemediationRunView>, GatewayError> {
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyRemediationRunRow>(
        r#"
        select
          id,
          incident_id,
          policy_id,
          route_policy_id,
          action_key,
          title,
          execution_mode,
          status,
          dry_run,
          actor_user_id,
          note,
          input,
          result,
          before_incident,
          after_incident,
          before_route_policy,
          after_route_policy,
          error_summary,
          created_at,
          completed_at
        from gateway_analysis_anomaly_remediation_runs
        where incident_id = $1 and action_key = $2
        order by created_at desc
        limit 1
        "#,
    )
    .bind(incident_id)
    .bind(action_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    row.map(to_remediation_run_view).transpose()
}

pub(super) async fn count_anomaly_remediation_runs_by_status(
    pool: &PgPool,
    incident_id: &str,
    action_key: &str,
    status: &str,
) -> Result<i32, GatewayError> {
    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from gateway_analysis_anomaly_remediation_runs
        where incident_id = $1 and action_key = $2 and status = $3
        "#,
    )
    .bind(incident_id)
    .bind(action_key)
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;
    Ok(count.clamp(0, i64::from(i32::MAX)) as i32)
}

fn parse_optional_json_field<T: DeserializeOwned>(
    value: Option<Json<Value>>,
) -> Result<Option<T>, GatewayError> {
    value
        .map(|value| {
            serde_json::from_value::<T>(value.0).map_err(|error| {
                GatewayError::server_error(format!("parse remediation snapshot: {error}"))
            })
        })
        .transpose()
}
