//! Explicit saved-account read-only discovery, with secret consent and revision fencing; no writes.
use super::{chatgpt_oauth::authorize, json_with_no_store};
use crate::{
    error::GatewayError,
    http::extractors::OptionalBearerToken,
    protocol::{luma_mcp, luma_oauth::TokenMaterial},
    routing::config::RouteConfigSnapshot,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json, Router,
};
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

pub(crate) fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/v1/internal/gateway/console/luma-mcp-tools",
            axum::routing::post(read_tools)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
                .layer(axum::middleware::map_response(no_store)),
        )
        .route(
            "/v1/internal/gateway/console/luma-mcp-models",
            axum::routing::post(read_models)
                .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
                .layer(axum::middleware::map_response(no_store)),
        )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
}

fn resolve(snapshot: &RouteConfigSnapshot, input: &Input) -> Result<TokenMaterial, GatewayError> {
    let provider = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == input.provider_id && crate::routing::config::luma_mcp::is_provider(p))
        .ok_or_else(luma_mcp::boundary::invalid)?;
    let credential = provider
        .credentials
        .iter()
        .find(|c| c.id.as_deref() == Some(input.credential_id.as_str()))
        .ok_or_else(luma_mcp::boundary::invalid)?;
    let context = credential
        .luma_oauth
        .as_ref()
        .ok_or_else(luma_mcp::boundary::invalid)?;
    context
        .restore_material(credential.api_key.as_deref().unwrap_or(""))
        .map_err(|error| {
            GatewayError::bad_request(error.to_string()).with_code("luma_mcp_material_unavailable")
        })
}

async fn read_tools(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Response, GatewayError> {
    read(state, info, token, headers, input, None).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ModelInput {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    action: String,
    model: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

async fn read_models(
    State(state): State<Arc<AppState>>,
    info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(input): Json<ModelInput>,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    let invalid = || {
        GatewayError::bad_request("Invalid Luma model query").with_code("luma_model_query_invalid")
    };
    let query = match (input.action.as_str(), input.model, input.kind) {
        ("list", None, None) => luma_mcp::ModelQuery::List,
        ("describe", Some(model), Some(kind)) => luma_mcp::ModelQuery::Describe { model, kind },
        _ => return Err(invalid()),
    };
    query.validate().map_err(|_| invalid())?;
    read(
        state,
        info,
        token,
        headers,
        Input {
            provider_id: input.provider_id,
            credential_id: input.credential_id,
            expected_revision: input.expected_revision,
        },
        Some(query),
    )
    .await
}

async fn read(
    state: Arc<AppState>,
    info: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    input: Input,
    query: Option<luma_mcp::ModelQuery>,
) -> Result<Response, GatewayError> {
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(GatewayError::service_unavailable("Gateway is stopping."));
    }
    let snapshot = state.route_config.snapshot();
    if snapshot.revision().id() != input.expected_revision {
        return Err(revision_changed());
    }
    let material = resolve(&snapshot, &input)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    // Dedicated default client has no cookie store or caller-defined retry/header behavior.
    let http = rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::service_unavailable("Luma MCP transport unavailable."))?;
    let (key, result) = match query {
        Some(query) => (
            "result",
            luma_mcp::read_models(&http, &material, &query).await,
        ),
        None => (
            "tools",
            luma_mcp::inspect_tools(&http, &material)
                .await
                .map(|tools| serde_json::json!(tools)),
        ),
    };
    let result = result.map_err(|error| {
        GatewayError::service_unavailable(error.to_string()).with_code("luma_mcp_catalog_failed")
    })?;
    authorize(&state, &headers, token.as_deref(), info.as_ref(), true)?;
    if state.route_config.snapshot().revision().id() != snapshot.revision().id() {
        return Err(revision_changed());
    }
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(),
        "providerId":input.provider_id,"credentialId":input.credential_id,(key):result}),
    ))
}

fn revision_changed() -> GatewayError {
    GatewayError::conflict("Route configuration changed; retry from the saved Luma MCP account.")
        .with_code("console_revision_conflict")
}
async fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
