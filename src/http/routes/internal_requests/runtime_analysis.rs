//! Management runtime pressure and provider-routing analysis.

use super::super::internal_gateway::assert_management_access;
use super::audits::{into_filters, RequestAuditQuery};
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePressureQuery {
    pub project_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub limit: Option<usize>,
}

pub async fn get_model_association_matrix(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let matrix = db::get_model_association_matrix(required_pg_pool(state.as_ref())?).await?;
    Ok(Json(serde_json::json!({
        "matrix": matrix,
    })))
}

pub async fn get_cost_overview(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let runtime_providers = route_document_provider_identities(state.as_ref());
    let overview =
        db::get_cost_overview(required_pg_pool(state.as_ref())?, &runtime_providers).await?;
    Ok(Json(serde_json::json!({
        "overview": overview,
    })))
}

/// Describe the route document's providers for the runtime pressure and cost views.
///
/// Request audits record whichever provider served the request, and in a
/// standalone deployment that provider lives in the route document rather than
/// in `gateway_provider_accounts`. Passing these descriptors keeps the
/// per-provider concurrency and usage panels populated instead of dropping every
/// route-document provider from the response.
fn route_document_provider_identities(state: &AppState) -> Vec<db::GatewayRuntimeProviderIdentity> {
    state
        .route_config
        .get_providers()
        .into_iter()
        .map(|provider| {
            // A provider with an empty credential pool carries its credential in
            // the payload itself, so it is routable; one with a pool is routable
            // only while at least one credential in it stays enabled.
            let routable = provider.credential_pool.is_empty()
                || provider
                    .credential_pool
                    .iter()
                    .any(|credential| credential.enabled);
            db::GatewayRuntimeProviderIdentity {
                id: provider.id,
                label: provider.label,
                status: if routable { "active" } else { "disabled" }.to_string(),
                adapter: provider.payload.adapter.clone(),
                protocol_family: provider.protocol_family,
                supported_models: provider.supported_models,
            }
        })
        .collect()
}

pub async fn get_runtime_pressure(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RuntimePressureQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let concurrency_snapshots = state.concurrency_registry.snapshot_all();
    let runtime_providers = route_document_provider_identities(state.as_ref());
    let pressure = db::get_runtime_pressure(
        required_pg_pool(state.as_ref())?,
        &state.redis_pool,
        &concurrency_snapshots,
        &runtime_providers,
        &db::GatewayRuntimePressureFilters {
            project_id: query.project_id,
            provider_account_id: query.provider_account_id,
            limit: query.limit,
        },
    )
    .await?;
    Ok(Json(serde_json::json!({
        "pressure": pressure,
    })))
}

pub async fn summarize_provider_routing_analysis(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let summary = db::summarize_provider_routing_analysis(
        required_pg_pool(state.as_ref())?,
        &into_filters(query),
    )
    .await?;
    Ok(Json(serde_json::json!({
        "summary": summary,
    })))
}

pub async fn get_provider_routing_anomaly_report(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(query.clone());
    let overrides = db::GatewayProviderRoutingAnalysisAnomalyOverrides {
        routing_score_warning_threshold: query.routing_score_warning_threshold,
        routing_score_critical_threshold: query.routing_score_critical_threshold,
        degraded_route_warning_threshold: query.degraded_route_warning_threshold,
        degraded_route_critical_threshold: query.degraded_route_critical_threshold,
        saturated_route_warning_threshold: query.saturated_route_warning_threshold,
        saturated_route_critical_threshold: query.saturated_route_critical_threshold,
        breaker_open_route_warning_threshold: query.breaker_open_route_warning_threshold,
        breaker_open_route_critical_threshold: query.breaker_open_route_critical_threshold,
    };
    let report = db::get_provider_routing_anomaly_report(
        required_pg_pool(state.as_ref())?,
        &filters,
        query.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "report": report,
    })))
}

pub async fn sync_provider_routing_anomaly_incidents(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<RequestAuditQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let filters = into_filters(body.clone());
    let overrides = db::GatewayProviderRoutingAnalysisAnomalyOverrides {
        routing_score_warning_threshold: body.routing_score_warning_threshold,
        routing_score_critical_threshold: body.routing_score_critical_threshold,
        degraded_route_warning_threshold: body.degraded_route_warning_threshold,
        degraded_route_critical_threshold: body.degraded_route_critical_threshold,
        saturated_route_warning_threshold: body.saturated_route_warning_threshold,
        saturated_route_critical_threshold: body.saturated_route_critical_threshold,
        breaker_open_route_warning_threshold: body.breaker_open_route_warning_threshold,
        breaker_open_route_critical_threshold: body.breaker_open_route_critical_threshold,
    };
    let result = db::sync_provider_routing_anomaly_incidents(
        required_pg_pool(state.as_ref())?,
        &filters,
        body.profile_key.as_deref(),
        overrides,
    )
    .await?;
    Ok(Json(serde_json::json!({
        "result": result,
    })))
}
