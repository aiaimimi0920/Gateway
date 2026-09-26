use super::policies::find_anomaly_policy_by_id;
use super::policy_parameters::normalize_anomaly_policy_sync_status;
use super::policy_sync_filters::{
    build_ad_hoc_analysis_export_filters, build_ad_hoc_request_audit_filters,
    build_analysis_export_auto_escalation_config, build_policy_analysis_export_filters,
    build_policy_request_audit_filters, has_request_audit_specific_sync_hints,
    split_policy_tag_parts,
};
use super::*;

pub async fn sync_anomaly_policy(
    pool: &PgPool,
    policy_id: &str,
) -> Result<GatewayAnalysisAnomalyPolicySyncView, GatewayError> {
    let policy_id = trimmed_owned_ref(policy_id)
        .ok_or_else(|| GatewayError::bad_request("policyId 不能为空"))?;
    let policy = find_anomaly_policy_by_id(pool, policy_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly policy 不存在"))?;
    let timestamp = OffsetDateTime::now_utc();

    let sync_result = match determine_supported_policy_sync_kind(&policy) {
        SupportedPolicySyncKind::ProviderRouting => {
            let result = sync_provider_routing_anomaly_incidents(
                pool,
                &build_policy_request_audit_filters(&policy),
                Some(policy.profile_key.as_str()),
                GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::ProviderRouting(result)
        }
        SupportedPolicySyncKind::RateLimitHotspot => {
            let filters = build_policy_request_audit_filters(&policy);
            let snapshot = persist_rate_limit_hotspot_anomaly_snapshot(
                pool,
                &filters,
                Some(policy.name.as_str()),
                Some(24),
                Some(policy.profile_key.as_str()),
                GatewayRateLimitHotspotAnomalyOverrides::default(),
            )
            .await?;
            let result =
                sync_rate_limit_hotspot_anomaly_incidents(pool, &snapshot.snapshot_id).await?;
            PolicySyncResult::RateLimitHotspot(result)
        }
        SupportedPolicySyncKind::AnalysisExport => {
            let export_filters = build_policy_analysis_export_filters(&policy);
            let report = get_analysis_export_anomaly_report(
                pool,
                &export_filters,
                Some(policy.id.as_str()),
                Some(policy.profile_key.as_str()),
                GatewayAnalysisExportAnomalyOverrides::default(),
            )
            .await?;
            let result = sync_analysis_export_anomaly_incidents(
                pool,
                report,
                GatewaySyncAnalysisExportAnomalyIncidentsInput {
                    policy_id: Some(policy.id.clone()),
                    project_id: export_filters.project_id.clone(),
                    route_policy_id: policy.route_policy_id.clone(),
                    tag: export_filters.tag.clone(),
                    text_mode: export_filters.text_mode.clone(),
                    auto_escalation: build_analysis_export_auto_escalation_config(Some(&policy)),
                },
            )
            .await?;
            PolicySyncResult::AnalysisExport(result)
        }
        SupportedPolicySyncKind::Unsupported(reason) => {
            update_anomaly_policy_sync_state(
                pool,
                policy_id,
                "error",
                timestamp,
                Some(reason.as_str()),
            )
            .await?;
            return Err(GatewayError::conflict(reason));
        }
    };

    update_anomaly_policy_sync_state(pool, policy_id, "ok", timestamp, None).await?;
    let refreshed_policy = find_anomaly_policy_by_id(pool, policy_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("Gateway analysis anomaly policy 同步后不存在"))?;

    Ok(GatewayAnalysisAnomalyPolicySyncView {
        policy: refreshed_policy,
        sync_kind: sync_result.kind().to_string(),
        anomaly_count: sync_result.anomaly_count(),
        opened_incident_count: sync_result.opened_incident_count(),
        updated_incident_count: sync_result.updated_incident_count(),
        resolved_incident_count: sync_result.resolved_incident_count(),
        sync: sync_result.into_value()?,
    })
}

pub async fn sync_anomaly_incidents(
    pool: &PgPool,
    input: GatewayAnalysisAnomalyIncidentSyncInput,
) -> Result<GatewayAnalysisAnomalyIncidentSyncView, GatewayError> {
    let policy = if let Some(policy_id) = trimmed_owned_ref_opt(input.policy_id.as_deref()) {
        find_anomaly_policy_by_id(pool, policy_id).await?
    } else {
        None
    };
    let effective_tag = trimmed_owned_ref_opt(input.tag.as_deref())
        .map(str::to_string)
        .or_else(|| {
            policy
                .as_ref()
                .and_then(|item| trimmed_owned(item.tag.as_deref()))
        });
    let sync_kind = determine_supported_ad_hoc_sync_kind(effective_tag.as_deref(), &input);
    let filters =
        build_ad_hoc_request_audit_filters(policy.as_ref(), &input, effective_tag.as_deref());
    let effective_profile_key = trimmed_owned_ref_opt(input.profile_key.as_deref())
        .map(str::to_string)
        .or_else(|| {
            effective_tag
                .as_deref()
                .and_then(|tag| split_policy_tag_parts(tag).0)
        })
        .or_else(|| policy.as_ref().map(|item| item.profile_key.clone()));

    let sync_result = match sync_kind {
        SupportedPolicySyncKind::ProviderRouting => PolicySyncResult::ProviderRouting(
            sync_provider_routing_anomaly_incidents(
                pool,
                &filters,
                effective_profile_key.as_deref(),
                GatewayProviderRoutingAnalysisAnomalyOverrides::default(),
            )
            .await?,
        ),
        SupportedPolicySyncKind::RateLimitHotspot => {
            let snapshot = persist_rate_limit_hotspot_anomaly_snapshot(
                pool,
                &filters,
                trimmed_owned_ref_opt(input.label.as_deref()),
                None,
                effective_profile_key.as_deref(),
                GatewayRateLimitHotspotAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::RateLimitHotspot(
                sync_rate_limit_hotspot_anomaly_incidents(pool, &snapshot.snapshot_id).await?,
            )
        }
        SupportedPolicySyncKind::AnalysisExport => {
            let export_filters = build_ad_hoc_analysis_export_filters(
                policy.as_ref(),
                &input,
                effective_tag.as_deref(),
            );
            let report = get_analysis_export_anomaly_report(
                pool,
                &export_filters,
                policy.as_ref().map(|item| item.id.as_str()),
                effective_profile_key.as_deref(),
                GatewayAnalysisExportAnomalyOverrides::default(),
            )
            .await?;
            PolicySyncResult::AnalysisExport(
                sync_analysis_export_anomaly_incidents(
                    pool,
                    report,
                    GatewaySyncAnalysisExportAnomalyIncidentsInput {
                        policy_id: policy.as_ref().map(|item| item.id.clone()),
                        project_id: export_filters.project_id.clone(),
                        route_policy_id: input.route_policy_id.clone().or_else(|| {
                            policy
                                .as_ref()
                                .and_then(|item| item.route_policy_id.clone())
                        }),
                        tag: export_filters.tag.clone(),
                        text_mode: export_filters.text_mode.clone(),
                        auto_escalation: build_analysis_export_auto_escalation_config(
                            policy.as_ref(),
                        ),
                    },
                )
                .await?,
            )
        }
        SupportedPolicySyncKind::Unsupported(reason) => {
            return Err(GatewayError::conflict(reason));
        }
    };

    Ok(GatewayAnalysisAnomalyIncidentSyncView {
        policy,
        sync_kind: sync_result.kind().to_string(),
        anomaly_count: sync_result.anomaly_count(),
        opened_incident_count: sync_result.opened_incident_count(),
        updated_incident_count: sync_result.updated_incident_count(),
        resolved_incident_count: sync_result.resolved_incident_count(),
        sync: sync_result.into_value()?,
    })
}

fn determine_supported_policy_sync_kind(
    policy: &GatewayAnalysisAnomalyPolicyView,
) -> SupportedPolicySyncKind {
    match determine_supported_sync_kind_from_tag(policy.tag.as_deref()) {
        SupportedPolicySyncKind::Unsupported(_) => SupportedPolicySyncKind::AnalysisExport,
        supported => supported,
    }
}

pub(super) fn determine_supported_sync_kind_from_tag(tag: Option<&str>) -> SupportedPolicySyncKind {
    let Some(tag) = trimmed_owned_ref_opt(tag) else {
        return SupportedPolicySyncKind::Unsupported(
            "当前 anomaly policy 缺少可识别 tag，Rust 还无法判断应同步哪类异常源。".to_string(),
        );
    };
    if tag.starts_with("provider-routing:") {
        return SupportedPolicySyncKind::ProviderRouting;
    }
    if tag.starts_with("rate-limit-hotspot:") {
        return SupportedPolicySyncKind::RateLimitHotspot;
    }
    if tag.starts_with("analysis-export:") {
        return SupportedPolicySyncKind::AnalysisExport;
    }
    SupportedPolicySyncKind::Unsupported(format!(
        "当前 Rust 仅支持 provider-routing / rate-limit-hotspot / analysis-export anomaly sync，收到 tag: {tag}"
    ))
}

pub(super) fn determine_supported_ad_hoc_sync_kind(
    tag: Option<&str>,
    input: &GatewayAnalysisAnomalyIncidentSyncInput,
) -> SupportedPolicySyncKind {
    match determine_supported_sync_kind_from_tag(tag) {
        SupportedPolicySyncKind::Unsupported(_) => {
            if has_request_audit_specific_sync_hints(input) {
                SupportedPolicySyncKind::Unsupported(
                    "当前 ad-hoc anomaly sync 若使用 request-audit 维度，必须显式提供 provider-routing 或 rate-limit-hotspot tag。".to_string(),
                )
            } else {
                SupportedPolicySyncKind::AnalysisExport
            }
        }
        supported => supported,
    }
}

async fn update_anomaly_policy_sync_state(
    pool: &PgPool,
    policy_id: &str,
    status: &str,
    synced_at: OffsetDateTime,
    error: Option<&str>,
) -> Result<(), GatewayError> {
    let policy_id = trimmed_owned_ref(policy_id)
        .ok_or_else(|| GatewayError::bad_request("policyId 不能为空"))?;
    let sync_status = normalize_anomaly_policy_sync_status(Some(status))
        .ok_or_else(|| GatewayError::bad_request("lastSyncStatus 不合法"))?;
    let error = error
        .and_then(trimmed_owned_ref)
        .map(|value| truncate_error_summary(value, 2_000));
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_policies
        set
          last_synced_at = $2,
          last_sync_status = $3,
          last_sync_error = $4,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(policy_id)
    .bind(synced_at)
    .bind(sync_status)
    .bind(error.as_deref())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub(super) fn truncate_error_summary(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut count = 0usize;
    for ch in value.chars() {
        if count == max_chars {
            break;
        }
        result.push(ch);
        count += 1;
    }
    result
}

enum PolicySyncResult {
    ProviderRouting(GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult),
    RateLimitHotspot(GatewaySyncRateLimitHotspotAnomalyIncidentsResult),
    AnalysisExport(GatewaySyncAnalysisExportAnomalyIncidentsResult),
}

impl PolicySyncResult {
    fn kind(&self) -> &'static str {
        match self {
            Self::ProviderRouting(_) => "provider_routing",
            Self::RateLimitHotspot(_) => "rate_limit_hotspot",
            Self::AnalysisExport(_) => "analysis_export",
        }
    }

    fn anomaly_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.report.anomalies.len(),
            Self::RateLimitHotspot(result) => result.snapshot.report.anomalies.len(),
            Self::AnalysisExport(result) => result.report.anomalies.len(),
        }
    }

    fn opened_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.opened_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.opened_incident_ids.len(),
            Self::AnalysisExport(result) => result.opened_incident_ids.len(),
        }
    }

    fn updated_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.updated_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.updated_incident_ids.len(),
            Self::AnalysisExport(result) => result.updated_incident_ids.len(),
        }
    }

    fn resolved_incident_count(&self) -> usize {
        match self {
            Self::ProviderRouting(result) => result.resolved_incident_ids.len(),
            Self::RateLimitHotspot(result) => result.resolved_incident_ids.len(),
            Self::AnalysisExport(result) => result.resolved_incident_ids.len(),
        }
    }

    fn into_value(self) -> Result<Value, GatewayError> {
        serde_json::to_value(self).map_err(|error| {
            GatewayError::server_error(format!("serialize anomaly policy sync result: {error}"))
        })
    }
}

impl Serialize for PolicySyncResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::ProviderRouting(result) => result.serialize(serializer),
            Self::RateLimitHotspot(result) => result.serialize(serializer),
            Self::AnalysisExport(result) => result.serialize(serializer),
        }
    }
}
