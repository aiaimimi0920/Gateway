//! Bootstrap and session handlers preserve authentication and secret-grant response contracts.

use super::{
    console_request_context, json_with_no_store, required_console_management_token,
    BootstrapRequest, ConfirmSecretAccessRequest, RotateSessionRequest,
};
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::state::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn get_bootstrap_status(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let status = state.console_auth.bootstrap_status(&request)?;
    Ok(json_with_no_store(
        serde_json::to_value(status).expect("bootstrap status JSON"),
    ))
}

pub async fn bootstrap_console_admin(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<BootstrapRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.bootstrap(&request, &body.token)?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console administrator configured"
    })))
}

pub async fn verify_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let session = state.console_auth.verify_session(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        Some(state.route_config.snapshot().revision().id()),
    )?;
    Ok(json_with_no_store(
        serde_json::to_value(session).expect("console session JSON"),
    ))
}

pub async fn confirm_console_secret_access(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<ConfirmSecretAccessRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let grant = state.console_auth.confirm_secret_access(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        &body.token,
    )?;
    Ok(json_with_no_store(
        serde_json::to_value(grant).expect("secret grant JSON"),
    ))
}

pub async fn rotate_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<RotateSessionRequest>,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.rotate_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
        &body.new_token,
    )?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console management token rotated"
    })))
}

pub async fn logout_console_session(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    state.console_auth.logout(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    Ok(json_with_no_store(serde_json::json!({
        "success": true,
        "message": "Gateway console session cleared"
    })))
}
