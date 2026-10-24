//! Authenticated backend log settings/downloads. No path or client-machine state is accepted.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    console::diagnostics, error::GatewayError, http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, connect_info.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let snapshot = tokio::task::spawn_blocking(move || {
        state
            .console_auth
            .authenticate_management_token(&context, &token)?;
        diagnostics::settings(&state.config.console).map_err(GatewayError::service_unavailable)
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Gateway diagnostics task failed"))??;
    Ok(json_with_no_store(serde_json::to_value(snapshot).map_err(
        |_| GatewayError::service_unavailable("Cannot encode log settings"),
    )?))
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(policy): Json<diagnostics::LogPolicy>,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, connect_info.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let snapshot = tokio::task::spawn_blocking(move || {
        state
            .console_auth
            .authenticate_management_token(&context, &token)?;
        policy.validate().map_err(GatewayError::bad_request)?;
        diagnostics::save_policy(&state.config.console, policy)
            .map_err(GatewayError::service_unavailable)
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Gateway diagnostics task failed"))??;
    Ok(json_with_no_store(serde_json::to_value(snapshot).map_err(
        |_| GatewayError::service_unavailable("Cannot encode log settings"),
    )?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    kind: ExportKind,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ExportKind {
    Diagnostics,
    Logs,
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ExportRequest>,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, connect_info.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let download = tokio::task::spawn_blocking(move || {
        state
            .console_auth
            .authenticate_management_token(&context, &token)?;
        match body.kind {
            ExportKind::Diagnostics => diagnostics::export_diagnostics(&state.config),
            ExportKind::Logs => diagnostics::export_logs(&state.config.console),
        }
        .map_err(GatewayError::service_unavailable)
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Gateway diagnostics export task failed"))??;
    Ok(json_with_no_store(serde_json::to_value(download).map_err(
        |_| GatewayError::service_unavailable("Cannot encode log export"),
    )?))
}
