use super::*;
use deadpool_redis::redis::AsyncCommands;

fn resolve_anomaly_policy_auto_remediation_config(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> AutoRemediationConfig {
    AutoRemediationConfig {
        auto_remediation_enabled: policy.auto_remediation_enabled,
        auto_remediation_interval_minutes: policy.auto_remediation_interval_minutes.unwrap_or(180),
        auto_remediation_dry_run_first: policy.auto_remediation_dry_run_first,
        auto_remediation_action_keys: policy.auto_remediation_action_keys.clone(),
        auto_remediation_max_apply_runs_per_incident: policy
            .auto_remediation_max_apply_runs_per_incident,
        auto_remediation_require_alert_before_apply: policy
            .auto_remediation_require_alert_before_apply,
        auto_remediation_freeze_on_provider_health_degrade: policy
            .auto_remediation_freeze_on_provider_health_degrade,
    }
}

pub(super) fn resolve_rate_limit_hotspot_auto_remediation_config(
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    let disabled = || AutoRemediationConfig {
        auto_remediation_enabled: false,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(Vec::new()),
        auto_remediation_max_apply_runs_per_incident: None,
        auto_remediation_require_alert_before_apply: false,
        auto_remediation_freeze_on_provider_health_degrade: true,
    };
    if incident.route_policy_id.is_none() {
        return disabled();
    }

    if let Some(configured_profile) = route_policy
        .and_then(|item| item.config.rate_limit_hotspot_auto_remediation.as_ref())
        .and_then(|value| {
            serde_json::from_value::<GatewayRoutePolicyRateLimitHotspotAutoRemediationProfile>(
                value.clone(),
            )
            .ok()
        })
    {
        let action_key = configured_profile
            .action_by_code
            .as_ref()
            .and_then(|items| items.get(&incident.code))
            .cloned()
            .flatten();
        return AutoRemediationConfig {
            auto_remediation_enabled: configured_profile.enabled.unwrap_or(true),
            auto_remediation_interval_minutes: configured_profile.interval_minutes.unwrap_or(180),
            auto_remediation_dry_run_first: configured_profile.dry_run_first.unwrap_or(true),
            auto_remediation_action_keys: Some(action_key.into_iter().collect()),
            auto_remediation_max_apply_runs_per_incident: configured_profile
                .max_apply_runs_per_incident,
            auto_remediation_require_alert_before_apply: configured_profile
                .require_alert_before_apply
                .unwrap_or(true),
            auto_remediation_freeze_on_provider_health_degrade: configured_profile
                .freeze_on_provider_health_degrade
                .unwrap_or(true),
        };
    }

    let action_keys = match incident.code.as_str() {
        "rate_limit_request_spike"
        | "rate_limit_code_concentration"
        | "rate_limit_project_hotspot" => vec!["tighten-project-rate-limit".to_string()],
        "rate_limit_api_key_hotspot" => vec!["tighten-api-key-rate-limit".to_string()],
        "rate_limit_model_hotspot" => vec!["tighten-model-rate-limit".to_string()],
        "rate_limit_endpoint_hotspot" => vec!["tighten-endpoint-rate-limit".to_string()],
        _ => Vec::new(),
    };
    if action_keys.is_empty() {
        return disabled();
    }

    AutoRemediationConfig {
        auto_remediation_enabled: true,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(action_keys),
        auto_remediation_max_apply_runs_per_incident: Some(1),
        auto_remediation_require_alert_before_apply: true,
        auto_remediation_freeze_on_provider_health_degrade: true,
    }
}

fn resolve_routing_anomaly_auto_remediation_config(
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    let disabled = || AutoRemediationConfig {
        auto_remediation_enabled: false,
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(Vec::new()),
        auto_remediation_max_apply_runs_per_incident: None,
        auto_remediation_require_alert_before_apply: false,
        auto_remediation_freeze_on_provider_health_degrade: true,
    };
    let Some(route_policy) = route_policy else {
        return disabled();
    };
    if incident.route_policy_id.is_none() {
        return disabled();
    }

    if let Some(configured_profile) = route_policy
        .config
        .routing_anomaly_auto_remediation
        .as_ref()
        .and_then(|value| {
            serde_json::from_value::<GatewayRoutePolicyRoutingAnomalyAutoRemediationProfile>(
                value.clone(),
            )
            .ok()
        })
    {
        let action_keys = configured_profile
            .action_keys_by_code
            .as_ref()
            .and_then(|items| items.get(&incident.code))
            .cloned()
            .unwrap_or_default();
        return AutoRemediationConfig {
            auto_remediation_enabled: configured_profile.enabled.unwrap_or(true),
            auto_remediation_interval_minutes: configured_profile.interval_minutes.unwrap_or(180),
            auto_remediation_dry_run_first: configured_profile.dry_run_first.unwrap_or(true),
            auto_remediation_action_keys: Some(action_keys),
            auto_remediation_max_apply_runs_per_incident: configured_profile
                .max_apply_runs_per_incident,
            auto_remediation_require_alert_before_apply: configured_profile
                .require_alert_before_apply
                .unwrap_or(true),
            auto_remediation_freeze_on_provider_health_degrade: configured_profile
                .freeze_on_provider_health_degrade
                .unwrap_or(true),
        };
    }

    let allow_provider_isolation = route_policy
        .config
        .allowed_provider_account_ids
        .as_ref()
        .map(|items| items.len() > 1)
        .unwrap_or(false);
    let action_keys = match incident.code.as_str() {
        "saturated_provider_route_spike" => vec!["reduce-provider-concurrency".to_string()],
        "breaker_open_provider_route_detected" => {
            let mut keys = Vec::new();
            if allow_provider_isolation {
                keys.push("provider-isolation".to_string());
            }
            keys.push("disable-prestream-fallback".to_string());
            keys
        }
        "failure_rate_spike"
        | "completion_rate_drop"
        | "provider_routing_score_drop"
        | "degraded_provider_route_spike" => {
            let mut keys = vec![
                "disable-prestream-fallback".to_string(),
                "reduce-provider-concurrency".to_string(),
            ];
            if allow_provider_isolation {
                keys.push("provider-isolation".to_string());
            }
            keys
        }
        _ => Vec::new(),
    };

    AutoRemediationConfig {
        auto_remediation_enabled: !action_keys.is_empty(),
        auto_remediation_interval_minutes: 180,
        auto_remediation_dry_run_first: true,
        auto_remediation_action_keys: Some(action_keys),
        auto_remediation_max_apply_runs_per_incident: Some(1),
        auto_remediation_require_alert_before_apply: true,
        auto_remediation_freeze_on_provider_health_degrade: true,
    }
}

pub(super) fn resolve_incident_auto_remediation_config(
    policy: Option<&GatewayAnalysisAnomalyPolicyView>,
    incident: &GatewayAnalysisAnomalyIncidentView,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> AutoRemediationConfig {
    if let Some(policy) = policy {
        return resolve_anomaly_policy_auto_remediation_config(policy);
    }
    let hotspot = resolve_rate_limit_hotspot_auto_remediation_config(incident, route_policy);
    if hotspot.auto_remediation_enabled {
        return hotspot;
    }
    resolve_routing_anomaly_auto_remediation_config(incident, route_policy)
}

pub(super) async fn read_route_policy_health_degraded(
    pool: &PgPool,
    redis_pool: &RedisPool,
    route_policy: Option<&GatewayRoutePolicyView>,
) -> Result<bool, GatewayError> {
    let provider_ids = route_policy
        .and_then(|item| item.config.allowed_provider_account_ids.as_ref())
        .cloned()
        .unwrap_or_default();
    if provider_ids.is_empty() {
        return Ok(false);
    }

    #[derive(Debug, Clone, FromRow)]
    struct ProviderStatusRow {
        status: String,
    }

    let provider_rows = sqlx::query_as::<_, ProviderStatusRow>(
        r#"
        select status
        from gateway_provider_accounts
        where id = any($1)
        "#,
    )
    .bind(&provider_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    if provider_rows.iter().any(|row| row.status != "active") {
        return Ok(true);
    }

    if let Ok(mut connection) = redis_pool.get().await {
        for provider_id in &provider_ids {
            if connection
                .exists::<_, bool>(provider_breaker_open_key(provider_id))
                .await
                .unwrap_or(false)
            {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GatewayRoutePolicyRoutingAnomalyAutoRemediationProfile {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    dry_run_first: Option<bool>,
    require_alert_before_apply: Option<bool>,
    freeze_on_provider_health_degrade: Option<bool>,
    max_apply_runs_per_incident: Option<i32>,
    action_keys_by_code: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GatewayRoutePolicyRateLimitHotspotAutoRemediationProfile {
    enabled: Option<bool>,
    interval_minutes: Option<i32>,
    dry_run_first: Option<bool>,
    require_alert_before_apply: Option<bool>,
    freeze_on_provider_health_degrade: Option<bool>,
    max_apply_runs_per_incident: Option<i32>,
    action_by_code: Option<HashMap<String, Option<String>>>,
}
