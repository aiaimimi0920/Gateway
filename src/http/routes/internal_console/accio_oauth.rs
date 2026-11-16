//! Management identity, Secret Grant and saved revision remain independent of Accio PKCE state.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::accio_oauth::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::accio_oauth::{LoginSite, OAuthClient, OAuthError},
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
#[serde(rename_all = "snake_case")]
pub enum Site {
    China,
    Global,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
    client_id: String,
    login_site: Site,
    callback_port: u16,
    utdid: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
    callback_url: Option<String>,
}

pub async fn create_accio_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !cfg!(feature = "line-accio-web-reverse-api") {
        return Err(accio_oauth::login_error(OAuthError::FeatureDisabled.into()));
    }
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(accio_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(accio_oauth::login_error)?;
    }
    let utdid = accio_oauth::resolve_utdid(&state, &binding, body.utdid.as_deref())?;
    let client =
        OAuthClient::new(&body.client_id).map_err(|e| accio_oauth::login_error(e.into()))?;
    let site = match body.login_site {
        Site::China => LoginSite::China,
        Site::Global => LoginSite::Global,
    };
    let session = state
        .lifecycle
        .accio_oauth
        .create(binding, client, site, body.callback_port, utdid)
        .map_err(accio_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_accio_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let session = state
        .lifecycle
        .accio_oauth
        .get_owned(&id, &owner)
        .map_err(accio_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn act_accio_oauth(
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
        .accio_oauth
        .get_owned(&id, &owner)
        .map_err(accio_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Accio login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    match action.as_str() {
        "cancel" if body.callback_url.is_none() => session.cancel(),
        "complete" => {
            accio_oauth::validate_target(&state, binding)?;
            let callback = body
                .callback_url
                .ok_or_else(|| GatewayError::bad_request("callbackUrl is required"))?;
            let http = rquest::Client::builder().build().map_err(|_| {
                GatewayError::service_unavailable("Accio OAuth transport unavailable")
            })?;
            session
                .complete(&http, &callback)
                .await
                .map_err(accio_oauth::login_error)?;
        }
        "import" if body.callback_url.is_none() => {
            let view = state
                .lifecycle
                .accio_oauth
                .start_import(state.clone(), &id, binding)?;
            return Ok(json_with_no_store(serde_json::json!({"session":view})));
        }
        _ => return Err(GatewayError::bad_request("Unsupported Accio login action")),
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}

#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod tests;
