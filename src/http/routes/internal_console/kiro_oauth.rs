//! OIDC user/device codes are never management authority. Mutation uses normal Origin/Secret Grant.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::kiro_oauth::{self, Binding},
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::kiro_oauth::{Directory, OAuthError, Scope},
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
#[serde(
    tag = "authMethod",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LoginDirectory {
    BuilderId {},
    IdentityCenter {
        directory_region: String,
        start_url: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
    directory: LoginDirectory,
    scopes: Vec<Scope>,
    machine_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    expected_revision: String,
}

pub async fn create_kiro_oauth(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if !cfg!(feature = "line-kiro-official-vendor-api") {
        return Err(kiro_oauth::login_error(OAuthError::FeatureDisabled.into()));
    }
    let mut binding = Binding::new(&owner, &body.provider_id, &body.expected_revision)
        .map_err(kiro_oauth::login_error)?;
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(kiro_oauth::login_error)?;
    }
    let machine = kiro_oauth::resolve_machine(&state, &binding, body.machine_id.as_deref())?;
    let directory = match body.directory {
        LoginDirectory::BuilderId {} => Directory::builder_id(),
        LoginDirectory::IdentityCenter {
            directory_region,
            start_url,
        } => Directory::identity_center(&directory_region, &start_url)
            .map_err(|e| kiro_oauth::login_error(e.into()))?,
    };
    let http = transport()?;
    let session = state
        .lifecycle
        .kiro_oauth
        .start(binding, http, directory, body.scopes, machine)
        .map_err(kiro_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn get_kiro_oauth(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), false)?;
    let session = state
        .lifecycle
        .kiro_oauth
        .get_owned(&id, &owner)
        .map_err(kiro_oauth::login_error)?;
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
pub async fn act_kiro_oauth(
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
        .kiro_oauth
        .get_owned(&id, &owner)
        .map_err(kiro_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Kiro login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    let view = match action.as_str() {
        "cancel" => {
            session.cancel();
            session.view()
        }
        "poll" => {
            kiro_oauth::validate_target(&state, binding, session.machine())?;
            state
                .lifecycle
                .kiro_oauth
                .start_poll(&id, binding, transport()?)
                .map_err(kiro_oauth::login_error)?
        }
        "import" => state
            .lifecycle
            .kiro_oauth
            .start_import(state.clone(), &id, binding)?,
        _ => return Err(GatewayError::bad_request("Unsupported Kiro login action")),
    };
    Ok(json_with_no_store(serde_json::json!({"session":view})))
}
fn transport() -> Result<rquest::Client, GatewayError> {
    rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Kiro OAuth transport unavailable"))
}

/// Also cover admission/extractor failures, which do not pass through json_with_no_store.
pub async fn kiro_oauth_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
mod tests;
