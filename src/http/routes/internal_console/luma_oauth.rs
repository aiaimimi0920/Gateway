//! OAuth callback authority and management/save consent remain separate.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    console::luma_oauth::{self, Binding},
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
#[path = "luma_registration.rs"]
mod registration;

pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    registration::mount(router)
        .route(
            "/v1/internal/gateway/console/luma-refresh-sessions",
            post(prepare_refresh)
                .layer(axum::extract::DefaultBodyLimit::max(4096))
                .layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-refresh-sessions/:id",
            get(read).layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-refresh-sessions/:id/:action",
            post(act)
                .layer(axum::extract::DefaultBodyLimit::max(4096))
                .layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-auth-sessions",
            post(create)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
                .layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-auth-sessions/:id",
            get(read).layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-auth-sessions/:id/:action",
            post(act)
                .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
                .layer(axum::middleware::map_response(no_store)),
        )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Create {
    provider_id: String,
    expected_revision: String,
    credential_id: Option<String>,
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    offline_access: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action {
    expected_revision: String,
    callback_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Refresh {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
}
async fn prepare_refresh(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Refresh>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let session = state.lifecycle.luma_oauth.prepare_refresh(
        &state,
        &owner,
        &body.provider_id,
        &body.credential_id,
        &body.expected_revision,
    )?;
    Ok(json_with_no_store(serde_json::json!({"session":session})))
}

async fn create(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<Response, GatewayError> {
    let (owner, _) = authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let mut binding = Binding::new(
        &owner,
        &body.provider_id,
        &body.expected_revision,
        &body.client_id,
        &body.redirect_uri,
    )
    .map_err(luma_oauth::login_error)?;
    if body.offline_access {
        binding = binding.with_offline_access();
    }
    if let Some(id) = body.credential_id {
        binding = binding
            .with_credential(&id)
            .map_err(luma_oauth::login_error)?;
    }
    let session = state
        .lifecycle
        .luma_oauth
        .create(&state, binding, http()?)
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
        .luma_oauth
        .get_owned(&id, &owner)
        .map_err(luma_oauth::login_error)?;
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
        .luma_oauth
        .get_owned(&id, &owner)
        .map_err(luma_oauth::login_error)?;
    let binding = session.binding();
    if body.expected_revision != binding.revision() {
        return Err(
            GatewayError::conflict("Luma login revision does not match the session")
                .with_code("console_revision_conflict"),
        );
    }
    match action.as_str() {
        "resume" if body.callback_url.is_none() => {
            let view = state
                .lifecycle
                .luma_oauth
                .resume_refresh(state.clone(), &id, &owner)?;
            return Ok(json_with_no_store(serde_json::json!({"session":view})));
        }
        "cancel" if body.callback_url.is_none() => session.cancel(),
        "complete" => {
            luma_oauth::validate_target(&state, binding)?;
            let callback = body
                .callback_url
                .ok_or_else(|| GatewayError::bad_request("callbackUrl is required"))?;
            session
                .complete(&http()?, &callback)
                .await
                .map_err(luma_oauth::login_error)?;
        }
        "import" if body.callback_url.is_none() => {
            let view = state
                .lifecycle
                .luma_oauth
                .start_import(state.clone(), &id, binding)?;
            return Ok(json_with_no_store(serde_json::json!({"session":view})));
        }
        _ => return Err(GatewayError::bad_request("Unsupported Luma login action")),
    }
    Ok(json_with_no_store(
        serde_json::json!({"session":session.view()}),
    ))
}
fn http() -> Result<rquest::Client, GatewayError> {
    let builder = rquest::Client::builder();
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder
        .build()
        .map_err(|_| GatewayError::service_unavailable("Luma OAuth transport unavailable"))
}
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
