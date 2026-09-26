use super::policy_parameters::{
    normalize_anomaly_policy_status, normalize_anomaly_policy_sync_status,
    normalize_anomaly_profile_key,
};
use super::*;

pub async fn list_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<Vec<GatewayAnalysisAnomalyPolicyView>, GatewayError> {
    let limit = filters.limit.unwrap_or(100).clamp(1, 500);
    let due_only = filters.due_only == Some(true);
    let raw_limit = if due_only { limit.max(500) } else { limit };

    let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        r#"
        select
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          last_synced_at,
          last_sync_status,
          last_sync_error,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        from gateway_analysis_anomaly_policies
        where 1 = 1
        "#,
    );
    push_optional_filter(&mut builder, "id", filters.policy_id.as_deref());
    push_optional_filter(&mut builder, "project_id", filters.project_id.as_deref());
    push_optional_filter(
        &mut builder,
        "route_policy_id",
        filters.route_policy_id.as_deref(),
    );
    push_optional_filter(&mut builder, "status", filters.status.as_deref());
    if let Some(tag) = trimmed_owned_ref_opt(filters.tag.as_deref()) {
        builder
            .push(" and tag = ")
            .push_bind(tag.to_ascii_lowercase());
    }
    push_optional_filter(&mut builder, "text_mode", filters.text_mode.as_deref());
    if let Some(value) = filters.auto_sync_enabled {
        builder.push(" and auto_sync_enabled = ").push_bind(value);
    }
    if let Some(value) = filters.auto_escalate_enabled {
        builder
            .push(" and auto_escalate_enabled = ")
            .push_bind(value);
    }
    if let Some(value) = filters.auto_remediation_enabled {
        builder
            .push(" and auto_remediation_enabled = ")
            .push_bind(value);
    }
    if let Some(value) = filters.alerting_enabled {
        builder.push(" and alerting_enabled = ").push_bind(value);
    }
    builder
        .push(" order by updated_at desc limit ")
        .push_bind(i64::try_from(raw_limit).unwrap_or(500));

    let rows = builder
        .build_query_as::<GatewayAnalysisAnomalyPolicyRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;
    let now = OffsetDateTime::now_utc();
    let mut policies = rows
        .into_iter()
        .map(|row| to_anomaly_policy_view(row, now))
        .collect::<Vec<_>>();
    if due_only {
        policies.retain(|item| item.sync_due);
    }
    policies.truncate(limit);
    Ok(policies)
}

pub async fn summarize_anomaly_policies(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyPolicyFilters,
) -> Result<GatewayAnalysisAnomalyPolicySummaryView, GatewayError> {
    let policies = list_anomaly_policies(
        pool,
        &GatewayAnalysisAnomalyPolicyFilters {
            limit: Some(filters.limit.unwrap_or(200).max(200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut by_status = BTreeMap::new();
    let mut by_sync_status = BTreeMap::new();
    let mut enabled_policies = 0usize;
    let mut disabled_policies = 0usize;
    let mut auto_sync_enabled_policies = 0usize;
    let mut auto_escalate_enabled_policies = 0usize;
    let mut auto_remediation_enabled_policies = 0usize;
    let mut alerting_enabled_policies = 0usize;
    let mut due_policies = 0usize;

    for policy in &policies {
        accumulate_key_bucket(&mut by_status, Some(&policy.status));
        accumulate_key_bucket(&mut by_sync_status, policy.last_sync_status.as_deref());
        if policy.status == "enabled" {
            enabled_policies += 1;
        } else if policy.status == "disabled" {
            disabled_policies += 1;
        }
        if policy.auto_sync_enabled {
            auto_sync_enabled_policies += 1;
        }
        if policy.auto_escalate_enabled {
            auto_escalate_enabled_policies += 1;
        }
        if policy.auto_remediation_enabled {
            auto_remediation_enabled_policies += 1;
        }
        if policy.alerting_enabled {
            alerting_enabled_policies += 1;
        }
        if policy.sync_due {
            due_policies += 1;
        }
    }

    Ok(GatewayAnalysisAnomalyPolicySummaryView {
        total_policies: policies.len(),
        enabled_policies,
        disabled_policies,
        auto_sync_enabled_policies,
        auto_escalate_enabled_policies,
        auto_remediation_enabled_policies,
        alerting_enabled_policies,
        due_policies,
        by_status: into_key_buckets(by_status),
        by_sync_status: into_key_buckets(by_sync_status),
    })
}

pub(super) async fn find_anomaly_policy_by_id(
    pool: &PgPool,
    policy_id: &str,
) -> Result<Option<GatewayAnalysisAnomalyPolicyView>, GatewayError> {
    let Some(policy_id) = trimmed_owned_ref(policy_id) else {
        return Ok(None);
    };
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyPolicyRow>(
        r#"
        select
          id,
          name,
          status,
          project_id,
          route_policy_id,
          tag,
          text_mode,
          profile_key,
          thresholds,
          auto_sync_enabled,
          auto_sync_interval_minutes,
          last_synced_at,
          last_sync_status,
          last_sync_error,
          auto_escalate_enabled,
          escalate_severity_threshold,
          escalate_after_sync_count,
          auto_escalate_owner_user_id,
          auto_escalate_follow_up_status,
          auto_remediation_enabled,
          auto_remediation_interval_minutes,
          auto_remediation_dry_run_first,
          auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled,
          alert_interval_minutes,
          notify_operators_on_escalation,
          notify_owner_on_escalation,
          created_at,
          updated_at
        from gateway_analysis_anomaly_policies
        where id = $1
        limit 1
        "#,
    )
    .bind(policy_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;

    Ok(row.map(|value| to_anomaly_policy_view(value, OffsetDateTime::now_utc())))
}

pub(super) fn to_anomaly_policy_view(
    row: GatewayAnalysisAnomalyPolicyRow,
    now: OffsetDateTime,
) -> GatewayAnalysisAnomalyPolicyView {
    let status = normalize_anomaly_policy_status(Some(&row.status));
    let last_synced_at = row.last_synced_at.map(format_timestamp);
    let (next_sync_due_at, sync_due) = resolve_anomaly_policy_schedule(
        &status,
        row.auto_sync_enabled,
        row.auto_sync_interval_minutes,
        last_synced_at.as_deref(),
        now,
    );
    GatewayAnalysisAnomalyPolicyView {
        id: row.id,
        name: row.name,
        status,
        project_id: row.project_id,
        route_policy_id: row.route_policy_id,
        tag: row.tag,
        text_mode: row.text_mode,
        profile_key: normalize_anomaly_profile_key(Some(&row.profile_key)),
        thresholds: row.thresholds.0,
        auto_sync_enabled: row.auto_sync_enabled,
        auto_sync_interval_minutes: row.auto_sync_interval_minutes,
        last_synced_at,
        last_sync_status: normalize_anomaly_policy_sync_status(row.last_sync_status.as_deref()),
        last_sync_error: row.last_sync_error,
        next_sync_due_at,
        sync_due,
        auto_escalate_enabled: row.auto_escalate_enabled,
        escalate_severity_threshold: row.escalate_severity_threshold,
        escalate_after_sync_count: row.escalate_after_sync_count,
        auto_escalate_owner_user_id: row.auto_escalate_owner_user_id,
        auto_escalate_follow_up_status: row.auto_escalate_follow_up_status,
        auto_remediation_enabled: row.auto_remediation_enabled,
        auto_remediation_interval_minutes: row.auto_remediation_interval_minutes,
        auto_remediation_dry_run_first: row.auto_remediation_dry_run_first,
        auto_remediation_action_keys: row.auto_remediation_action_keys.map(|value| value.0),
        auto_remediation_max_apply_runs_per_incident: row
            .auto_remediation_max_apply_runs_per_incident,
        auto_remediation_require_alert_before_apply: row
            .auto_remediation_require_alert_before_apply,
        auto_remediation_freeze_on_provider_health_degrade: row
            .auto_remediation_freeze_on_provider_health_degrade,
        alerting_enabled: row.alerting_enabled,
        alert_interval_minutes: row.alert_interval_minutes,
        notify_operators_on_escalation: row.notify_operators_on_escalation,
        notify_owner_on_escalation: row.notify_owner_on_escalation,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

pub(super) fn resolve_anomaly_policy_schedule(
    status: &str,
    auto_sync_enabled: bool,
    auto_sync_interval_minutes: Option<i32>,
    last_synced_at: Option<&str>,
    now: OffsetDateTime,
) -> (Option<String>, bool) {
    if status != "enabled" || !auto_sync_enabled {
        return (None, false);
    }

    let interval_minutes = auto_sync_interval_minutes.unwrap_or(60).max(1);
    let Some(last_synced_at) = trimmed_owned_ref_opt(last_synced_at) else {
        return (None, true);
    };
    let Ok(reference_time) = OffsetDateTime::parse(last_synced_at, &Rfc3339) else {
        return (None, true);
    };

    let next_sync_due_at = reference_time + time::Duration::minutes(i64::from(interval_minutes));
    (
        Some(format_timestamp(next_sync_due_at)),
        next_sync_due_at <= now,
    )
}
