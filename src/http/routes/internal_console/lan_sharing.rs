//! LAN sharing changes belong to the selected backend and take effect after atomic persistence.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError,
    http::{
        extractors::OptionalBearerToken,
        lan_sharing::{Runtime, Settings},
    },
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Extension, Json,
};
use std::{net::SocketAddr, sync::Arc};

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    Extension(runtime): Extension<Arc<Runtime>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    execute(state, runtime, peer, token, headers, None).await
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    Extension(runtime): Extension<Arc<Runtime>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Settings>,
) -> Result<Response, GatewayError> {
    execute(state, runtime, peer, token, headers, Some(body)).await
}

async fn execute(
    state: Arc<AppState>,
    runtime: Arc<Runtime>,
    peer: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    update: Option<Settings>,
) -> Result<Response, GatewayError> {
    let context = console_request_context(&headers, peer.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    let settings = tokio::task::spawn_blocking(move || {
        state
            .console_auth
            .authenticate_management_token(&context, &token)?;
        match update {
            Some(settings) => runtime.save(settings),
            None => runtime.settings(),
        }
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("LAN sharing task failed"))??;
    Ok(json_with_no_store(
        serde_json::json!({"enabled": settings.enabled}),
    ))
}
