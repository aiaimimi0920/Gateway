//! Management-only default egress settings. Tests never save or invoke a model.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken,
    http_client::proxy_settings as policy, state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

async fn authorize(
    state: Arc<AppState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
) -> Result<(), GatewayError> {
    let context = console_request_context(&headers, peer.as_ref());
    let token = required_console_management_token(token.as_deref(), &headers)?.to_owned();
    tokio::task::spawn_blocking(move || {
        state
            .console_auth
            .authenticate_management_token(&context, &token)
    })
    .await
    .map_err(|_| GatewayError::service_unavailable("Proxy authorization failed"))??;
    Ok(())
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    authorize(state.clone(), peer, token, headers).await?;
    let snapshot =
        tokio::task::spawn_blocking(move || policy::snapshot(&state.config.console.state_dir))
            .await
            .map_err(|_| GatewayError::service_unavailable("Proxy settings task failed"))?
            .map_err(GatewayError::service_unavailable)?;
    Ok(json_with_no_store(serde_json::to_value(snapshot).map_err(
        |_| GatewayError::service_unavailable("Cannot encode proxy settings"),
    )?))
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<policy::Policy>,
) -> Result<Response, GatewayError> {
    authorize(state.clone(), peer, token, headers).await?;
    let body = body.normalized().map_err(GatewayError::bad_request)?;
    let snapshot =
        tokio::task::spawn_blocking(move || policy::save(&state.config.console.state_dir, body))
            .await
            .map_err(|_| GatewayError::service_unavailable("Proxy settings task failed"))?
            .map_err(GatewayError::service_unavailable)?;
    Ok(json_with_no_store(serde_json::to_value(snapshot).map_err(
        |_| GatewayError::service_unavailable("Cannot encode proxy settings"),
    )?))
}

pub async fn test_settings(
    State(state): State<Arc<AppState>>,
    peer: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<policy::Policy>,
) -> Result<Response, GatewayError> {
    authorize(state, peer, token, headers).await?;
    let body = body.normalized().map_err(GatewayError::bad_request)?;
    static TEST_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _permit = TEST_SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::service_unavailable("Proxy test already in progress"))?;
    let client = policy::rquest(rquest::Client::builder(), &body)
        .redirect(rquest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| GatewayError::bad_request("Cannot build proxy test client"))?;
    let start = Instant::now();
    // Fixed public HTTPS endpoint: no arbitrary target, model credentials, cookies or response body.
    let response = client
        .head("https://example.com/")
        .send()
        .await
        .map_err(|_| {
            GatewayError::service_unavailable("Proxy test could not reach the HTTPS test endpoint")
        })?;
    if !response.status().is_success() {
        return Err(GatewayError::service_unavailable(
            "HTTPS test endpoint rejected the request",
        ));
    }
    Ok(json_with_no_store(
        serde_json::json!({"ok": true, "latencyMs": start.elapsed().as_millis() as u64}),
    ))
}
