//! LobsterAI device actions require management identity, secret consent and the original saved revision.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::lobsterai_oauth::{self, Binding},
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
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
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
        .map_err(lobsterai_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(lobsterai_oauth::login_error)?;
    }
    let session = state
        .lifecycle
        .lobsterai_oauth
        .create(&state, binding)
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
        .lobsterai_oauth
        .get_owned(&id, &owner)
        .map_err(lobsterai_oauth::login_error)?;
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
        .lobsterai_oauth
        .get_owned(&id, &owner)
        .map_err(lobsterai_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("LobsterAI login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    let view = match action.as_str() {
        "cancel" => {
            session.cancel();
            session.view()
        }
        "import" => state
            .lifecycle
            .lobsterai_oauth
            .start_import(state.clone(), &id, binding)?,
        _ => {
            return Err(GatewayError::bad_request(
                "Unsupported LobsterAI login action",
            ))
        }
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
                "/v1/internal/gateway/console/lobsterai-auth-sessions",
                post(create),
            )
            .route(
                "/v1/internal/gateway/console/lobsterai-auth-sessions/:session_id",
                get(read),
            )
            .route(
                "/v1/internal/gateway/console/lobsterai-auth-sessions/:session_id/:action",
                post(act),
            )
            .route(
                "/v1/internal/gateway/console/lobsterai-model-catalog",
                post(super::lobsterai_catalog::read),
            )
            .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
            .layer(axum::middleware::map_response(no_store)),
    )
}
#[cfg(test)]
mod tests;
