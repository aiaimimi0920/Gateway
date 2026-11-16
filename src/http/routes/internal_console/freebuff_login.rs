//! FreeBuff approval is not OAuth or management authority. Every mutation uses the normal Secret Grant.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::freebuff_login::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::freebuff_login::{LoginError, LoginTransport},
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
    fingerprint_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
}

pub async fn create_freebuff_login(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !cfg!(feature = "line-freebuff-web-reverse-api") {
        return Err(freebuff_login::login_error(
            LoginError::FeatureDisabled.into(),
        ));
    }
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(freebuff_login::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(freebuff_login::login_error)?;
    }
    let fingerprint =
        freebuff_login::resolve_fingerprint(&state, &binding, body.fingerprint_id.as_deref())?;
    let session = state
        .lifecycle
        .freebuff_login
        .start(binding, transport()?, fingerprint)
        .map_err(freebuff_login::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_freebuff_login(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let session = state
        .lifecycle
        .freebuff_login
        .get_owned(&id, &owner)
        .map_err(freebuff_login::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn act_freebuff_login(
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
        .freebuff_login
        .get_owned(&id, &owner)
        .map_err(freebuff_login::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("FreeBuff login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    let view = match action.as_str() {
        "cancel" => {
            session.cancel();
            session.view()
        }
        "poll" => {
            freebuff_login::validate_target(&state, binding, session.fingerprint())?;
            state
                .lifecycle
                .freebuff_login
                .start_poll(&id, binding, transport()?)
                .map_err(freebuff_login::login_error)?
        }
        "import" => state
            .lifecycle
            .freebuff_login
            .start_import(state.clone(), &id, binding)?,
        _ => {
            return Err(GatewayError::bad_request(
                "Unsupported FreeBuff login action",
            ))
        }
    };
    Ok(json_with_no_store(serde_json::json!({"session":view})))
}
fn transport() -> Result<LoginTransport, GatewayError> {
    LoginTransport::new().map_err(|e| freebuff_login::login_error(e.into()))
}
pub async fn freebuff_login_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
