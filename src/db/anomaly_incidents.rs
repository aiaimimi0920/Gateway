use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::rate_limit_hotspots::GatewayRateLimitHotspotAnomalyView;
use crate::error::GatewayError;

use super::analysis_exports::{
    GatewayAnalysisExportAnomalyReportView, GatewayAnalysisExportAnomalyView,
};
use super::request_audits::{
    GatewayProviderRoutingAnalysisAnomalyView, GatewaySummaryBucketKeyView,
};
use super::{
    format_timestamp, get_provider_routing_anomaly_report, get_rate_limit_hotspot_anomaly_snapshot,
    map_db_error, GatewayProviderRoutingAnalysisAnomalyOverrides,
    GatewayProviderRoutingAnalysisAnomalyReportView, GatewayRateLimitHotspotAnomalySnapshotView,
    RequestAuditFilters,
};

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisAnomalyIncidentFilters {
    pub incident_id: Option<String>,
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub owner_user_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub status: Option<String>,
    pub follow_up_status: Option<String>,
    pub escalation_status: Option<String>,
    pub code: Option<String>,
    pub severity: Option<String>,
    pub due_only: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentView {
    pub id: String,
    pub policy_id: Option<String>,
    pub fingerprint: String,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub code: String,
    pub severity: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub follow_up_status: String,
    pub sync_hit_count: i32,
    pub escalation_status: String,
    pub escalated_at: Option<String>,
    pub escalation_reason: Option<String>,
    pub latest_note: Option<String>,
    pub resolution_note: Option<String>,
    pub last_action_at: Option<String>,
    pub last_alert_attempt_at: Option<String>,
    pub last_alerted_at: Option<String>,
    pub last_alert_severity: Option<String>,
    pub alert_delivery_count: i32,
    pub summary: String,
    pub latest_export_id: Option<String>,
    pub previous_export_id: Option<String>,
    pub latest_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub delta_value: Option<f64>,
    pub delta_ratio: Option<f64>,
    pub threshold_value: Option<f64>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub acknowledged_at: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentSummaryView {
    pub total_incidents: usize,
    pub open_incidents: usize,
    pub acknowledged_incidents: usize,
    pub resolved_incidents: usize,
    pub escalated_incidents: usize,
    pub by_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_severity: Vec<GatewaySummaryBucketKeyView>,
    pub by_code: Vec<GatewaySummaryBucketKeyView>,
    pub by_follow_up_status: Vec<GatewaySummaryBucketKeyView>,
    pub by_escalation_status: Vec<GatewaySummaryBucketKeyView>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentFollowUpInput {
    pub owner_user_id: Option<String>,
    pub follow_up_status: Option<String>,
    pub note: Option<String>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput {
    pub alerted_at: Option<String>,
    pub alert_severity: Option<String>,
    pub alert_level: Option<i32>,
    pub note: Option<String>,
    pub mailbox_recipient_count: Option<i32>,
    pub webhook_dispatched: Option<bool>,
    pub webhook_skipped_reason: Option<String>,
    pub remediation_action_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAnalysisAnomalyIncidentHistoryView {
    pub id: String,
    pub incident_id: String,
    pub event_type: String,
    pub actor_user_id: Option<String>,
    pub note: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncProviderRoutingAnalysisAnomalyIncidentsResult {
    pub report: GatewayProviderRoutingAnalysisAnomalyReportView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncRateLimitHotspotAnomalyIncidentsResult {
    pub snapshot: GatewayRateLimitHotspotAnomalySnapshotView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewaySyncAnalysisExportAnomalyIncidentsInput {
    pub policy_id: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub tag: Option<String>,
    pub text_mode: Option<String>,
    pub auto_escalation: GatewayAnalysisExportAutoEscalationConfig,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayAnalysisExportAutoEscalationConfig {
    pub enabled: bool,
    pub severity_threshold: Option<String>,
    pub after_sync_count: Option<i32>,
    pub owner_user_id: Option<String>,
    pub follow_up_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySyncAnalysisExportAnomalyIncidentsResult {
    pub report: GatewayAnalysisExportAnomalyReportView,
    pub incidents: Vec<GatewayAnalysisAnomalyIncidentView>,
    pub opened_incident_ids: Vec<String>,
    pub updated_incident_ids: Vec<String>,
    pub resolved_incident_ids: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentRow {
    id: String,
    policy_id: Option<String>,
    fingerprint: String,
    project_id: Option<String>,
    route_policy_id: Option<String>,
    tag: Option<String>,
    text_mode: Option<String>,
    code: String,
    severity: String,
    status: String,
    owner_user_id: Option<String>,
    follow_up_status: String,
    sync_hit_count: i32,
    escalation_status: String,
    escalated_at: Option<OffsetDateTime>,
    escalation_reason: Option<String>,
    latest_note: Option<String>,
    resolution_note: Option<String>,
    last_action_at: Option<OffsetDateTime>,
    last_alert_attempt_at: Option<OffsetDateTime>,
    last_alerted_at: Option<OffsetDateTime>,
    last_alert_severity: Option<String>,
    alert_delivery_count: i32,
    summary: String,
    latest_export_id: Option<String>,
    previous_export_id: Option<String>,
    latest_value: Option<f64>,
    previous_value: Option<f64>,
    delta_value: Option<f64>,
    delta_ratio: Option<f64>,
    threshold_value: Option<f64>,
    first_seen_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    acknowledged_at: Option<OffsetDateTime>,
    resolved_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayAnalysisAnomalyIncidentHistoryRow {
    id: String,
    incident_id: String,
    event_type: String,
    actor_user_id: Option<String>,
    note: Option<String>,
    metadata: Option<sqlx::types::Json<Value>>,
    created_at: OffsetDateTime,
}

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

pub async fn summarize_anomaly_incidents(
    pool: &PgPool,
    filters: &GatewayAnalysisAnomalyIncidentFilters,
) -> Result<GatewayAnalysisAnomalyIncidentSummaryView, GatewayError> {
    let incidents = list_anomaly_incidents(
        pool,
        &GatewayAnalysisAnomalyIncidentFilters {
            limit: Some(filters.limit.unwrap_or(200).max(200)),
            ..filters.clone()
        },
    )
    .await?;

    let mut by_status = BTreeMap::new();
    let mut by_severity = BTreeMap::new();
    let mut by_code = BTreeMap::new();
    let mut by_follow_up_status = BTreeMap::new();
    let mut by_escalation_status = BTreeMap::new();
    let mut open_incidents = 0;
    let mut acknowledged_incidents = 0;
    let mut resolved_incidents = 0;
    let mut escalated_incidents = 0;

    for incident in &incidents {
        accumulate_key_bucket(&mut by_status, Some(incident.status.as_str()));
        accumulate_key_bucket(&mut by_severity, Some(incident.severity.as_str()));
        accumulate_key_bucket(&mut by_code, Some(incident.code.as_str()));
        accumulate_key_bucket(
            &mut by_follow_up_status,
            Some(incident.follow_up_status.as_str()),
        );
        accumulate_key_bucket(
            &mut by_escalation_status,
            Some(incident.escalation_status.as_str()),
        );
        match incident.status.as_str() {
            "open" => open_incidents += 1,
            "acknowledged" => acknowledged_incidents += 1,
            "resolved" => resolved_incidents += 1,
            _ => {}
        }
        if incident.escalation_status == "escalated" {
            escalated_incidents += 1;
        }
    }

    Ok(GatewayAnalysisAnomalyIncidentSummaryView {
        total_incidents: incidents.len(),
        open_incidents,
        acknowledged_incidents,
        resolved_incidents,
        escalated_incidents,
        by_status: into_key_buckets(by_status),
        by_severity: into_key_buckets(by_severity),
        by_code: into_key_buckets(by_code),
        by_follow_up_status: into_key_buckets(by_follow_up_status),
        by_escalation_status: into_key_buckets(by_escalation_status),
    })
}

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

pub async fn acknowledge_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'acknowledged',
          follow_up_status = 'investigating',
          acknowledged_at = $2,
          last_action_at = $2,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    append_anomaly_incident_history(
        pool,
        incident_id,
        "acknowledged",
        None,
        Some("Operator acknowledged incident."),
        Some(build_incident_snapshot_metadata_from_view(&updated)),
        Some(timestamp),
    )
    .await?;
    Ok(updated)
}

pub async fn resolve_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let existing = get_anomaly_incident_row(pool, incident_id).await?;
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let timestamp = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          follow_up_status = 'done',
          escalation_status = $2,
          resolved_at = $3,
          last_action_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(if previous_escalation_status == "escalated" {
        "resolved"
    } else {
        existing.escalation_status.as_str()
    })
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    append_anomaly_incident_history(
        pool,
        incident_id,
        "resolved",
        None,
        Some(
            updated
                .resolution_note
                .as_deref()
                .unwrap_or("Operator resolved incident."),
        ),
        Some(build_incident_snapshot_metadata_from_view(&updated)),
        Some(timestamp),
    )
    .await?;

    if previous_escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            incident_id,
            "escalation_cleared",
            None,
            Some(
                updated
                    .escalation_reason
                    .as_deref()
                    .unwrap_or("Escalation cleared by operator resolution."),
            ),
            Some(build_incident_snapshot_metadata_from_view(&updated)),
            Some(timestamp),
        )
        .await?;
    }

    Ok(updated)
}

pub async fn update_anomaly_incident_follow_up(
    pool: &PgPool,
    incident_id: &str,
    input: GatewayAnalysisAnomalyIncidentFollowUpInput,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    let incident_id = require_incident_id(incident_id)?;
    let existing = get_anomaly_incident_row(pool, incident_id).await?;
    let timestamp = OffsetDateTime::now_utc();
    let next_owner_user_id = if input.owner_user_id.is_some() {
        trimmed_owned(input.owner_user_id.as_deref())
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status = if let Some(value) = input.follow_up_status.as_deref() {
        normalize_incident_follow_up_status(value).to_string()
    } else {
        normalize_incident_follow_up_status(&existing.follow_up_status).to_string()
    };
    let next_note = if input.note.is_some() {
        trimmed_owned(input.note.as_deref())
    } else {
        existing.latest_note.clone()
    };
    let next_resolution_note = if input.resolution_note.is_some() {
        trimmed_owned(input.resolution_note.as_deref())
    } else {
        existing.resolution_note.clone()
    };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          owner_user_id = $2,
          follow_up_status = $3,
          latest_note = $4,
          resolution_note = $5,
          last_action_at = $6,
          updated_at = $6
        where id = $1
        "#,
    )
    .bind(incident_id)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_note.as_deref())
    .bind(next_resolution_note.as_deref())
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let updated = get_anomaly_incident(pool, incident_id).await?;
    let changed_fields = [
        input.owner_user_id.as_ref().map(|_| "ownerUserId"),
        input.follow_up_status.as_ref().map(|_| "followUpStatus"),
        input.note.as_ref().map(|_| "note"),
        input.resolution_note.as_ref().map(|_| "resolutionNote"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let mut metadata = build_incident_snapshot_metadata_from_view(&updated);
    if let Some(object) = metadata.as_object_mut() {
        object.insert("changedFields".to_string(), json!(changed_fields));
        object.insert(
            "previousOwnerUserId".to_string(),
            json!(existing.owner_user_id.as_deref()),
        );
        object.insert(
            "previousFollowUpStatus".to_string(),
            json!(normalize_incident_follow_up_status(
                &existing.follow_up_status
            )),
        );
        object.insert(
            "previousSyncHitCount".to_string(),
            json!(existing.sync_hit_count),
        );
        object.insert(
            "previousEscalationStatus".to_string(),
            json!(normalize_incident_escalation_status(
                &existing.escalation_status
            )),
        );
        object.insert(
            "previousEscalatedAt".to_string(),
            json!(existing.escalated_at.map(format_timestamp)),
        );
        object.insert(
            "previousEscalationReason".to_string(),
            json!(existing.escalation_reason.as_deref()),
        );
        object.insert(
            "previousLatestNote".to_string(),
            json!(existing.latest_note.as_deref()),
        );
        object.insert(
            "previousResolutionNote".to_string(),
            json!(existing.resolution_note.as_deref()),
        );
    }

    append_anomaly_incident_history(
        pool,
        incident_id,
        "follow_up_updated",
        None,
        Some(
            updated
                .resolution_note
                .as_deref()
                .or(updated.latest_note.as_deref())
                .unwrap_or("Operator updated incident follow-up."),
        ),
        Some(metadata),
        Some(timestamp),
    )
    .await?;
    Ok(updated)
}

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

async fn get_anomaly_incident_row(
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

async fn get_anomaly_incident(
    pool: &PgPool,
    incident_id: &str,
) -> Result<GatewayAnalysisAnomalyIncidentView, GatewayError> {
    Ok(to_anomaly_incident_view(
        get_anomaly_incident_row(pool, incident_id).await?,
    ))
}

async fn fetch_scope_incidents(
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

async fn create_provider_routing_incident(
    pool: &PgPool,
    fingerprint: &str,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let sync_hit_count = 1;
    let escalation = resolve_provider_routing_auto_escalation(&anomaly.severity, sync_hit_count);
    let escalation_status = if escalation.should_escalate {
        "escalated"
    } else {
        "none"
    };
    let follow_up_status = escalation
        .follow_up_status
        .clone()
        .unwrap_or_else(|| "pending".to_string());
    let incident_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incidents (
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        ) values (
          $1, null, $2, $3, $4, $5, null, $6, $7, 'open',
          $8, $9, $10, $11, $12, $13,
          null, null, null, null, null, null,
          0, $14, null, null, $15, $16, $17, $18, $19, $20, $20, null, null, $20, $20
        )
        "#,
    )
    .bind(&incident_id)
    .bind(fingerprint)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(escalation.owner_user_id.as_deref())
    .bind(&follow_up_status)
    .bind(sync_hit_count)
    .bind(escalation_status)
    .bind(escalation.should_escalate.then_some(timestamp))
    .bind(escalation.reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &incident_id,
        "sync_opened",
        None,
        Some(&anomaly.message),
        Some(build_incident_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            anomaly,
            "open",
            escalation.owner_user_id.as_deref(),
            Some(&follow_up_status),
            sync_hit_count,
            Some(escalation_status),
            escalation.should_escalate.then_some(&timestamp),
            escalation.reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            &incident_id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_incident_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                anomaly,
                "open",
                escalation.owner_user_id.as_deref(),
                Some(&follow_up_status),
                sync_hit_count,
                Some(escalation_status),
                Some(&timestamp),
                escalation.reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(incident_id)
}

async fn update_provider_routing_incident(
    pool: &PgPool,
    existing: &GatewayAnalysisAnomalyIncidentRow,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let previous_status = normalize_incident_status(&existing.status);
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let previous_follow_up_status = normalize_incident_follow_up_status(&existing.follow_up_status);
    let was_resolved = previous_status == "resolved";
    let next_status = if previous_status == "acknowledged" {
        "acknowledged"
    } else {
        "open"
    };
    let next_sync_hit_count = if was_resolved {
        1
    } else {
        existing.sync_hit_count.max(0) + 1
    };
    let escalation =
        resolve_provider_routing_auto_escalation(&anomaly.severity, next_sync_hit_count);
    let escalation_transitioned =
        escalation.should_escalate && previous_escalation_status != "escalated";
    let next_escalation_status = if escalation.should_escalate {
        "escalated"
    } else if was_resolved {
        "none"
    } else {
        previous_escalation_status
    };
    let next_escalated_at = if escalation.should_escalate {
        existing.escalated_at.unwrap_or(timestamp)
    } else {
        existing.escalated_at.unwrap_or(timestamp)
    };
    let next_escalation_reason = if escalation.should_escalate {
        escalation.reason.clone()
    } else if was_resolved {
        None
    } else {
        existing.escalation_reason.clone()
    };
    let next_owner_user_id = if escalation_transitioned && existing.owner_user_id.is_none() {
        escalation.owner_user_id.clone()
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status =
        if escalation_transitioned && (was_resolved || previous_follow_up_status == "pending") {
            escalation
                .follow_up_status
                .clone()
                .unwrap_or_else(|| previous_follow_up_status.to_string())
        } else {
            previous_follow_up_status.to_string()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          policy_id = null,
          project_id = $2,
          route_policy_id = $3,
          tag = $4,
          text_mode = null,
          code = $5,
          severity = $6,
          status = $7,
          owner_user_id = $8,
          follow_up_status = $9,
          sync_hit_count = $10,
          escalation_status = $11,
          escalated_at = $12,
          escalation_reason = $13,
          summary = $14,
          latest_export_id = null,
          previous_export_id = null,
          latest_value = $15,
          previous_value = $16,
          delta_value = $17,
          delta_ratio = $18,
          threshold_value = $19,
          last_seen_at = $20,
          resolved_at = null,
          updated_at = $20
        where id = $1
        "#,
    )
    .bind(&existing.id)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(next_status)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_sync_hit_count)
    .bind(next_escalation_status)
    .bind(if escalation.should_escalate {
        Some(next_escalated_at)
    } else if was_resolved {
        None
    } else {
        existing.escalated_at
    })
    .bind(next_escalation_reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &existing.id,
        "sync_updated",
        None,
        Some(&anomaly.message),
        Some(build_incident_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            anomaly,
            next_status,
            next_owner_user_id.as_deref(),
            Some(&next_follow_up_status),
            next_sync_hit_count,
            Some(next_escalation_status),
            if escalation.should_escalate {
                Some(&next_escalated_at)
            } else {
                existing.escalated_at.as_ref()
            },
            next_escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_transitioned {
        append_anomaly_incident_history(
            pool,
            &existing.id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_incident_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                anomaly,
                next_status,
                next_owner_user_id.as_deref(),
                Some(&next_follow_up_status),
                next_sync_hit_count,
                Some(next_escalation_status),
                Some(&next_escalated_at),
                next_escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(existing.id.clone())
}

async fn resolve_provider_routing_incident(
    pool: &PgPool,
    row: &GatewayAnalysisAnomalyIncidentRow,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let next_escalation_status =
        if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
            "resolved"
        } else {
            row.escalation_status.as_str()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          sync_hit_count = 0,
          escalation_status = $2,
          resolved_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_escalation_status)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let anomaly = build_row_anomaly_view(row);
    append_anomaly_incident_history(
        pool,
        &row.id,
        "sync_resolved",
        None,
        Some(&row.summary),
        Some(build_incident_snapshot_metadata(
            row.project_id.as_deref(),
            row.route_policy_id.as_deref(),
            row.tag.as_deref(),
            &anomaly,
            "resolved",
            row.owner_user_id.as_deref(),
            Some(row.follow_up_status.as_str()),
            0,
            Some(next_escalation_status),
            row.escalated_at.as_ref(),
            row.escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
        append_anomaly_incident_history(
            pool,
            &row.id,
            "escalation_cleared",
            None,
            Some(row.escalation_reason.as_deref().unwrap_or(
                "Escalation cleared because provider routing anomaly no longer matched.",
            )),
            Some(build_incident_snapshot_metadata(
                row.project_id.as_deref(),
                row.route_policy_id.as_deref(),
                row.tag.as_deref(),
                &anomaly,
                "resolved",
                row.owner_user_id.as_deref(),
                Some(row.follow_up_status.as_str()),
                0,
                Some("resolved"),
                row.escalated_at.as_ref(),
                row.escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(())
}

async fn create_rate_limit_hotspot_incident(
    pool: &PgPool,
    fingerprint: &str,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    anomaly: &GatewayRateLimitHotspotAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let sync_hit_count = 1;
    let escalation = resolve_rate_limit_hotspot_auto_escalation(&anomaly.severity, sync_hit_count);
    let escalation_status = if escalation.should_escalate {
        "escalated"
    } else {
        "none"
    };
    let follow_up_status = escalation
        .follow_up_status
        .clone()
        .unwrap_or_else(|| "pending".to_string());
    let incident_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incidents (
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        ) values (
          $1, null, $2, $3, $4, $5, null, $6, $7, 'open',
          $8, $9, $10, $11, $12, $13,
          null, null, null, null, null, null,
          0, $14, null, null, $15, $16, $17, $18, $19, $20, $20, null, null, $20, $20
        )
        "#,
    )
    .bind(&incident_id)
    .bind(fingerprint)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(escalation.owner_user_id.as_deref())
    .bind(&follow_up_status)
    .bind(sync_hit_count)
    .bind(escalation_status)
    .bind(escalation.should_escalate.then_some(timestamp))
    .bind(escalation.reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &incident_id,
        "sync_opened",
        None,
        Some(&anomaly.message),
        Some(build_rate_limit_hotspot_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            snapshot,
            anomaly,
            "open",
            escalation.owner_user_id.as_deref(),
            Some(&follow_up_status),
            sync_hit_count,
            Some(escalation_status),
            escalation.should_escalate.then_some(&timestamp),
            escalation.reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            &incident_id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_rate_limit_hotspot_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                snapshot,
                anomaly,
                "open",
                escalation.owner_user_id.as_deref(),
                Some(&follow_up_status),
                sync_hit_count,
                Some(escalation_status),
                Some(&timestamp),
                escalation.reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(incident_id)
}

async fn update_rate_limit_hotspot_incident(
    pool: &PgPool,
    existing: &GatewayAnalysisAnomalyIncidentRow,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: &str,
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    anomaly: &GatewayRateLimitHotspotAnomalyView,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let previous_status = normalize_incident_status(&existing.status);
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let previous_follow_up_status = normalize_incident_follow_up_status(&existing.follow_up_status);
    let was_resolved = previous_status == "resolved";
    let next_status = if previous_status == "acknowledged" {
        "acknowledged"
    } else {
        "open"
    };
    let next_sync_hit_count = if was_resolved {
        1
    } else {
        existing.sync_hit_count.max(0) + 1
    };
    let escalation =
        resolve_rate_limit_hotspot_auto_escalation(&anomaly.severity, next_sync_hit_count);
    let escalation_transitioned =
        escalation.should_escalate && previous_escalation_status != "escalated";
    let next_escalation_status = if escalation.should_escalate {
        "escalated"
    } else if was_resolved {
        "none"
    } else {
        previous_escalation_status
    };
    let next_escalated_at = if escalation.should_escalate {
        existing.escalated_at.unwrap_or(timestamp)
    } else {
        existing.escalated_at.unwrap_or(timestamp)
    };
    let next_escalation_reason = if escalation.should_escalate {
        escalation.reason.clone()
    } else if was_resolved {
        None
    } else {
        existing.escalation_reason.clone()
    };
    let next_owner_user_id = if escalation_transitioned && existing.owner_user_id.is_none() {
        escalation.owner_user_id.clone()
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status =
        if escalation_transitioned && (was_resolved || previous_follow_up_status == "pending") {
            escalation
                .follow_up_status
                .clone()
                .unwrap_or_else(|| previous_follow_up_status.to_string())
        } else {
            previous_follow_up_status.to_string()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          policy_id = null,
          project_id = $2,
          route_policy_id = $3,
          tag = $4,
          text_mode = null,
          code = $5,
          severity = $6,
          status = $7,
          owner_user_id = $8,
          follow_up_status = $9,
          sync_hit_count = $10,
          escalation_status = $11,
          escalated_at = $12,
          escalation_reason = $13,
          summary = $14,
          latest_export_id = null,
          previous_export_id = null,
          latest_value = $15,
          previous_value = $16,
          delta_value = $17,
          delta_ratio = $18,
          threshold_value = $19,
          last_seen_at = $20,
          resolved_at = null,
          updated_at = $20
        where id = $1
        "#,
    )
    .bind(&existing.id)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(next_status)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_sync_hit_count)
    .bind(next_escalation_status)
    .bind(if escalation.should_escalate {
        Some(next_escalated_at)
    } else if was_resolved {
        None
    } else {
        existing.escalated_at
    })
    .bind(next_escalation_reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &existing.id,
        "sync_updated",
        None,
        Some(&anomaly.message),
        Some(build_rate_limit_hotspot_snapshot_metadata(
            project_id,
            route_policy_id,
            Some(tag),
            snapshot,
            anomaly,
            next_status,
            next_owner_user_id.as_deref(),
            Some(&next_follow_up_status),
            next_sync_hit_count,
            Some(next_escalation_status),
            if escalation.should_escalate {
                Some(&next_escalated_at)
            } else {
                existing.escalated_at.as_ref()
            },
            next_escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_transitioned {
        append_anomaly_incident_history(
            pool,
            &existing.id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_rate_limit_hotspot_snapshot_metadata(
                project_id,
                route_policy_id,
                Some(tag),
                snapshot,
                anomaly,
                next_status,
                next_owner_user_id.as_deref(),
                Some(&next_follow_up_status),
                next_sync_hit_count,
                Some(next_escalation_status),
                Some(&next_escalated_at),
                next_escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(existing.id.clone())
}

async fn resolve_rate_limit_hotspot_incident(
    pool: &PgPool,
    row: &GatewayAnalysisAnomalyIncidentRow,
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let next_escalation_status =
        if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
            "resolved"
        } else {
            row.escalation_status.as_str()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          sync_hit_count = 0,
          escalation_status = $2,
          resolved_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_escalation_status)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let anomaly = build_row_rate_limit_hotspot_anomaly_view(row);
    append_anomaly_incident_history(
        pool,
        &row.id,
        "sync_resolved",
        None,
        Some(&row.summary),
        Some(build_rate_limit_hotspot_snapshot_metadata(
            row.project_id.as_deref(),
            row.route_policy_id.as_deref(),
            row.tag.as_deref(),
            snapshot,
            &anomaly,
            "resolved",
            row.owner_user_id.as_deref(),
            Some(&row.follow_up_status),
            0,
            Some(next_escalation_status),
            row.escalated_at.as_ref(),
            row.escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
        append_anomaly_incident_history(
            pool,
            &row.id,
            "escalation_cleared",
            None,
            Some(
                row.escalation_reason
                    .as_deref()
                    .unwrap_or("Escalation cleared because hotspot anomaly no longer matched."),
            ),
            Some(build_rate_limit_hotspot_snapshot_metadata(
                row.project_id.as_deref(),
                row.route_policy_id.as_deref(),
                row.tag.as_deref(),
                snapshot,
                &anomaly,
                "resolved",
                row.owner_user_id.as_deref(),
                Some(&row.follow_up_status),
                0,
                Some("resolved"),
                row.escalated_at.as_ref(),
                row.escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(())
}

async fn create_analysis_export_incident(
    pool: &PgPool,
    fingerprint: &str,
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    auto_escalation: &GatewayAnalysisExportAutoEscalationConfig,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let sync_hit_count = 1;
    let escalation =
        resolve_analysis_export_auto_escalation(auto_escalation, &anomaly.severity, sync_hit_count);
    let escalation_status = if escalation.should_escalate {
        "escalated"
    } else {
        "none"
    };
    let follow_up_status = escalation
        .follow_up_status
        .clone()
        .unwrap_or_else(|| "pending".to_string());
    let incident_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        insert into gateway_analysis_anomaly_incidents (
          id, policy_id, fingerprint, project_id, route_policy_id, tag, text_mode, code, severity, status,
          owner_user_id, follow_up_status, sync_hit_count, escalation_status, escalated_at, escalation_reason,
          latest_note, resolution_note, last_action_at, last_alert_attempt_at, last_alerted_at, last_alert_severity,
          alert_delivery_count, summary, latest_export_id, previous_export_id, latest_value, previous_value, delta_value,
          delta_ratio, threshold_value, first_seen_at, last_seen_at, acknowledged_at, resolved_at, created_at, updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, 'open',
          $10, $11, $12, $13, $14, $15,
          null, null, null, null, null, null,
          0, $16, $17, $18, $19, $20, $21, $22, $23, $24, $24, null, null, $24, $24
        )
        "#,
    )
    .bind(&incident_id)
    .bind(policy_id)
    .bind(fingerprint)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(text_mode)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(escalation.owner_user_id.as_deref())
    .bind(&follow_up_status)
    .bind(sync_hit_count)
    .bind(escalation_status)
    .bind(escalation.should_escalate.then_some(timestamp))
    .bind(escalation.reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_export_id.as_deref())
    .bind(anomaly.previous_export_id.as_deref())
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &incident_id,
        "sync_opened",
        None,
        Some(&anomaly.message),
        Some(build_analysis_export_incident_snapshot_metadata(
            policy_id,
            project_id,
            route_policy_id,
            tag,
            text_mode,
            anomaly,
            "open",
            escalation.owner_user_id.as_deref(),
            Some(&follow_up_status),
            sync_hit_count,
            Some(escalation_status),
            escalation.should_escalate.then_some(&timestamp),
            escalation.reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_status == "escalated" {
        append_anomaly_incident_history(
            pool,
            &incident_id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_analysis_export_incident_snapshot_metadata(
                policy_id,
                project_id,
                route_policy_id,
                tag,
                text_mode,
                anomaly,
                "open",
                escalation.owner_user_id.as_deref(),
                Some(&follow_up_status),
                sync_hit_count,
                Some(escalation_status),
                Some(&timestamp),
                escalation.reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(incident_id)
}

async fn update_analysis_export_incident(
    pool: &PgPool,
    existing: &GatewayAnalysisAnomalyIncidentRow,
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    auto_escalation: &GatewayAnalysisExportAutoEscalationConfig,
    timestamp: OffsetDateTime,
) -> Result<String, GatewayError> {
    let previous_status = normalize_incident_status(&existing.status);
    let previous_escalation_status =
        normalize_incident_escalation_status(&existing.escalation_status);
    let previous_follow_up_status = normalize_incident_follow_up_status(&existing.follow_up_status);
    let was_resolved = previous_status == "resolved";
    let next_status = if previous_status == "acknowledged" {
        "acknowledged"
    } else {
        "open"
    };
    let next_sync_hit_count = if was_resolved {
        1
    } else {
        existing.sync_hit_count.max(0) + 1
    };
    let escalation = resolve_analysis_export_auto_escalation(
        auto_escalation,
        &anomaly.severity,
        next_sync_hit_count,
    );
    let escalation_transitioned =
        escalation.should_escalate && previous_escalation_status != "escalated";
    let next_escalation_status = if escalation.should_escalate {
        "escalated"
    } else if was_resolved {
        "none"
    } else {
        previous_escalation_status
    };
    let next_escalated_at = if escalation.should_escalate {
        existing.escalated_at.unwrap_or(timestamp)
    } else {
        existing.escalated_at.unwrap_or(timestamp)
    };
    let next_escalation_reason = if escalation.should_escalate {
        escalation.reason.clone()
    } else if was_resolved {
        None
    } else {
        existing.escalation_reason.clone()
    };
    let next_owner_user_id = if escalation_transitioned && existing.owner_user_id.is_none() {
        escalation.owner_user_id.clone()
    } else {
        existing.owner_user_id.clone()
    };
    let next_follow_up_status =
        if escalation_transitioned && (was_resolved || previous_follow_up_status == "pending") {
            escalation
                .follow_up_status
                .clone()
                .unwrap_or_else(|| previous_follow_up_status.to_string())
        } else {
            previous_follow_up_status.to_string()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          policy_id = $2,
          project_id = $3,
          route_policy_id = $4,
          tag = $5,
          text_mode = $6,
          code = $7,
          severity = $8,
          status = $9,
          owner_user_id = $10,
          follow_up_status = $11,
          sync_hit_count = $12,
          escalation_status = $13,
          escalated_at = $14,
          escalation_reason = $15,
          summary = $16,
          latest_export_id = $17,
          previous_export_id = $18,
          latest_value = $19,
          previous_value = $20,
          delta_value = $21,
          delta_ratio = $22,
          threshold_value = $23,
          last_seen_at = $24,
          resolved_at = null,
          updated_at = $24
        where id = $1
        "#,
    )
    .bind(&existing.id)
    .bind(policy_id)
    .bind(project_id)
    .bind(route_policy_id)
    .bind(tag)
    .bind(text_mode)
    .bind(&anomaly.code)
    .bind(&anomaly.severity)
    .bind(next_status)
    .bind(next_owner_user_id.as_deref())
    .bind(&next_follow_up_status)
    .bind(next_sync_hit_count)
    .bind(next_escalation_status)
    .bind(if escalation.should_escalate {
        Some(next_escalated_at)
    } else if was_resolved {
        None
    } else {
        existing.escalated_at
    })
    .bind(next_escalation_reason.as_deref())
    .bind(&anomaly.message)
    .bind(anomaly.latest_export_id.as_deref())
    .bind(anomaly.previous_export_id.as_deref())
    .bind(anomaly.latest_value)
    .bind(anomaly.previous_value)
    .bind(anomaly.delta_value)
    .bind(anomaly.delta_ratio)
    .bind(anomaly.threshold_value)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    append_anomaly_incident_history(
        pool,
        &existing.id,
        "sync_updated",
        None,
        Some(&anomaly.message),
        Some(build_analysis_export_incident_snapshot_metadata(
            policy_id,
            project_id,
            route_policy_id,
            tag,
            text_mode,
            anomaly,
            next_status,
            next_owner_user_id.as_deref(),
            Some(&next_follow_up_status),
            next_sync_hit_count,
            Some(next_escalation_status),
            if escalation.should_escalate {
                Some(&next_escalated_at)
            } else {
                existing.escalated_at.as_ref()
            },
            next_escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if escalation_transitioned {
        append_anomaly_incident_history(
            pool,
            &existing.id,
            "auto_escalated",
            None,
            escalation.reason.as_deref(),
            Some(build_analysis_export_incident_snapshot_metadata(
                policy_id,
                project_id,
                route_policy_id,
                tag,
                text_mode,
                anomaly,
                next_status,
                next_owner_user_id.as_deref(),
                Some(&next_follow_up_status),
                next_sync_hit_count,
                Some(next_escalation_status),
                Some(&next_escalated_at),
                next_escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(existing.id.clone())
}

async fn resolve_analysis_export_incident(
    pool: &PgPool,
    row: &GatewayAnalysisAnomalyIncidentRow,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    let next_escalation_status =
        if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
            "resolved"
        } else {
            row.escalation_status.as_str()
        };

    sqlx::query(
        r#"
        update gateway_analysis_anomaly_incidents
        set
          status = 'resolved',
          sync_hit_count = 0,
          escalation_status = $2,
          resolved_at = $3,
          updated_at = $3
        where id = $1
        "#,
    )
    .bind(&row.id)
    .bind(next_escalation_status)
    .bind(timestamp)
    .execute(pool)
    .await
    .map_err(map_db_error)?;

    let anomaly = build_row_analysis_export_anomaly_view(row);
    append_anomaly_incident_history(
        pool,
        &row.id,
        "sync_resolved",
        None,
        Some(&row.summary),
        Some(build_analysis_export_incident_snapshot_metadata(
            row.policy_id.as_deref(),
            row.project_id.as_deref(),
            row.route_policy_id.as_deref(),
            row.tag.as_deref(),
            row.text_mode.as_deref(),
            &anomaly,
            "resolved",
            row.owner_user_id.as_deref(),
            Some(&row.follow_up_status),
            0,
            Some(next_escalation_status),
            row.escalated_at.as_ref(),
            row.escalation_reason.as_deref(),
        )),
        Some(timestamp),
    )
    .await?;

    if normalize_incident_escalation_status(&row.escalation_status) == "escalated" {
        append_anomaly_incident_history(
            pool,
            &row.id,
            "escalation_cleared",
            None,
            Some(row.escalation_reason.as_deref().unwrap_or(
                "Escalation cleared because analysis export anomaly no longer matched.",
            )),
            Some(build_analysis_export_incident_snapshot_metadata(
                row.policy_id.as_deref(),
                row.project_id.as_deref(),
                row.route_policy_id.as_deref(),
                row.tag.as_deref(),
                row.text_mode.as_deref(),
                &anomaly,
                "resolved",
                row.owner_user_id.as_deref(),
                Some(&row.follow_up_status),
                0,
                Some("resolved"),
                row.escalated_at.as_ref(),
                row.escalation_reason.as_deref(),
            )),
            Some(timestamp),
        )
        .await?;
    }

    Ok(())
}

async fn append_anomaly_incident_history(
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

fn build_provider_routing_incident_tag(
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

fn build_adhoc_incident_fingerprint(
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

fn build_incident_snapshot_metadata(
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    anomaly: &GatewayProviderRoutingAnalysisAnomalyView,
    status: &str,
    owner_user_id: Option<&str>,
    follow_up_status: Option<&str>,
    sync_hit_count: i32,
    escalation_status: Option<&str>,
    escalated_at: Option<&OffsetDateTime>,
    escalation_reason: Option<&str>,
) -> Value {
    json!({
        "policyId": Value::Null,
        "projectId": project_id.and_then(trimmed_owned_ref),
        "routePolicyId": route_policy_id.and_then(trimmed_owned_ref),
        "tag": tag.and_then(trimmed_owned_ref),
        "textMode": Value::Null,
        "code": anomaly.code,
        "severity": anomaly.severity,
        "status": status,
        "ownerUserId": owner_user_id.and_then(trimmed_owned_ref),
        "followUpStatus": follow_up_status.and_then(trimmed_owned_ref),
        "syncHitCount": sync_hit_count,
        "escalationStatus": escalation_status.and_then(trimmed_owned_ref),
        "escalatedAt": escalated_at.map(|value| format_timestamp(*value)),
        "escalationReason": escalation_reason.and_then(trimmed_owned_ref),
        "lastAlertAttemptAt": Value::Null,
        "lastAlertedAt": Value::Null,
        "lastAlertSeverity": Value::Null,
        "alertDeliveryCount": Value::Null,
        "latestExportId": Value::Null,
        "previousExportId": Value::Null,
        "latestValue": anomaly.latest_value,
        "previousValue": anomaly.previous_value,
        "deltaValue": anomaly.delta_value,
        "deltaRatio": anomaly.delta_ratio,
        "thresholdValue": anomaly.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}

fn build_rate_limit_hotspot_snapshot_metadata(
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    snapshot: &GatewayRateLimitHotspotAnomalySnapshotView,
    anomaly: &GatewayRateLimitHotspotAnomalyView,
    status: &str,
    owner_user_id: Option<&str>,
    follow_up_status: Option<&str>,
    sync_hit_count: i32,
    escalation_status: Option<&str>,
    escalated_at: Option<&OffsetDateTime>,
    escalation_reason: Option<&str>,
) -> Value {
    json!({
        "policyId": Value::Null,
        "projectId": project_id.and_then(|value| trimmed_owned_ref(value)),
        "routePolicyId": route_policy_id.and_then(|value| trimmed_owned_ref(value)),
        "tag": tag.and_then(|value| trimmed_owned_ref(value)),
        "textMode": Value::Null,
        "code": anomaly.code,
        "severity": anomaly.severity,
        "status": status,
        "ownerUserId": owner_user_id.and_then(trimmed_owned_ref),
        "followUpStatus": follow_up_status.and_then(trimmed_owned_ref),
        "syncHitCount": sync_hit_count,
        "escalationStatus": escalation_status.and_then(trimmed_owned_ref),
        "escalatedAt": escalated_at.map(|value| format_timestamp(*value)),
        "escalationReason": escalation_reason.and_then(trimmed_owned_ref),
        "lastAlertAttemptAt": Value::Null,
        "lastAlertedAt": Value::Null,
        "lastAlertSeverity": Value::Null,
        "alertDeliveryCount": Value::Null,
        "latestExportId": Value::Null,
        "previousExportId": Value::Null,
        "latestValue": anomaly.latest_value,
        "previousValue": anomaly.previous_value,
        "deltaValue": anomaly.delta_value,
        "deltaRatio": anomaly.delta_ratio,
        "thresholdValue": anomaly.threshold_value,
        "snapshotId": snapshot.snapshot_id,
        "entityKey": anomaly.entity_key,
        "latestBucketStartAt": anomaly.latest_bucket_start_at,
        "previousBucketStartAt": anomaly.previous_bucket_start_at
    })
}

fn build_analysis_export_incident_snapshot_metadata(
    policy_id: Option<&str>,
    project_id: Option<&str>,
    route_policy_id: Option<&str>,
    tag: Option<&str>,
    text_mode: Option<&str>,
    anomaly: &GatewayAnalysisExportAnomalyView,
    status: &str,
    owner_user_id: Option<&str>,
    follow_up_status: Option<&str>,
    sync_hit_count: i32,
    escalation_status: Option<&str>,
    escalated_at: Option<&OffsetDateTime>,
    escalation_reason: Option<&str>,
) -> Value {
    json!({
        "policyId": policy_id.and_then(trimmed_owned_ref),
        "projectId": project_id.and_then(trimmed_owned_ref),
        "routePolicyId": route_policy_id.and_then(trimmed_owned_ref),
        "tag": tag.and_then(trimmed_owned_ref),
        "textMode": text_mode.and_then(trimmed_owned_ref),
        "code": anomaly.code,
        "severity": anomaly.severity,
        "status": status,
        "ownerUserId": owner_user_id.and_then(trimmed_owned_ref),
        "followUpStatus": follow_up_status.and_then(trimmed_owned_ref),
        "syncHitCount": sync_hit_count,
        "escalationStatus": escalation_status.and_then(trimmed_owned_ref),
        "escalatedAt": escalated_at.map(|value| format_timestamp(*value)),
        "escalationReason": escalation_reason.and_then(trimmed_owned_ref),
        "lastAlertAttemptAt": Value::Null,
        "lastAlertedAt": Value::Null,
        "lastAlertSeverity": Value::Null,
        "alertDeliveryCount": Value::Null,
        "latestExportId": anomaly.latest_export_id.clone(),
        "previousExportId": anomaly.previous_export_id.clone(),
        "latestValue": anomaly.latest_value,
        "previousValue": anomaly.previous_value,
        "deltaValue": anomaly.delta_value,
        "deltaRatio": anomaly.delta_ratio,
        "thresholdValue": anomaly.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}

fn build_row_anomaly_view(
    row: &GatewayAnalysisAnomalyIncidentRow,
) -> GatewayProviderRoutingAnalysisAnomalyView {
    GatewayProviderRoutingAnalysisAnomalyView {
        code: row.code.clone(),
        severity: row.severity.clone(),
        message: row.summary.clone(),
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value,
    }
}

fn build_row_rate_limit_hotspot_anomaly_view(
    row: &GatewayAnalysisAnomalyIncidentRow,
) -> GatewayRateLimitHotspotAnomalyView {
    GatewayRateLimitHotspotAnomalyView {
        code: row.code.clone(),
        severity: row.severity.clone(),
        message: row.summary.clone(),
        entity_key: None,
        latest_bucket_start_at: None,
        previous_bucket_start_at: None,
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value.unwrap_or_default(),
    }
}

fn build_row_analysis_export_anomaly_view(
    row: &GatewayAnalysisAnomalyIncidentRow,
) -> GatewayAnalysisExportAnomalyView {
    GatewayAnalysisExportAnomalyView {
        code: row.code.clone(),
        severity: row.severity.clone(),
        message: row.summary.clone(),
        latest_export_id: row.latest_export_id.clone(),
        previous_export_id: row.previous_export_id.clone(),
        latest_value: row.latest_value,
        previous_value: row.previous_value,
        delta_value: row.delta_value,
        delta_ratio: row.delta_ratio,
        threshold_value: row.threshold_value,
    }
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

fn build_incident_snapshot_metadata_from_view(
    incident: &GatewayAnalysisAnomalyIncidentView,
) -> Value {
    json!({
        "policyId": incident.policy_id,
        "projectId": incident.project_id,
        "routePolicyId": incident.route_policy_id,
        "tag": incident.tag,
        "textMode": incident.text_mode,
        "code": incident.code,
        "severity": incident.severity,
        "status": incident.status,
        "ownerUserId": incident.owner_user_id,
        "followUpStatus": incident.follow_up_status,
        "syncHitCount": incident.sync_hit_count,
        "escalationStatus": incident.escalation_status,
        "escalatedAt": incident.escalated_at,
        "escalationReason": incident.escalation_reason,
        "lastAlertAttemptAt": incident.last_alert_attempt_at,
        "lastAlertedAt": incident.last_alerted_at,
        "lastAlertSeverity": incident.last_alert_severity,
        "alertDeliveryCount": incident.alert_delivery_count,
        "latestExportId": incident.latest_export_id,
        "previousExportId": incident.previous_export_id,
        "latestValue": incident.latest_value,
        "previousValue": incident.previous_value,
        "deltaValue": incident.delta_value,
        "deltaRatio": incident.delta_ratio,
        "thresholdValue": incident.threshold_value,
        "snapshotId": Value::Null,
        "entityKey": Value::Null,
        "latestBucketStartAt": Value::Null,
        "previousBucketStartAt": Value::Null
    })
}

fn normalize_incident_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "acknowledged" => "acknowledged",
        "resolved" => "resolved",
        _ => "open",
    }
}

fn normalize_incident_follow_up_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "investigating" => "investigating",
        "monitoring" => "monitoring",
        "done" => "done",
        _ => "pending",
    }
}

fn normalize_incident_escalation_status(value: &str) -> &str {
    match value.trim().to_lowercase().as_str() {
        "escalated" => "escalated",
        "resolved" => "resolved",
        _ => "none",
    }
}

fn normalize_alert_delivery_severity(value: Option<&str>) -> &str {
    match value.and_then(trimmed_owned_ref) {
        Some(value) if value.eq_ignore_ascii_case("info") => "info",
        Some(value) if value.eq_ignore_ascii_case("danger") => "danger",
        _ => "warning",
    }
}

fn accumulate_key_bucket(map: &mut BTreeMap<String, usize>, value: Option<&str>) {
    let Some(key) = value.and_then(trimmed_owned_ref) else {
        return;
    };
    *map.entry(key.to_string()).or_insert(0) += 1;
}

fn into_key_buckets(map: BTreeMap<String, usize>) -> Vec<GatewaySummaryBucketKeyView> {
    let mut buckets = map
        .into_iter()
        .map(|(key, count)| GatewaySummaryBucketKeyView { key, count })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    buckets
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

fn trimmed_owned(value: Option<&str>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn trimmed_owned_ref(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn require_incident_id(incident_id: &str) -> Result<&str, GatewayError> {
    trimmed_owned_ref(incident_id).ok_or_else(|| GatewayError::bad_request("incidentId 不能为空"))
}

#[derive(Debug, Clone)]
struct ProviderRoutingEscalationDecision {
    should_escalate: bool,
    reason: Option<String>,
    owner_user_id: Option<String>,
    follow_up_status: Option<String>,
}

fn resolve_provider_routing_auto_escalation(
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    let normalized_severity = severity.trim().to_lowercase();
    let required_hit_count = if normalized_severity == "critical" {
        1
    } else {
        3
    };
    if sync_hit_count < required_hit_count {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }
    let reason = if normalized_severity == "critical" {
        format!(
            "Provider routing auto escalated immediately at severity critical after {sync_hit_count} sync hit(s)."
        )
    } else {
        format!("Provider routing auto escalated after {sync_hit_count} warning sync hit(s).")
    };
    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(reason),
        owner_user_id: None,
        follow_up_status: Some(
            if normalized_severity == "critical" {
                "investigating"
            } else {
                "monitoring"
            }
            .to_string(),
        ),
    }
}

fn resolve_rate_limit_hotspot_auto_escalation(
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    let normalized_severity = severity.trim().to_lowercase();
    let required_hit_count = if normalized_severity == "critical" {
        1
    } else {
        3
    };
    if sync_hit_count < required_hit_count {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }
    let reason = if normalized_severity == "critical" {
        format!(
            "Hotspot auto escalated immediately at severity critical after {sync_hit_count} sync hit(s)."
        )
    } else {
        format!("Hotspot auto escalated after {sync_hit_count} warning sync hit(s).")
    };
    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(reason),
        owner_user_id: None,
        follow_up_status: Some(
            if normalized_severity == "critical" {
                "investigating"
            } else {
                "monitoring"
            }
            .to_string(),
        ),
    }
}

fn severity_rank(value: &str) -> i32 {
    match value.trim().to_lowercase().as_str() {
        "critical" => 2,
        "warning" => 1,
        _ => 0,
    }
}

fn resolve_analysis_export_auto_escalation(
    config: &GatewayAnalysisExportAutoEscalationConfig,
    severity: &str,
    sync_hit_count: i32,
) -> ProviderRoutingEscalationDecision {
    if !config.enabled {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }

    let normalized_severity = severity.trim().to_lowercase();
    let required_severity = config
        .severity_threshold
        .as_deref()
        .and_then(trimmed_owned_ref)
        .unwrap_or("critical");
    let required_hit_count = config.after_sync_count.unwrap_or(3).max(1);
    if severity_rank(&normalized_severity) < severity_rank(required_severity)
        || sync_hit_count < required_hit_count
    {
        return ProviderRoutingEscalationDecision {
            should_escalate: false,
            reason: None,
            owner_user_id: None,
            follow_up_status: None,
        };
    }

    ProviderRoutingEscalationDecision {
        should_escalate: true,
        reason: Some(format!(
            "Auto escalated after {sync_hit_count} sync hit(s) at severity {normalized_severity}."
        )),
        owner_user_id: trimmed_owned(config.owner_user_id.as_deref()),
        follow_up_status: trimmed_owned(config.follow_up_status.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_adhoc_incident_fingerprint, build_provider_routing_incident_tag,
        resolve_analysis_export_auto_escalation, resolve_provider_routing_auto_escalation,
        GatewayAnalysisExportAutoEscalationConfig, GatewayProviderRoutingAnalysisAnomalyReportView,
    };
    use crate::db::request_audits::{
        GatewayAnalysisMetricDistributionView,
        GatewayProviderRoutingAnalysisAnomalyThresholdConfig,
        GatewayProviderRoutingAnalysisFilterView, GatewayProviderRoutingAnalysisSummaryView,
    };

    #[test]
    fn provider_routing_incident_tag_matches_ts_shape() {
        let tag =
            build_provider_routing_incident_tag(&GatewayProviderRoutingAnalysisAnomalyReportView {
                generated_at: "2026-04-13T00:00:00Z".to_string(),
                filters: GatewayProviderRoutingAnalysisFilterView {
                    project_id: Some("proj-1".to_string()),
                    route_policy_id: Some("rp-1".to_string()),
                    provider_account_id: Some("provider-1".to_string()),
                    session_id: Some("session-1".to_string()),
                    api_key_id: Some("key-1".to_string()),
                    response_id: Some("resp-1".to_string()),
                    protocol_family: Some("OpenAI".to_string()),
                    endpoint_kind: Some("Chat".to_string()),
                    status: Some("FAILED".to_string()),
                    created_from: None,
                    created_to: None,
                    limit: 50,
                },
                profile_key: "balanced".to_string(),
                thresholds: GatewayProviderRoutingAnalysisAnomalyThresholdConfig {
                    routing_score_warning_threshold: 0.65,
                    routing_score_critical_threshold: 0.4,
                    degraded_route_warning_threshold: 0.15,
                    degraded_route_critical_threshold: 0.35,
                    saturated_route_warning_threshold: 0.1,
                    saturated_route_critical_threshold: 0.25,
                    breaker_open_route_warning_threshold: 0.05,
                    breaker_open_route_critical_threshold: 0.15,
                },
                summary: GatewayProviderRoutingAnalysisSummaryView {
                    total_samples: 0,
                    selected_provider_samples: 0,
                    degraded_selected_provider_samples: 0,
                    saturated_selected_provider_samples: 0,
                    breaker_open_selected_provider_samples: 0,
                    routing_score: GatewayAnalysisMetricDistributionView {
                        avg: None,
                        p50: None,
                        p95: None,
                    },
                    health_weight: GatewayAnalysisMetricDistributionView {
                        avg: None,
                        p50: None,
                        p95: None,
                    },
                    capacity_weight: GatewayAnalysisMetricDistributionView {
                        avg: None,
                        p50: None,
                        p95: None,
                    },
                    by_selected_provider: Vec::new(),
                    by_degradation_reason: Vec::new(),
                },
                anomalies: Vec::new(),
                by_severity: Vec::new(),
                by_code: Vec::new(),
            });

        assert_eq!(
            tag,
            "provider-routing:balanced:provider:provider-1:protocol:openai:endpoint:chat:api-key:key-1:session:session-1:response:resp-1:status:failed"
        );
    }

    #[test]
    fn provider_routing_adhoc_fingerprint_matches_ts_shape() {
        let fingerprint = build_adhoc_incident_fingerprint(
            None,
            Some("project-1"),
            Some("route-1"),
            Some("provider-routing:balanced"),
            None,
            "routing_score_low",
        );
        assert_eq!(
            fingerprint,
            "adhoc:project:project-1:routePolicy:route-1:tag:provider-routing:balanced:textMode:*:code:routing_score_low"
        );
    }

    #[test]
    fn analysis_export_policy_fingerprint_matches_ts_shape() {
        let fingerprint = build_adhoc_incident_fingerprint(
            Some("policy-1"),
            Some("project-1"),
            Some("route-1"),
            Some("custom-export-tag"),
            Some("chat"),
            "failure_rate_spike",
        );
        assert_eq!(fingerprint, "policy:policy-1:code:failure_rate_spike");
    }

    #[test]
    fn analysis_export_adhoc_fingerprint_keeps_text_mode() {
        let fingerprint = build_adhoc_incident_fingerprint(
            None,
            Some("project-1"),
            None,
            Some("custom-export-tag"),
            Some("chat"),
            "failure_rate_spike",
        );
        assert_eq!(
            fingerprint,
            "adhoc:project:project-1:routePolicy:*:tag:custom-export-tag:textMode:chat:code:failure_rate_spike"
        );
    }

    #[test]
    fn provider_routing_auto_escalation_matches_ts_rules() {
        let warning_before_threshold = resolve_provider_routing_auto_escalation("warning", 2);
        assert!(!warning_before_threshold.should_escalate);

        let warning_after_threshold = resolve_provider_routing_auto_escalation("warning", 3);
        assert!(warning_after_threshold.should_escalate);
        assert_eq!(
            warning_after_threshold.reason.as_deref(),
            Some("Provider routing auto escalated after 3 warning sync hit(s).")
        );
        assert_eq!(
            warning_after_threshold.follow_up_status.as_deref(),
            Some("monitoring")
        );

        let critical = resolve_provider_routing_auto_escalation("critical", 1);
        assert!(critical.should_escalate);
        assert_eq!(
            critical.reason.as_deref(),
            Some("Provider routing auto escalated immediately at severity critical after 1 sync hit(s).")
        );
        assert_eq!(critical.follow_up_status.as_deref(), Some("investigating"));
    }

    #[test]
    fn analysis_export_auto_escalation_matches_ts_rules() {
        let disabled = resolve_analysis_export_auto_escalation(
            &GatewayAnalysisExportAutoEscalationConfig::default(),
            "critical",
            4,
        );
        assert!(!disabled.should_escalate);

        let warning_before_threshold = resolve_analysis_export_auto_escalation(
            &GatewayAnalysisExportAutoEscalationConfig {
                enabled: true,
                severity_threshold: Some("warning".to_string()),
                after_sync_count: Some(3),
                owner_user_id: Some("owner-1".to_string()),
                follow_up_status: Some("monitoring".to_string()),
            },
            "warning",
            2,
        );
        assert!(!warning_before_threshold.should_escalate);

        let warning = resolve_analysis_export_auto_escalation(
            &GatewayAnalysisExportAutoEscalationConfig {
                enabled: true,
                severity_threshold: Some("warning".to_string()),
                after_sync_count: Some(3),
                owner_user_id: Some("owner-1".to_string()),
                follow_up_status: Some("monitoring".to_string()),
            },
            "warning",
            3,
        );
        assert!(warning.should_escalate);
        assert_eq!(
            warning.reason.as_deref(),
            Some("Auto escalated after 3 sync hit(s) at severity warning.")
        );
        assert_eq!(warning.owner_user_id.as_deref(), Some("owner-1"));
        assert_eq!(warning.follow_up_status.as_deref(), Some("monitoring"));

        let critical = resolve_analysis_export_auto_escalation(
            &GatewayAnalysisExportAutoEscalationConfig {
                enabled: true,
                severity_threshold: None,
                after_sync_count: None,
                owner_user_id: None,
                follow_up_status: Some("investigating".to_string()),
            },
            "critical",
            3,
        );
        assert!(critical.should_escalate);
        assert_eq!(
            critical.reason.as_deref(),
            Some("Auto escalated after 3 sync hit(s) at severity critical.")
        );
        assert_eq!(critical.follow_up_status.as_deref(), Some("investigating"));
    }
}
