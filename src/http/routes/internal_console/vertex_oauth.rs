//! Full Vertex management has its own authorization identity and never borrows Antigravity clients.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::vertex_oauth::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::vertex_oauth::{OAuthClient, OAuthError, VertexTarget},
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    response::Response,
    Json,
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
    client_secret: Option<String>,
    target: VertexTarget,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
    callback_url: Option<String>,
}

pub async fn create_vertex_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !cfg!(feature = "line-google-agent-platform-official") {
        return Err(vertex_oauth::login_error(
            OAuthError::FeatureDisabled.into(),
        ));
    }
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(vertex_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(vertex_oauth::login_error)?;
    }
    let client = OAuthClient::new(&body.client_id, body.client_secret.as_deref())
        .map_err(|e| vertex_oauth::login_error(e.into()))?;
    let session = state
        .lifecycle
        .vertex_oauth
        .create(&state, binding, client, body.target, transport()?)
        .await?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_vertex_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let session = state
        .lifecycle
        .vertex_oauth
        .get_owned(&id, &owner)
        .map_err(vertex_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn act_vertex_oauth(
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
        .vertex_oauth
        .get_owned(&id, &owner)
        .map_err(vertex_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Vertex login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    match action.as_str() {
        "cancel" if body.callback_url.is_none() => session.cancel(),
        "complete" => {
            vertex_oauth::validate_target(&state, binding, session.target())?;
            let callback = body
                .callback_url
                .ok_or_else(|| GatewayError::bad_request("callbackUrl is required"))?;
            session
                .complete(&transport()?, &callback)
                .await
                .map_err(vertex_oauth::login_error)?;
        }
        "import" if body.callback_url.is_none() => {
            let view = state
                .lifecycle
                .vertex_oauth
                .start_import(state.clone(), &id, binding)?;
            return Ok(json_with_no_store(serde_json::json!({"session":view})));
        }
        _ => return Err(GatewayError::bad_request("Unsupported Vertex login action")),
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
fn transport() -> Result<rquest::Client, GatewayError> {
    rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Vertex OAuth transport unavailable"))
}
/// Include auth and JSON-extractor rejections, not only successful JSON responses.
pub async fn vertex_oauth_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(all(test, feature = "line-google-agent-platform-official"))]
mod tests;
