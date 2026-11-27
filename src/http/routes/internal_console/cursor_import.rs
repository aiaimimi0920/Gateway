//! Normal management/secret-grant boundary. Login completion auto-saves, never runs inference.
use super::{
    chatgpt_oauth::{authorize, authorize_actor},
    json_with_no_store,
};
use crate::{
    console::cursor_import::{Consent, Start},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};

async fn create(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Start>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let (actor, context) = authorize_actor(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        true,
        false,
    )?;
    let consent = Consent {
        actor,
        context,
        grant: headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .into(),
    };
    let view = state
        .lifecycle
        .cursor_import
        .create(state.clone(), owner, body, consent)?;
    Ok(json_with_no_store(serde_json::json!({"session":view})))
}
async fn read(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":state.lifecycle.cursor_import.get(&id, &owner)?.view()}),
    ))
}
async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = state.lifecycle.cursor_import.get(&id, &owner)?;
    session.cancel();
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
async fn resume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let (actor, context) = authorize_actor(
        &state,
        &headers,
        token.as_deref(),
        info.as_ref(),
        true,
        false,
    )?;
    let consent = Consent {
        actor,
        context,
        grant: headers
            .get("x-secret-grant")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .into(),
    };
    let view = state
        .lifecycle
        .cursor_import
        .resume(&state, &id, &owner, consent)?;
    Ok(json_with_no_store(serde_json::json!({"session":view})))
}
pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.merge(
        Router::new()
            .route(
                "/v1/internal/gateway/console/cursor-auth-sessions",
                post(create),
            )
            .route(
                "/v1/internal/gateway/console/cursor-auth-sessions/:id",
                get(read),
            )
            .route(
                "/v1/internal/gateway/console/cursor-auth-sessions/:id/cancel",
                post(cancel),
            )
            .route(
                "/v1/internal/gateway/console/cursor-auth-sessions/:id/resume",
                post(resume),
            )
            .layer(axum::extract::DefaultBodyLimit::max(40 * 1024))
            .layer(axum::middleware::map_response(no_store)),
    )
}
