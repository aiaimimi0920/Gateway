//! Management anomaly incident lifecycle, attribution and alert dispatch.

use super::super::internal_gateway::assert_management_access;
use super::{management_actor_user_id, required_pg_pool};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyIncidentQuery {
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertDispatchInput {
    pub alerted_at: Option<String>,
    pub alert_severity: Option<String>,
    pub alert_level: Option<i32>,
    pub note: Option<String>,
    pub mailbox_recipient_count: Option<i32>,
    pub webhook_dispatched: Option<bool>,
    pub webhook_skipped_reason: Option<String>,
    pub remediation_action_keys: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnomalyIncidentSyncInput {
    pub policy_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub profile_key: Option<String>,
}

pub async fn sync_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<AnomalyIncidentSyncInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let sync = db::sync_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        db::GatewayAnalysisAnomalyIncidentSyncInput {
            policy_id: body.policy_id,
            label: body.label,
            tag: body.tag,
            project_id: body.project_id,
            route_policy_id: body.route_policy_id,
            provider_account_id: body.provider_account_id,
            session_id: body.session_id,
            api_key_id: body.api_key_id,
            user_credential_id: body.user_credential_id,
            response_id: body.response_id,
            protocol_family: body.protocol_family,
            status: body.status,
            endpoint_kind: body.endpoint_kind,
            stream: body.stream,
            error_code: body.error_code,
            fallback_eligible: body.fallback_eligible,
            artifact_available: body.artifact_available,
            text_mode: body.text_mode,
            created_from: body.created_from,
            created_to: body.created_to,
            limit: body.limit,
            profile_key: body.profile_key,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "sync": sync,
    })))
}

pub async fn list_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incidents = db::list_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incidents": incidents,
    })))
}

pub async fn summarize_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn list_anomaly_incident_alert_queue(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let queue = db::list_anomaly_incident_alert_queue(
        required_pg_pool(state.as_ref())?,
        &into_incident_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "queue": queue,
    })))
}

pub async fn list_anomaly_incident_history(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Query(query): Query<AnomalyIncidentQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let history = db::list_anomaly_incident_history(
        required_pg_pool(state.as_ref())?,
        incident_id.trim(),
        query.limit,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "history": history,
    })))
}

pub async fn acknowledge_anomaly_incident(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident =
        db::acknowledge_anomaly_incident(required_pg_pool(state.as_ref())?, incident_id.trim())
            .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn resolve_anomaly_incident(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident =
        db::resolve_anomaly_incident(required_pg_pool(state.as_ref())?, incident_id.trim()).await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn update_anomaly_incident_follow_up(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Json(body): Json<db::GatewayAnalysisAnomalyIncidentFollowUpInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident = db::update_anomaly_incident_follow_up(
        required_pg_pool(state.as_ref())?,
        incident_id.trim(),
        body,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub async fn record_anomaly_incident_alert_dispatch(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(incident_id): Path<String>,
    Json(body): Json<AlertDispatchInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let incident = db::record_anomaly_incident_alert_dispatch(
        required_pg_pool(state.as_ref())?,
        management_actor_user_id(&headers),
        incident_id.trim(),
        db::RecordGatewayAnalysisAnomalyIncidentAlertDispatchInput {
            alerted_at: body.alerted_at,
            alert_severity: body.alert_severity,
            alert_level: body.alert_level,
            note: body.note,
            mailbox_recipient_count: body.mailbox_recipient_count,
            webhook_dispatched: body.webhook_dispatched,
            webhook_skipped_reason: body.webhook_skipped_reason,
            remediation_action_keys: body.remediation_action_keys,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "incident": incident,
    })))
}

pub(super) fn into_incident_filters(
    query: AnomalyIncidentQuery,
) -> db::GatewayAnalysisAnomalyIncidentFilters {
    db::GatewayAnalysisAnomalyIncidentFilters {
        incident_id: query.incident_id,
        policy_id: query.policy_id,
        project_id: query.project_id,
        route_policy_id: query.route_policy_id,
        owner_user_id: query.owner_user_id,
        tag: query.tag,
        text_mode: query.text_mode,
        status: query.status,
        follow_up_status: query.follow_up_status,
        escalation_status: query.escalation_status,
        code: query.code,
        severity: query.severity,
        due_only: query.due_only,
        limit: query.limit,
    }
}
