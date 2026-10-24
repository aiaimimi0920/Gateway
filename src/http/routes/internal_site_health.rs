//! Management-only site operations. No credential grants or credential test writes are involved.
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

fn no_store(value: serde_json::Value) -> Response {
    ([("Cache-Control", "no-store")], Json(value)).into_response()
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    super::internal_gateway::assert_management_access(&state, token.as_deref(), &headers)?;
    Ok(no_store(crate::site_health::runtime::entries(&state)))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    super::internal_gateway::assert_management_access(&state, token.as_deref(), &headers)?;
    let report = tokio::select! {
        _=state.shutdown.wait()=>return Err(GatewayError::service_unavailable("Gateway is shutting down")),
        result=crate::site_health::runtime::refresh(&state,&id,true)=>result?,
    };
    Ok(no_store(serde_json::json!({"snapshot":report})))
}

pub async fn refresh_plan(
    State(state): State<Arc<AppState>>,
    Path((id, profile)): Path<(String, String)>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    super::internal_gateway::assert_management_access(&state, token.as_deref(), &headers)?;
    let report = tokio::select! {
        _=state.shutdown.wait()=>return Err(GatewayError::service_unavailable("Gateway is shutting down")),
        result=crate::site_health::runtime::refresh_plan(&state,&id,Some(&profile),true)=>result?,
    };
    Ok(no_store(serde_json::json!({"snapshot":report})))
}
