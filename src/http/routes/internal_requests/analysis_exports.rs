//! Management analysis export creation, storage and metadata.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
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
pub struct AnalysisExportQuery {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub text_mode: Option<String>,
    pub max_text_chars: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedAnalysisExportQuery {
    pub export_id: Option<String>,
    pub label: Option<String>,
    pub tag: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub text_mode: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistAnalysisExportInput {
    pub label: Option<String>,
    pub tags: Option<Vec<String>>,
    pub retention_expires_at: Option<String>,
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub response_id: Option<String>,
    pub protocol_family: Option<String>,
    pub status: Option<String>,
    pub endpoint_kind: Option<String>,
    pub stream: Option<bool>,
    pub error_code: Option<String>,
    pub fallback_eligible: Option<bool>,
    pub artifact_available: Option<bool>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
    pub text_mode: Option<String>,
    pub max_text_chars: Option<usize>,
}

pub async fn export_analysis_rows(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<AnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::export_analysis_rows(
        required_pg_pool(state.as_ref())?,
        &into_analysis_export_filters(&query),
        query.text_mode.as_deref(),
        query.max_text_chars,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn persist_analysis_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<PersistAnalysisExportInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::persist_analysis_export(
        required_pg_pool(state.as_ref())?,
        &into_analysis_export_filters_from_body(&body),
        body.text_mode.as_deref(),
        body.max_text_chars,
        db::PersistGatewayAnalysisExportInput {
            label: body.label,
            tags: body.tags,
            retention_expires_at: body.retention_expires_at,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn list_persisted_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let exports = db::list_persisted_analysis_exports(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "exports": exports,
    })))
}

pub async fn summarize_persisted_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PersistedAnalysisExportQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_persisted_analysis_exports(
        required_pg_pool(state.as_ref())?,
        &into_persisted_analysis_export_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_persisted_analysis_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(export_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export =
        db::get_persisted_analysis_export(required_pg_pool(state.as_ref())?, export_id.trim())
            .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn update_persisted_analysis_export_metadata(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(export_id): Path<String>,
    Json(body): Json<db::GatewayAnalysisExportMetadataUpdateInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let export = db::update_persisted_analysis_export_metadata(
        required_pg_pool(state.as_ref())?,
        export_id.trim(),
        body,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "export": export,
    })))
}

pub async fn cleanup_expired_analysis_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<db::GatewayAnalysisExportCleanupInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let cleanup =
        db::cleanup_expired_analysis_exports(required_pg_pool(state.as_ref())?, body).await?;
    Ok(Json(serde_json::json!({
        "cleanup": cleanup,
    })))
}

fn into_analysis_export_filters(query: &AnalysisExportQuery) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: query.project_id.clone(),
        route_policy_id: query.route_policy_id.clone(),
        provider_account_id: query.provider_account_id.clone(),
        session_id: query.session_id.clone(),
        api_key_id: query.api_key_id.clone(),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: query.response_id.clone(),
        protocol_family: query.protocol_family.clone(),
        status: query.status.clone(),
        endpoint_kind: query.endpoint_kind.clone(),
        stream: query.stream,
        error_code: query.error_code.clone(),
        fallback_eligible: query.fallback_eligible,
        artifact_available: query.artifact_available,
        created_from: query.created_from.clone(),
        created_to: query.created_to.clone(),
        limit: query.limit,
    }
}

fn into_analysis_export_filters_from_body(
    body: &PersistAnalysisExportInput,
) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: body.project_id.clone(),
        route_policy_id: body.route_policy_id.clone(),
        provider_account_id: body.provider_account_id.clone(),
        session_id: body.session_id.clone(),
        api_key_id: body.api_key_id.clone(),
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        response_id: body.response_id.clone(),
        protocol_family: body.protocol_family.clone(),
        status: body.status.clone(),
        endpoint_kind: body.endpoint_kind.clone(),
        stream: body.stream,
        error_code: body.error_code.clone(),
        fallback_eligible: body.fallback_eligible,
        artifact_available: body.artifact_available,
        created_from: body.created_from.clone(),
        created_to: body.created_to.clone(),
        limit: body.limit,
    }
}

pub(super) fn into_persisted_analysis_export_filters(
    query: PersistedAnalysisExportQuery,
) -> db::GatewayPersistedAnalysisExportFilters {
    db::GatewayPersistedAnalysisExportFilters {
        export_id: query.export_id,
        label: query.label,
        tag: query.tag,
        project_id: query.project_id,
        status: query.status,
        text_mode: query.text_mode,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}
