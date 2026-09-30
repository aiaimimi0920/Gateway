//! Management audit queries and object-storage artifact reads.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::object_storage::gateway_object_storage;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAuditQuery {
    pub project_id: Option<String>,
    pub route_policy_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub session_id: Option<String>,
    pub api_key_id: Option<String>,
    pub user_credential_id: Option<String>,
    pub access_key_id: Option<String>,
    pub source_access_key_id: Option<String>,
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
    pub profile_key: Option<String>,
    pub routing_score_warning_threshold: Option<f64>,
    pub routing_score_critical_threshold: Option<f64>,
    pub degraded_route_warning_threshold: Option<f64>,
    pub degraded_route_critical_threshold: Option<f64>,
    pub saturated_route_warning_threshold: Option<f64>,
    pub saturated_route_critical_threshold: Option<f64>,
    pub breaker_open_route_warning_threshold: Option<f64>,
    pub breaker_open_route_critical_threshold: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCacheQuery {
    #[serde(flatten)]
    pub filters: RequestAuditQuery,
    pub input_price_per_million: Option<f64>,
    pub bucket_size: Option<String>,
}

pub async fn list_request_audits(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(query);
    let requests = if let Some(local) = &state.local_runtime {
        local.list_audits(&filters).await?
    } else {
        db::list_request_audits(required_pg_pool(state.as_ref())?, &filters).await?
    };
    Ok(Json(serde_json::json!({
        "requests": requests,
    })))
}

pub async fn summarize_request_audits(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(query);
    let summary = if let Some(local) = &state.local_runtime {
        db::request_audits::summarize_request_audit_rows(&local.list_audits(&filters).await?)
    } else {
        db::summarize_request_audits(required_pg_pool(state.as_ref())?, &filters).await?
    };
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn list_analysis_samples(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let samples =
        db::list_analysis_samples(required_pg_pool(state.as_ref())?, &into_filters(query)).await?;
    Ok(Json(serde_json::json!({
        "samples": samples,
    })))
}

pub async fn summarize_analysis(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary =
        db::summarize_analysis(required_pg_pool(state.as_ref())?, &into_filters(query)).await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn summarize_prompt_cache(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_prompt_cache(
        required_pg_pool(state.as_ref())?,
        &into_filters(query.filters),
        query.input_price_per_million,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_prompt_cache_trend_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<PromptCacheQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let report = db::get_prompt_cache_trend_report(
        required_pg_pool(state.as_ref())?,
        &into_filters(query.filters),
        query.input_price_per_million,
        query.bucket_size.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn get_request_audit_by_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(request_audit_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        Some(request_audit_id.trim()),
        None,
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "requestAudit": request_audit,
    })))
}

pub async fn get_request_audit_by_response_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        None,
        Some(response_id.trim()),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "requestAudit": request_audit,
    })))
}

pub async fn get_request_artifacts_by_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(request_audit_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        Some(request_audit_id.trim()),
        None,
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "artifacts": build_request_artifacts_view(&request_audit).await,
    })))
}

pub async fn get_request_artifacts_by_response_id(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(response_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let request_audit = db::get_request_audit(
        required_pg_pool(state.as_ref())?,
        None,
        Some(response_id.trim()),
    )
    .await?
    .ok_or_else(|| GatewayError::not_found("Gateway request audit 不存在"))?;
    Ok(Json(serde_json::json!({
        "artifacts": build_request_artifacts_view(&request_audit).await,
    })))
}

pub(super) fn into_filters(query: RequestAuditQuery) -> db::RequestAuditFilters {
    db::RequestAuditFilters {
        project_id: query.project_id,
        route_policy_id: query.route_policy_id,
        provider_account_id: query.provider_account_id,
        session_id: query.session_id,
        api_key_id: query.api_key_id,
        user_credential_id: query.user_credential_id,
        access_key_id: query.access_key_id,
        source_access_key_id: query.source_access_key_id,
        response_id: query.response_id,
        protocol_family: query.protocol_family,
        status: query.status,
        endpoint_kind: query.endpoint_kind,
        stream: query.stream,
        error_code: query.error_code,
        fallback_eligible: query.fallback_eligible,
        artifact_available: query.artifact_available,
        created_from: query.created_from,
        created_to: query.created_to,
        limit: query.limit,
    }
}

async fn build_request_artifacts_view(request_audit: &db::GatewayRequestAuditView) -> Value {
    let request_artifact = match request_audit.request_artifact_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };
    let response_artifact = match request_audit.response_artifact_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };

    serde_json::json!({
        "requestAudit": request_audit,
        "requestArtifact": request_artifact,
        "responseArtifact": response_artifact,
    })
}

async fn read_optional_artifact(object_key: &str) -> Option<Value> {
    let storage = gateway_object_storage().ok()?;
    storage.read_json(object_key).await.ok()
}
