//! Management-only lookup with transient, caller-supplied Service Key material.
use super::chatgpt_oauth::authorize_actor;
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken, protocol::sap_aicore,
    state::AppState,
};
use axum::{
    extract::{rejection::JsonRejection, ConnectInfo, DefaultBodyLimit, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::map_response,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};
mod responses;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Lookup {
    service_key: String,
    resource_group: String,
    #[serde(default)]
    protocol: sap_aicore::deployments::DeploymentProtocol,
}

pub(crate) fn mount_sap_deployments(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    responses::mount(router).route(
        "/v1/internal/gateway/console/sap-aicore-deployments",
        post(read)
            .layer(DefaultBodyLimit::max(128 * 1024))
            .layer(map_response(no_store)),
    )
}

async fn read(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(bearer): OptionalBearerToken,
    headers: HeaderMap,
    input: Result<Json<Lookup>, JsonRejection>,
) -> Result<Response, GatewayError> {
    authorize_actor(
        &state,
        &headers,
        bearer.as_deref(),
        info.as_ref(),
        true,
        false,
    )?;
    if !cfg!(feature = "line-sap-aicore-chat") {
        return Err(
            GatewayError::conflict("SAP AI Core implementation line was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    // Never reproduce serde's error, which can contain caller-provided property names.
    let Json(input) = input.map_err(|_| sap_aicore::invalid())?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let catalog = sap_aicore::deployments::discover(
        state.upstream_client.client(),
        &state.upstream_client.sap_aicore,
        &input.service_key,
        &input.resource_group,
        input.protocol,
    )
    .await?;
    let (actor, _) = authorize_actor(
        &state,
        &headers,
        bearer.as_deref(),
        info.as_ref(),
        false,
        false,
    )?;
    if !actor.secret_access_granted() {
        return Err(crate::console::auth::console_secret_access_required_error());
    }
    Ok(Json(catalog).into_response())
}

async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
