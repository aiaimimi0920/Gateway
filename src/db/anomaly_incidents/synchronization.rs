use super::*;

pub async fn sync_provider_routing_anomaly_incidents(
    pool: &PgPool,
    filters: &RequestAuditFilters,
    profile_key: Option<&str>,
    overrides: GatewayProviderRoutingAnalysisAnomalyOverrides,
) -> Result<GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult, GatewayError> {
    let report = get_provider_routing_anomaly_report(pool, filters, profile_key, overrides).await?;
    let timestamp = OffsetDateTime::now_utc();
    let project_id = trimmed_owned(report.filters.project_id.as_deref());
    let route_policy_id = trimmed_owned(report.filters.route_policy_id.as_deref());
    let tag = build_provider_routing_incident_tag(&report);

    let existing_rows = fetch_scope_incidents(
        pool,
        None,
        project_id.as_deref(),
        route_policy_id.as_deref(),
        Some(tag.as_str()),
        None,
    )
    .await?;
    let existing_by_fingerprint = existing_rows
        .iter()
        .map(|row| (row.fingerprint.clone(), row.clone()))
        .collect::<HashMap<_, _>>();

    let mut opened_incident_ids = Vec::new();
    let mut updated_incident_ids = Vec::new();
    let mut resolved_incident_ids = Vec::new();
    let mut seen_fingerprints = HashSet::new();

    for anomaly in &report.anomalies {
        let fingerprint = build_adhoc_incident_fingerprint(
            None,
            project_id.as_deref(),
            route_policy_id.as_deref(),
            Some(tag.as_str()),
            None,
            &anomaly.code,
        );
        seen_fingerprints.insert(fingerprint.clone());

        if let Some(existing) = existing_by_fingerprint.get(&fingerprint) {
            let incident_id = update_provider_routing_incident(
                pool,
                existing,
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_str(),
                anomaly,
                timestamp,
            )
            .await?;
            updated_incident_ids.push(incident_id);
        } else {
            let incident_id = create_provider_routing_incident(
                pool,
                &fingerprint,
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_str(),
                anomaly,
                timestamp,
            )
            .await?;
            opened_incident_ids.push(incident_id);
        }
    }

    for row in existing_rows {
        if seen_fingerprints.contains(&row.fingerprint)
            || normalize_incident_status(&row.status) == "resolved"
        {
            continue;
        }
        resolve_provider_routing_incident(pool, &row, timestamp).await?;
        resolved_incident_ids.push(row.id);
    }

    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            project_id,
            route_policy_id,
            tag: Some(tag),
            limit: Some(200),
            ..GatewayAnalysisAnomalyIncidentFilters::default()
        },
    )
    .await?;

    Ok(GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult {
        report,
        incidents,
        opened_incident_ids,
        updated_incident_ids,
        resolved_incident_ids,
    })
}

pub async fn sync_rate_limit_hotspot_anomaly_incidents(
    pool: &PgPool,
    snapshot_id: &str,
) -> Result<GatewaySyncRateLimitHotspotAnomalyIncidentsResult, GatewayError> {
    let snapshot = get_rate_limit_hotspot_anomaly_snapshot(snapshot_id).await?;
    let timestamp = OffsetDateTime::now_utc();
    let project_id = trimmed_owned(snapshot.filters.project_id.as_deref());
    let route_policy_id = trimmed_owned(snapshot.filters.route_policy_id.as_deref());
    let tag = build_rate_limit_hotspot_incident_tag(&snapshot);

    let existing_rows = fetch_scope_incidents(
        pool,
        None,
        project_id.as_deref(),
        route_policy_id.as_deref(),
        Some(tag.as_str()),
        None,
    )
    .await?;
    let existing_by_fingerprint = existing_rows
        .iter()
        .map(|row| (row.fingerprint.clone(), row.clone()))
        .collect::<HashMap<_, _>>();

    let mut opened_incident_ids = Vec::new();
    let mut updated_incident_ids = Vec::new();
    let mut resolved_incident_ids = Vec::new();
    let mut seen_fingerprints = HashSet::new();

    for anomaly in &snapshot.report.anomalies {
        let fingerprint = build_adhoc_incident_fingerprint(
            None,
            project_id.as_deref(),
            route_policy_id.as_deref(),
            Some(tag.as_str()),
            None,
            &anomaly.code,
        );
        seen_fingerprints.insert(fingerprint.clone());

        if let Some(existing) = existing_by_fingerprint.get(&fingerprint) {
            let incident_id = update_rate_limit_hotspot_incident(
                pool,
                existing,
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_str(),
                &snapshot,
                anomaly,
                timestamp,
            )
            .await?;
            updated_incident_ids.push(incident_id);
        } else {
            let incident_id = create_rate_limit_hotspot_incident(
                pool,
                &fingerprint,
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_str(),
                &snapshot,
                anomaly,
                timestamp,
            )
            .await?;
            opened_incident_ids.push(incident_id);
        }
    }

    for row in existing_rows {
        if seen_fingerprints.contains(&row.fingerprint)
            || normalize_incident_status(&row.status) == "resolved"
        {
            continue;
        }
        resolve_rate_limit_hotspot_incident(pool, &row, &snapshot, timestamp).await?;
        resolved_incident_ids.push(row.id);
    }

    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            project_id,
            route_policy_id,
            tag: Some(tag),
            limit: Some(200),
            ..GatewayAnalysisAnomalyIncidentFilters::default()
        },
    )
    .await?;

    Ok(GatewaySyncRateLimitHotspotAnomalyIncidentsResult {
        snapshot,
        incidents,
        opened_incident_ids,
        updated_incident_ids,
        resolved_incident_ids,
    })
}

pub async fn sync_analysis_export_anomaly_incidents(
    pool: &PgPool,
    report: GatewayAnalysisExportAnomalyReportView,
    input: GatewaySyncAnalysisExportAnomalyIncidentsInput,
) -> Result<GatewaySyncAnalysisExportAnomalyIncidentsResult, GatewayError> {
    let timestamp = OffsetDateTime::now_utc();
    let policy_id = trimmed_owned(input.policy_id.as_deref());
    let project_id = trimmed_owned(input.project_id.as_deref());
    let route_policy_id = trimmed_owned(input.route_policy_id.as_deref());
    let tag = trimmed_owned(input.tag.as_deref());
    let text_mode = trimmed_owned(input.text_mode.as_deref());

    let existing_rows = fetch_scope_incidents(
        pool,
        policy_id.as_deref(),
        project_id.as_deref(),
        route_policy_id.as_deref(),
        tag.as_deref(),
        text_mode.as_deref(),
    )
    .await?;
    let existing_by_fingerprint = existing_rows
        .iter()
        .map(|row| (row.fingerprint.clone(), row.clone()))
        .collect::<HashMap<_, _>>();

    let mut opened_incident_ids = Vec::new();
    let mut updated_incident_ids = Vec::new();
    let mut resolved_incident_ids = Vec::new();
    let mut seen_fingerprints = HashSet::new();

    for anomaly in &report.anomalies {
        let fingerprint = build_adhoc_incident_fingerprint(
            policy_id.as_deref(),
            project_id.as_deref(),
            route_policy_id.as_deref(),
            tag.as_deref(),
            text_mode.as_deref(),
            &anomaly.code,
        );
        seen_fingerprints.insert(fingerprint.clone());

        if let Some(existing) = existing_by_fingerprint.get(&fingerprint) {
            let incident_id = update_analysis_export_incident(
                pool,
                existing,
                policy_id.as_deref(),
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_deref(),
                text_mode.as_deref(),
                anomaly,
                &input.auto_escalation,
                timestamp,
            )
            .await?;
            updated_incident_ids.push(incident_id);
        } else {
            let incident_id = create_analysis_export_incident(
                pool,
                &fingerprint,
                policy_id.as_deref(),
                project_id.as_deref(),
                route_policy_id.as_deref(),
                tag.as_deref(),
                text_mode.as_deref(),
                anomaly,
                &input.auto_escalation,
                timestamp,
            )
            .await?;
            opened_incident_ids.push(incident_id);
        }
    }

    for row in existing_rows {
        if seen_fingerprints.contains(&row.fingerprint)
            || normalize_incident_status(&row.status) == "resolved"
        {
            continue;
        }
        resolve_analysis_export_incident(pool, &row, timestamp).await?;
        resolved_incident_ids.push(row.id);
    }

    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            policy_id,
            project_id,
            route_policy_id,
            tag,
            text_mode,
            limit: Some(200),
            ..GatewayAnalysisAnomalyIncidentFilters::default()
        },
    )
    .await?;

    Ok(GatewaySyncAnalysisExportAnomalyIncidentsResult {
        report,
        incidents,
        opened_incident_ids,
        updated_incident_ids,
        resolved_incident_ids,
    })
}

pub(super) fn build_provider_routing_incident_tag(
    report: &GatewayProviderRoutingAnalysisAnomalyReportView,
) -> String {
    let mut parts = vec![format!("provider-routing:{}", report.profile_key.trim())];
    if let Some(provider_account_id) = trimmed_owned(report.filters.provider_account_id.as_deref())
    {
        parts.push(format!("provider:{provider_account_id}"));
    }
    if let Some(protocol_family) = trimmed_owned(report.filters.protocol_family.as_deref()) {
        parts.push(format!("protocol:{}", protocol_family.to_lowercase()));
    }
    if let Some(endpoint_kind) = trimmed_owned(report.filters.endpoint_kind.as_deref()) {
        parts.push(format!("endpoint:{}", endpoint_kind.to_lowercase()));
    }
    if let Some(api_key_id) = trimmed_owned(report.filters.api_key_id.as_deref()) {
        parts.push(format!("api-key:{api_key_id}"));
    }
    if let Some(session_id) = trimmed_owned(report.filters.session_id.as_deref()) {
        parts.push(format!("session:{session_id}"));
    }
    if let Some(response_id) = trimmed_owned(report.filters.response_id.as_deref()) {
        parts.push(format!("response:{response_id}"));
    }
    if let Some(status) = trimmed_owned(report.filters.status.as_deref()) {
        parts.push(format!("status:{}", status.to_lowercase()));
    }
    parts.join(":")
}

fn build_rate_limit_hotspot_incident_tag(
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
) -> String {
    let mut parts = vec![format!(
        "rate-limit-hotspot:{}",
        snapshot.filters.profile_key.trim()
    )];
    if let Some(api_key_id) = trimmed_owned(snapshot.filters.api_key_id.as_deref()) {
        parts.push(format!("api-key:{api_key_id}"));
    }
    if let Some(endpoint_kind) = trimmed_owned(snapshot.filters.endpoint_kind.as_deref()) {
        parts.push(format!("endpoint:{}", endpoint_kind.to_lowercase()));
    }
    parts.join(":")
}

pub(super) fn build_adhoc_incident_fingerprint(
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    code: &str,
) -> String {
    if let Some(policy_id) = policy_id.and_then(trimmed_owned_ref) {
        return format!("policy:{policy_id}:code:{}", code.trim());
    }
    format!(
        "adhoc:project:{}:routePolicy:{}:tag:{}:textMode:{}:code:{}",
        project_id.and_then(trimmed_owned_ref).unwrap_or("*"),
        route_policy_id.and_then(trimmed_owned_ref).unwrap_or("*"),
        tag.and_then(trimmed_owned_ref).unwrap_or("*"),
        text_mode.and_then(trimmed_owned_ref).unwrap_or("*"),
        code.trim()
    )
}
