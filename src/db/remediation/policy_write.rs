use super::plan_context::find_route_policy_by_id;
use super::policies::to_anomaly_policy_view;
use super::policy_parameters::{
    build_analysis_anomaly_threshold_config, normalize_anomaly_policy_status,
    normalize_anomaly_profile_key, normalize_anomaly_severity, normalize_follow_up_status,
    normalize_non_negative_int, normalize_string_list,
};
use super::*;

pub async fn save_anomaly_policy(
    pool: &PgPool,
    input: UpsertGatewayAnalysisAnomalyPolicyInput,
) -> Result<GatewayAnalysisAnomalyPolicyView, GatewayError> {
    let policy_id = input
        .id
        .and_then(|value| trimmed_owned(Some(&value)))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let name = trimmed_owned_ref(&input.name)
        .ok_or_else(|| GatewayError::bad_request("Policy 名称不能为空"))?;
    if name.chars().count() > 120 {
        return Err(GatewayError::bad_request("Policy 名称不能超过 120 个字符"));
    }

    let status = normalize_anomaly_policy_status(input.status.as_deref());
    let profile_key = normalize_anomaly_profile_key(input.profile_key.as_deref());
    let thresholds =
        build_analysis_anomaly_threshold_config(&profile_key, input.thresholds.as_ref());
    let tag = trimmed_owned(input.tag.as_deref()).map(|value| value.to_ascii_lowercase());
    if tag.as_ref().is_some_and(|value| value.chars().count() > 40) {
        return Err(GatewayError::bad_request("tag 不能超过 40 个字符"));
    }
    let route_policy =
        if let Some(route_policy_id) = trimmed_owned(input.route_policy_id.as_deref()) {
            find_route_policy_by_id(pool, &route_policy_id)
                .await?
                .ok_or_else(|| GatewayError::not_found("绑定的 route policy 不存在"))?
                .into()
        } else {
            None
        };
    let requested_project_id = trimmed_owned(input.project_id.as_deref());
    if let (Some(project_id), Some(route_policy)) =
        (requested_project_id.as_ref(), route_policy.as_ref())
    {
        if route_policy.project_id != *project_id {
            return Err(GatewayError::conflict(
                "anomaly policy 的 projectId 必须与 route policy 所属 project 一致。",
            ));
        }
    }
    let project_id =
        requested_project_id.or_else(|| route_policy.as_ref().map(|item| item.project_id.clone()));

    let auto_sync_enabled = input.auto_sync_enabled.unwrap_or(false);
    let auto_sync_interval_minutes = if auto_sync_enabled {
        normalize_non_negative_int(
            input.auto_sync_interval_minutes,
            Some(60),
            10_080,
            "autoSyncIntervalMinutes",
        )?
    } else {
        None
    };
    let auto_escalate_enabled = input.auto_escalate_enabled.unwrap_or(false);
    let escalate_severity_threshold = if auto_escalate_enabled {
        normalize_anomaly_severity(
            input
                .escalate_severity_threshold
                .as_deref()
                .or(Some("critical")),
        )
    } else {
        None
    };
    let escalate_after_sync_count = if auto_escalate_enabled {
        normalize_non_negative_int(
            input.escalate_after_sync_count,
            Some(3),
            1_000,
            "escalateAfterSyncCount",
        )?
    } else {
        None
    };
    let auto_escalate_owner_user_id = if auto_escalate_enabled {
        trimmed_owned(input.auto_escalate_owner_user_id.as_deref())
    } else {
        None
    };
    let auto_escalate_follow_up_status = if auto_escalate_enabled {
        normalize_follow_up_status(
            input
                .auto_escalate_follow_up_status
                .as_deref()
                .or(Some("investigating")),
        )
    } else {
        None
    };

    let auto_remediation_enabled = input.auto_remediation_enabled.unwrap_or(false);
    let auto_remediation_interval_minutes = if auto_remediation_enabled {
        normalize_non_negative_int(
            input.auto_remediation_interval_minutes,
            Some(180),
            10_080,
            "autoRemediationIntervalMinutes",
        )?
    } else {
        None
    };
    let auto_remediation_dry_run_first = if auto_remediation_enabled {
        input.auto_remediation_dry_run_first.unwrap_or(true)
    } else {
        true
    };
    let auto_remediation_action_keys = if auto_remediation_enabled {
        normalize_string_list(input.auto_remediation_action_keys.as_deref())
    } else {
        None
    };
    let auto_remediation_max_apply_runs_per_incident = if auto_remediation_enabled {
        normalize_non_negative_int(
            input.auto_remediation_max_apply_runs_per_incident,
            None,
            1_000,
            "autoRemediationMaxApplyRunsPerIncident",
        )?
    } else {
        None
    };
    let auto_remediation_require_alert_before_apply = if auto_remediation_enabled {
        input
            .auto_remediation_require_alert_before_apply
            .unwrap_or(false)
    } else {
        false
    };
    let auto_remediation_freeze_on_provider_health_degrade = if auto_remediation_enabled {
        input
            .auto_remediation_freeze_on_provider_health_degrade
            .unwrap_or(true)
    } else {
        true
    };

    let alerting_enabled = input.alerting_enabled.unwrap_or(true);
    let alert_interval_minutes = if alerting_enabled {
        normalize_non_negative_int(
            input.alert_interval_minutes,
            Some(DEFAULT_GATEWAY_ANALYSIS_ANOMALY_ALERT_INTERVAL_MINUTES),
            10_080,
            "alertIntervalMinutes",
        )?
    } else {
        None
    };
    let notify_operators_on_escalation = if alerting_enabled {
        input.notify_operators_on_escalation.unwrap_or(true)
    } else {
        false
    };
    let notify_owner_on_escalation = if alerting_enabled {
        input.notify_owner_on_escalation.unwrap_or(true)
    } else {
        false
    };

    let timestamp = OffsetDateTime::now_utc();
    let row = sqlx::query_as::<_, GatewayAnalysisAnomalyPolicyRow>(
        r#"
        insert into gateway_analysis_anomaly_policies (
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
        ) values (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$28
        )
        on conflict (id) do update set
          name = excluded.name,
          status = excluded.status,
          project_id = excluded.project_id,
          route_policy_id = excluded.route_policy_id,
          tag = excluded.tag,
          text_mode = excluded.text_mode,
          profile_key = excluded.profile_key,
          thresholds = excluded.thresholds,
          auto_sync_enabled = excluded.auto_sync_enabled,
          auto_sync_interval_minutes = excluded.auto_sync_interval_minutes,
          auto_escalate_enabled = excluded.auto_escalate_enabled,
          escalate_severity_threshold = excluded.escalate_severity_threshold,
          escalate_after_sync_count = excluded.escalate_after_sync_count,
          auto_escalate_owner_user_id = excluded.auto_escalate_owner_user_id,
          auto_escalate_follow_up_status = excluded.auto_escalate_follow_up_status,
          auto_remediation_enabled = excluded.auto_remediation_enabled,
          auto_remediation_interval_minutes = excluded.auto_remediation_interval_minutes,
          auto_remediation_dry_run_first = excluded.auto_remediation_dry_run_first,
          auto_remediation_action_keys = excluded.auto_remediation_action_keys,
          auto_remediation_max_apply_runs_per_incident = excluded.auto_remediation_max_apply_runs_per_incident,
          auto_remediation_require_alert_before_apply = excluded.auto_remediation_require_alert_before_apply,
          auto_remediation_freeze_on_provider_health_degrade = excluded.auto_remediation_freeze_on_provider_health_degrade,
          alerting_enabled = excluded.alerting_enabled,
          alert_interval_minutes = excluded.alert_interval_minutes,
          notify_operators_on_escalation = excluded.notify_operators_on_escalation,
          notify_owner_on_escalation = excluded.notify_owner_on_escalation,
          updated_at = excluded.updated_at
        returning
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
        "#,
    )
    .bind(policy_id)
    .bind(name)
    .bind(status)
    .bind(project_id)
    .bind(route_policy.as_ref().map(|item| item.id.as_str()))
    .bind(tag)
    .bind(trimmed_owned(input.text_mode.as_deref()))
    .bind(profile_key)
    .bind(sqlx::types::Json(thresholds))
    .bind(auto_sync_enabled)
    .bind(auto_sync_interval_minutes)
    .bind(auto_escalate_enabled)
    .bind(escalate_severity_threshold)
    .bind(escalate_after_sync_count)
    .bind(auto_escalate_owner_user_id)
    .bind(auto_escalate_follow_up_status)
    .bind(auto_remediation_enabled)
    .bind(auto_remediation_interval_minutes)
    .bind(auto_remediation_dry_run_first)
    .bind(auto_remediation_action_keys.map(sqlx::types::Json))
    .bind(auto_remediation_max_apply_runs_per_incident)
    .bind(auto_remediation_require_alert_before_apply)
    .bind(auto_remediation_freeze_on_provider_health_degrade)
    .bind(alerting_enabled)
    .bind(alert_interval_minutes)
    .bind(notify_operators_on_escalation)
    .bind(notify_owner_on_escalation)
    .bind(timestamp)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_anomaly_policy_view(row, timestamp))
}
