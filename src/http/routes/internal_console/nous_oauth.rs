//! Nous device actions require management identity, secret consent and the original saved revision.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::nous_oauth::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::nous_portal::OAuthClient,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
    client_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
}
async fn create(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(nous_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(nous_oauth::login_error)?;
    }
    let client =
        OAuthClient::new(&body.client_id).map_err(|e| nous_oauth::login_error(e.into()))?;
    let session = state
        .lifecycle
        .nous_oauth
        .create(&state, binding, client)
        .await?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
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
    let session = state
        .lifecycle
        .nous_oauth
        .get_owned(&id, &owner)
        .map_err(nous_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
async fn act(
    State(state): State<Arc<AppState>>,
    Path((id, action)): Path<(String, String)>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Action>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = state
        .lifecycle
        .nous_oauth
        .get_owned(&id, &owner)
        .map_err(nous_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Nous login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    let view = match action.as_str() {
        "cancel" => {
            session.cancel();
            session.view()
        }
        "poll" => state
            .lifecycle
            .nous_oauth
            .start_poll(state.clone(), &id, binding)?,
        "import" => state
            .lifecycle
            .nous_oauth
            .start_import(state.clone(), &id, binding)?,
        _ => return Err(GatewayError::bad_request("Unsupported Nous login action")),
    };
    Ok(json_with_no_store(serde_json::json!({"session":view})))
}
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.merge(
        Router::new()
            .route(
                "/v1/internal/gateway/console/nous-auth-sessions",
                post(create),
            )
            .route(
                "/v1/internal/gateway/console/nous-auth-sessions/:session_id",
                get(read),
            )
            .route(
                "/v1/internal/gateway/console/nous-auth-sessions/:session_id/:action",
                post(act),
            )
            .route(
                "/v1/internal/gateway/console/nous-model-catalog",
                post(super::nous_catalog::read),
            )
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
            .layer(axum::middleware::map_response(no_store)),
    )
}
#[cfg(all(test, feature = "line-nous-portal-oauth"))]
mod tests;
