//! Explicit catalog-only reads; persistence uses the normal revision/CAS owner.
use super::{console_request_context, json_with_no_store, required_console_management_token};
use crate::{
    error::GatewayError, http::extractors::OptionalBearerToken, provider_discovery::model_catalog,
    state::AppState,
};
use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use model_catalog::CatalogProvider;
use serde::Deserialize;
use std::{net::SocketAddr, sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRequest {
    credential_id: Option<String>,
    provider_id: Option<String>,
    api_key: Option<String>,
}

pub async fn read_anthropic_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Anthropic,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_openai_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::OpenAi,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_nvidia_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Nvidia,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_openrouter_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::OpenRouter,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_poe_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Poe,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_groq_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Groq,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_cohere_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Cohere,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

pub async fn read_gemini_model_catalog(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<CatalogRequest>,
) -> Result<Response, GatewayError> {
    read_catalog(
        CatalogProvider::Gemini,
        state,
        connect_info,
        token,
        headers,
        body,
    )
    .await
}

async fn read_catalog(
    kind: CatalogProvider,
    state: Arc<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    token: Option<String>,
    headers: HeaderMap,
    body: CatalogRequest,
) -> Result<Response, GatewayError> {
    let request = console_request_context(&headers, connect_info.as_ref());
    let actor = state.console_auth.authenticate_management_token(
        &request,
        required_console_management_token(token.as_deref(), &headers)?,
    )?;
    let grant = headers
        .get("x-secret-grant")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    let _slot = crate::provider_discovery::background::SLOTS
        .try_acquire()
        .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
    let snapshot = state.route_config.snapshot();
    let invalid = || {
        GatewayError::bad_request(
            "Catalog requires an explicit matching account at its official API origin",
        )
    };
    let key = if let Some(id) = body.credential_id {
        if body.provider_id.is_some() || body.api_key.is_some() {
            return Err(invalid());
        }
        let target = snapshot
            .select_credential_probe_target(&id)
            .ok_or_else(invalid)?;
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| {
                p.id == target.provider_id
                    && p.credentials.iter().any(|c| c.id.as_deref() == Some(&id))
            })
            .ok_or_else(invalid)?;
        if !kind.accepts(provider.preset.as_deref(), &target.payload.base_url) {
            return Err(invalid());
        }
        target.payload.api_key
    } else {
        let provider = snapshot
            .document()
            .providers
            .iter()
            .find(|p| Some(p.id.as_str()) == body.provider_id.as_deref())
            .ok_or_else(invalid)?;
        if !kind.accepts(provider.preset.as_deref(), &provider.base_url) {
            return Err(invalid());
        }
        body.api_key.unwrap_or_default()
    };
    if key.is_empty() || key.len() > 16384 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(GatewayError::bad_request("Invalid catalog API key"));
    }
    let catalog = model_catalog::fetch_catalog(kind, &key).await?;
    state
        .console_auth
        .verify_secret_grant(&request, &actor, grant)?;
    Ok(json_with_no_store(
        serde_json::json!({"revision":snapshot.revision().id(), "catalog":catalog}),
    ))
}
