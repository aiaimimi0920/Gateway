//! Prepare a local receipt before explicit confirmation can create a remote public client.
use super::super::{chatgpt_oauth::authorize, json_with_no_store};
use super::{http, no_store};
use crate::{error::GatewayError, http::extractors::OptionalBearerToken, state::AppState};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

pub(super) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/v1/internal/gateway/console/luma-client-registrations",
            post(prepare)
                .layer(axum::extract::DefaultBodyLimit::max(4096))
                .layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-client-registrations/:id",
            get(read).layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-client-registrations/:id/start",
            post(start)
                .layer(axum::extract::DefaultBodyLimit::max(4096))
                .layer(axum::middleware::map_response(no_store)),
        )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Prepare {
    provider_id: String,
    expected_revision: String,
    #[serde(default)]
    offline_access: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Start {
    expected_revision: String,
    confirm_registration: bool,
}

async fn prepare(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Prepare>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let receipt = state
        .lifecycle
        .luma_oauth
        .prepare_registration_with_offline(
            &state,
            &owner,
            &body.provider_id,
            &body.expected_revision,
            body.offline_access,
        )
        .await?;
    Ok(json_with_no_store(
        serde_json::json!({"registration":receipt.view()}),
    ))
}
async fn read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let receipt = state
        .lifecycle
        .luma_oauth
        .load_registration(&state, &id, &owner)
        .await?;
    Ok(json_with_no_store(
        serde_json::json!({"registration":receipt.view()}),
    ))
}
async fn start(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Start>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !body.confirm_registration {
        return Err(GatewayError::bad_request(
            "Explicit Luma client registration confirmation required",
        ));
    }
    let receipt = state.lifecycle.luma_oauth.start_registration(
        &state,
        &id,
        &owner,
        &body.expected_revision,
        http()?,
    )?;
    Ok(json_with_no_store(
        serde_json::json!({"registration":receipt}),
    ))
}
